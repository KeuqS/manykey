//! Linux backend: one reader thread per evdev keyboard, plus a hot-plug scanner.
//!
//! Needs read access to `/dev/input/event*` (usually the `input` group).
//! Keyboards are discovered from `/proc/bus/input/devices` (handler `kbd` and
//! the EV_REP capability bit, which separates real keyboards from power buttons
//! and the like).

use crate::{
    from_evdev, Backend, Event, EventOrigin, EventSink, KeyEvent, KeyState, KeyboardId, KeyboardInfo, Options, Scope,
};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const EV_KEY: u16 = 1;
const EV_REP_BIT: u64 = 1 << 20;

pub struct LinuxBackend {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Backend for LinuxBackend {}

impl LinuxBackend {
    pub fn start(tx: EventSink, options: &Options) -> io::Result<Self> {
        // evdev has no notion of window focus. Refuse rather than silently
        // reporting everything to a caller that asked for less.
        if options.scope != Scope::Global {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Scope::OwnProcessFocused is not supported on Linux (evdev cannot tell which window has focus)",
            ));
        }
        let release_on_unplug = options.release_on_unplug;
        // Fail early if /proc is unusable rather than silently reporting nothing.
        list_keyboards()?;
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let thread = thread::Builder::new()
            .name("multikeyboard-scan".into())
            .spawn(move || scan_loop(tx, s, release_on_unplug))?;
        Ok(Self { stop, thread: Some(thread) })
    }
}

impl Drop for LinuxBackend {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Found {
    name: String,
    path: String,
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    phys: Option<String>,
}

fn list_keyboards() -> io::Result<Vec<Found>> {
    let text = std::fs::read_to_string("/proc/bus/input/devices")?;
    let mut out = Vec::new();
    for block in text.split("\n\n") {
        let (mut name, mut event, mut is_kbd, mut ev_bits) = (String::new(), None, false, 0u64);
        let (mut vendor_id, mut product_id, mut phys) = (None, None, None);
        for line in block.lines() {
            if let Some(v) = line.strip_prefix("N: Name=") {
                name = v.trim_matches('"').to_string();
            } else if let Some(v) = line.strip_prefix("H: Handlers=") {
                is_kbd = v.split_whitespace().any(|h| h == "kbd");
                event = v.split_whitespace().find(|h| h.starts_with("event")).map(String::from);
            } else if let Some(v) = line.strip_prefix("I: ") {
                // I: Bus=0003 Vendor=046d Product=c31c Version=0110
                for kv in v.split_whitespace() {
                    match kv.split_once('=') {
                        Some(("Vendor", x)) => vendor_id = u16::from_str_radix(x, 16).ok(),
                        Some(("Product", x)) => product_id = u16::from_str_radix(x, 16).ok(),
                        _ => {}
                    }
                }
            } else if let Some(v) = line.strip_prefix("P: Phys=") {
                phys = Some(v.trim().to_string()).filter(|p| !p.is_empty());
            } else if let Some(v) = line.strip_prefix("B: EV=") {
                ev_bits = u64::from_str_radix(v.trim(), 16).unwrap_or(0);
            }
        }
        if let (true, Some(ev)) = (is_kbd && ev_bits & EV_REP_BIT != 0, event) {
            out.push(Found { name, path: format!("/dev/input/{ev}"), vendor_id, product_id, phys });
        }
    }
    Ok(out)
}

fn scan_loop(tx: EventSink, stop: Arc<AtomicBool>, release_on_unplug: bool) {
    let active: Arc<Mutex<HashSet<String>>> = Arc::default();
    // Identity is name + USB ids + port, so replugging into the same port
    // yields the same KeyboardId even though the eventN number changes.
    let mut ids: HashMap<String, KeyboardId> = HashMap::new();
    let mut readers: Vec<JoinHandle<()>> = Vec::new();

    while !stop.load(Ordering::Relaxed) {
        for kb in list_keyboards().unwrap_or_default() {
            if active.lock().unwrap().contains(&kb.path) {
                continue;
            }
            let Ok(file) = File::open(&kb.path) else { continue };
            let ident = format!("{}|{:?}|{:?}|{:?}", kb.name, kb.vendor_id, kb.product_id, kb.phys);
            let next = ids.len() as u64 + 1;
            let id = *ids.entry(ident).or_insert(KeyboardId(next));
            active.lock().unwrap().insert(kb.path.clone());
            tx.send(Event::KeyboardAdded(KeyboardInfo {
                id,
                name: kb.name,
                path: kb.path.clone(),
                vendor_id: kb.vendor_id,
                product_id: kb.product_id,
                location: kb.phys,
            }));
            let (tx, stop, active, path) = (tx.clone(), stop.clone(), active.clone(), kb.path);
            readers.push(thread::spawn(move || {
                read_device(file, id, &tx, &stop, release_on_unplug);
                active.lock().unwrap().remove(&path);
                tx.send(Event::KeyboardRemoved(id));
            }));
        }
        thread::sleep(Duration::from_millis(500));
    }
    for r in readers {
        let _ = r.join();
    }
}

/// Events that release every key in `held`, in a stable order.
fn release_events(id: KeyboardId, held: &HashSet<u32>) -> Vec<KeyEvent> {
    let mut codes: Vec<u32> = held.iter().copied().collect();
    codes.sort_unstable();
    codes
        .into_iter()
        .map(|code| KeyEvent {
            keyboard: id,
            key: from_evdev(code),
            native_code: code,
            state: KeyState::Released,
            origin: EventOrigin::Synthesized,
        })
        .collect()
}

fn read_device(mut file: File, id: KeyboardId, tx: &EventSink, stop: &AtomicBool, release_on_unplug: bool) {
    const SZ: usize = std::mem::size_of::<libc::input_event>();
    let mut buf = [0u8; SZ];
    let mut held: HashSet<u32> = HashSet::new();
    // Called when the device goes away (not when we're shutting down).
    let finish = |held: &HashSet<u32>| {
        if release_on_unplug {
            for ev in release_events(id, held) {
                tx.send(Event::Key(ev));
            }
        }
    };
    while !stop.load(Ordering::Relaxed) {
        // Poll with a timeout so we notice `stop` without needing a wakeup fd.
        let mut pfd = libc::pollfd { fd: file.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        let n = unsafe { libc::poll(&mut pfd, 1, 200) };
        if n == 0 {
            continue;
        }
        if n < 0 || pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            return finish(&held);
        }
        if file.read_exact(&mut buf).is_err() {
            return finish(&held); // ENODEV on unplug
        }
        let ev: libc::input_event = unsafe { std::ptr::read_unaligned(buf.as_ptr().cast()) };
        if ev.type_ != EV_KEY {
            continue;
        }
        let code = ev.code as u32;
        let state = match ev.value {
            0 => {
                held.remove(&code);
                KeyState::Released
            }
            1 => {
                held.insert(code);
                KeyState::Pressed
            }
            2 => KeyState::Repeat,
            _ => continue,
        };
        tx.send(Event::Key(KeyEvent {
            keyboard: id,
            key: from_evdev(code),
            native_code: code,
            state,
            origin: EventOrigin::Native,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_events_cover_every_held_key_in_order() {
        let held: HashSet<u32> = [30, 42].into_iter().collect(); // A, LeftShift
        let evs = release_events(KeyboardId(3), &held);
        assert_eq!(evs.len(), 2);
        assert!(evs.iter().all(|e| e.state == KeyState::Released && e.keyboard == KeyboardId(3)));
        assert!(evs.iter().all(|e| e.origin == EventOrigin::Synthesized));
        assert_eq!(evs[0].native_code, 30);
        assert_eq!(evs[1].native_code, 42);
    }
}
