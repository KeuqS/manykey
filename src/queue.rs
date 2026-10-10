//! Bounded event queue shared between a backend (producer) and a [`Listener`]
//! (consumer), plus an OS wait handle that is signalled while events are pending.
//!
//! Overflow policy: when full, one event is discarded, preferring (in order)
//! the oldest key repeat, press, then release, and only then device events.
//! Discarding a press is harmless to held-key tracking; discarding a release is
//! not, so those go last. Every discard queues an [`Event::Overflow`] marker
//! (delivered before the remaining events) so consumers know to resync.

use crate::{Event, KeyState, Listener};
use std::collections::VecDeque;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

struct Inner {
    q: VecDeque<Event>,
    overflow_pending: bool,
}

pub(crate) struct Shared {
    q: Mutex<Inner>,
    cv: Condvar,
    capacity: usize,
    dropped: AtomicU64,
    signal: Signal,
}

/// Producer end handed to backends.
#[derive(Clone)]
pub struct EventSink(pub(crate) Arc<Shared>);

impl Shared {
    pub(crate) fn new(capacity: usize) -> io::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            q: Mutex::new(Inner { q: VecDeque::new(), overflow_pending: false }),
            cv: Condvar::new(),
            capacity: capacity.max(1),
            dropped: AtomicU64::new(0),
            signal: Signal::new()?,
        }))
    }

    /// Take the next event (or the pending overflow marker) from `g`.
    fn take(&self, g: &mut Inner) -> Option<Event> {
        let e = if g.overflow_pending {
            g.overflow_pending = false;
            Some(Event::Overflow { dropped: self.dropped() })
        } else {
            g.q.pop_front()
        };
        if g.q.is_empty() && !g.overflow_pending {
            self.signal.reset();
        }
        e
    }

    pub(crate) fn try_pop(&self) -> Option<Event> {
        let mut g = self.q.lock().unwrap();
        self.take(&mut g)
    }

    pub(crate) fn pop_timeout(&self, timeout: Option<Duration>) -> Option<Event> {
        let deadline = timeout.map(|t| Instant::now() + t);
        let mut g = self.q.lock().unwrap();
        loop {
            if let Some(e) = self.take(&mut g) {
                return Some(e);
            }
            g = match deadline {
                None => self.cv.wait(g).unwrap(),
                Some(d) => {
                    let now = Instant::now();
                    if now >= d {
                        return None;
                    }
                    self.cv.wait_timeout(g, d - now).unwrap().0
                }
            };
        }
    }

    pub(crate) fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub(crate) fn raw_handle(&self) -> isize {
        self.signal.raw()
    }
}

impl EventSink {
    pub fn send(&self, event: Event) {
        let s = &self.0;
        let mut g = s.q.lock().unwrap();
        if g.q.len() >= s.capacity {
            let find = |want: &dyn Fn(&Event) -> bool| g.q.iter().position(want);
            let victim = find(&|e| matches!(e, Event::Key(k) if k.state == KeyState::Repeat))
                .or_else(|| find(&|e| matches!(e, Event::Key(k) if k.state == KeyState::Pressed)))
                .or_else(|| find(&|e| matches!(e, Event::Key(_))))
                .unwrap_or(0);
            g.q.remove(victim);
            s.dropped.fetch_add(1, Ordering::Relaxed);
            g.overflow_pending = true;
        }
        g.q.push_back(event);
        s.signal.set();
        s.cv.notify_one();
    }
}

impl Listener {
    pub(crate) fn from_shared(shared: Arc<Shared>, backend: Box<dyn crate::Backend>) -> Self {
        Self { shared, _backend: backend }
    }
}

// ---- wait handle -----------------------------------------------------------

#[cfg(target_os = "linux")]
struct Signal(i32);

#[cfg(target_os = "linux")]
impl Signal {
    fn new() -> io::Result<Self> {
        let fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(fd))
    }
    fn set(&self) {
        let one = 1u64;
        unsafe { libc::write(self.0, (&one as *const u64).cast(), 8) };
    }
    fn reset(&self) {
        let mut v = 0u64;
        unsafe { libc::read(self.0, (&mut v as *mut u64).cast(), 8) };
    }
    fn raw(&self) -> isize {
        self.0 as isize
    }
}

#[cfg(target_os = "linux")]
impl Drop for Signal {
    fn drop(&mut self) {
        unsafe { libc::close(self.0) };
    }
}

#[cfg(windows)]
struct Signal(isize);

#[cfg(windows)]
impl Signal {
    fn new() -> io::Result<Self> {
        use windows_sys::Win32::System::Threading::CreateEventW;
        // Manual-reset, initially unsignalled.
        let h = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if h.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(h as isize))
    }
    fn set(&self) {
        unsafe { windows_sys::Win32::System::Threading::SetEvent(self.0 as _) };
    }
    fn reset(&self) {
        unsafe { windows_sys::Win32::System::Threading::ResetEvent(self.0 as _) };
    }
    fn raw(&self) -> isize {
        self.0
    }
}

#[cfg(windows)]
impl Drop for Signal {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0 as _) };
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
struct Signal;

#[cfg(not(any(windows, target_os = "linux")))]
impl Signal {
    fn new() -> io::Result<Self> {
        Ok(Self)
    }
    fn set(&self) {}
    fn reset(&self) {}
    fn raw(&self) -> isize {
        -1
    }
}
