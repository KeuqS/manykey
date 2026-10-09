//! Windows backend: Raw Input on a hidden message-only window.
//!
//! `RIDEV_INPUTSINK` delivers input even when our process isn't focused, and
//! `RIDEV_DEVNOTIFY` gives hot-plug notifications.

use crate::{from_windows_scancode, Backend, Event, EventSink, KeyEvent, KeyState, KeyboardId, KeyboardInfo};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use windows_sys::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub struct WindowsBackend {
    hwnd: isize,
    thread: Option<JoinHandle<()>>,
}

impl Backend for WindowsBackend {}

impl WindowsBackend {
    pub fn start(tx: EventSink) -> io::Result<Self> {
        let (ready_tx, ready_rx) = mpsc::channel::<io::Result<isize>>();
        let thread = thread::Builder::new()
            .name("multikeyboard-rawinput".into())
            .spawn(move || run_message_loop(tx, ready_tx))?;
        match ready_rx.recv() {
            Ok(Ok(hwnd)) => Ok(Self { hwnd, thread: Some(thread) }),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(io::Error::other("raw input thread died during startup")),
        }
    }
}

impl Drop for WindowsBackend {
    fn drop(&mut self) {
        unsafe { PostMessageW(self.hwnd as HWND, WM_CLOSE, 0, 0) };
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct State {
    tx: EventSink,
    by_handle: HashMap<isize, KeyboardId>,
    by_path: HashMap<String, KeyboardId>,
    next_id: u64,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

impl State {
    /// Resolve a raw-input device handle to an id, announcing it if new.
    fn id_for(&mut self, handle: HANDLE) -> KeyboardId {
        if let Some(id) = self.by_handle.get(&(handle as isize)) {
            return *id;
        }
        let path = device_path(handle).unwrap_or_else(|| format!("unknown:{:?}", handle));
        let id = *self.by_path.entry(path.clone()).or_insert_with(|| {
            self.next_id += 1;
            KeyboardId(self.next_id)
        });
        self.by_handle.insert(handle as isize, id);
        let (vendor_id, product_id) = parse_vid_pid(&path);
        self.tx.send(Event::KeyboardAdded(KeyboardInfo {
            id,
            name: path.clone(),
            path,
            vendor_id,
            product_id,
            location: None,
        }));
        id
    }
}

/// Pull `VID_xxxx` / `PID_xxxx` out of a HID device path.
fn parse_vid_pid(path: &str) -> (Option<u16>, Option<u16>) {
    fn field(upper: &str, tag: &str) -> Option<u16> {
        let i = upper.find(tag)? + tag.len();
        u16::from_str_radix(upper.get(i..i + 4)?, 16).ok()
    }
    let upper = path.to_ascii_uppercase();
    (field(&upper, "VID_"), field(&upper, "PID_"))
}

fn device_path(handle: HANDLE) -> Option<String> {
    unsafe {
        let mut len = 0u32;
        GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, null_mut(), &mut len);
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        let n = GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, buf.as_mut_ptr().cast(), &mut len);
        if n == u32::MAX {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..end]))
    }
}

fn run_message_loop(tx: EventSink, ready: mpsc::Sender<io::Result<isize>>) {
    unsafe {
        let hinst = GetModuleHandleW(null());
        let class_name: Vec<u16> = "multikeyboard_rawinput\0".encode_utf16().collect();
        let mut wc: WNDCLASSW = zeroed();
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = hinst;
        wc.lpszClassName = class_name.as_ptr();
        // Ignore failure: the class may already exist from an earlier Listener.
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            0, class_name.as_ptr(), null(), 0, 0, 0, 0, 0,
            HWND_MESSAGE, null_mut(), hinst, null(),
        );
        if hwnd.is_null() {
            let _ = ready.send(Err(io::Error::last_os_error()));
            return;
        }

        let rid = RAWINPUTDEVICE {
            usUsagePage: 0x01, // generic desktop
            usUsage: 0x06,     // keyboard
            dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
            hwndTarget: hwnd,
        };
        if RegisterRawInputDevices(&rid, 1, size_of::<RAWINPUTDEVICE>() as u32) == 0 {
            let _ = ready.send(Err(io::Error::last_os_error()));
            DestroyWindow(hwnd);
            return;
        }

        STATE.with(|s| {
            let mut st = State { tx, by_handle: HashMap::new(), by_path: HashMap::new(), next_id: 0 };
            // Announce keyboards that are already connected.
            for h in enumerate_keyboards() {
                st.id_for(h);
            }
            *s.borrow_mut() = Some(st);
        });
        let _ = ready.send(Ok(hwnd as isize));

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        STATE.with(|s| *s.borrow_mut() = None);
    }
}

fn enumerate_keyboards() -> Vec<HANDLE> {
    unsafe {
        let sz = size_of::<RAWINPUTDEVICELIST>() as u32;
        let mut count = 0u32;
        GetRawInputDeviceList(null_mut(), &mut count, sz);
        let mut list: Vec<RAWINPUTDEVICELIST> = vec![zeroed(); count as usize];
        let got = GetRawInputDeviceList(list.as_mut_ptr(), &mut count, sz);
        if got == u32::MAX {
            return Vec::new();
        }
        list.iter()
            .take(got as usize)
            .filter(|d| d.dwType == RIM_TYPEKEYBOARD)
            .map(|d| d.hDevice)
            .collect()
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_INPUT => {
            handle_raw_input(lparam as HRAWINPUT);
            DefWindowProcW(hwnd, msg, wparam, lparam) // required for system cleanup
        }
        WM_INPUT_DEVICE_CHANGE => {
            let handle = lparam as HANDLE;
            STATE.with(|s| {
                if let Some(st) = s.borrow_mut().as_mut() {
                    if wparam as u32 == GIDC_REMOVAL {
                        // By removal time the handle can't be queried, so
                        // only report devices we've already seen.
                        if let Some(id) = st.by_handle.remove(&(handle as isize)) {
                            st.tx.send(Event::KeyboardRemoved(id));
                        }
                    }
                    // Arrivals are announced lazily on their first key event.
                }
            });
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn handle_raw_input(h: HRAWINPUT) {
    let hdr = size_of::<RAWINPUTHEADER>() as u32;
    let mut size = 0u32;
    if GetRawInputData(h, RID_INPUT, null_mut(), &mut size, hdr) != 0 || size == 0 {
        return;
    }
    // u64 backing store keeps the buffer 8-byte aligned for RAWINPUT.
    let mut buf = vec![0u64; (size as usize).div_ceil(8)];
    if GetRawInputData(h, RID_INPUT, buf.as_mut_ptr().cast(), &mut size, hdr) == u32::MAX {
        return;
    }
    let raw = &*(buf.as_ptr() as *const RAWINPUT);
    if raw.header.dwType != RIM_TYPEKEYBOARD {
        return;
    }
    let kb = raw.data.keyboard;
    // 0xFF make code / vkey marks "fake" keys (overrun, Shift injected by NumLock).
    if kb.MakeCode == 0xFF || kb.VKey == 0xFF {
        return;
    }
    let mut code = kb.MakeCode as u32;
    if kb.Flags as u32 & RI_KEY_E0 != 0 {
        code |= 0xE000;
    } else if kb.Flags as u32 & RI_KEY_E1 != 0 {
        code |= 0xE100;
    }
    let state = if kb.Flags as u32 & RI_KEY_BREAK != 0 {
        KeyState::Released
    } else {
        KeyState::Pressed
    };
    STATE.with(|s| {
        if let Some(st) = s.borrow_mut().as_mut() {
            let keyboard = st.id_for(raw.header.hDevice);
            st.tx.send(Event::Key(KeyEvent {
                keyboard,
                key: from_windows_scancode(code),
                native_code: code,
                state,
            }));
        }
    });
}
