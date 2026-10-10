//! Windows backend: Raw Input on a hidden message-only window.
//!
//! `RIDEV_INPUTSINK` delivers input even when our process isn't focused, and
//! `RIDEV_DEVNOTIFY` gives hot-plug notifications.

use crate::{from_windows_scancode, Backend, Event, EventOrigin, EventSink, Key, KeyEvent, KeyState, KeyboardId, KeyboardInfo, Options, Scope};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use windows_sys::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows_sys::Win32::UI::Input::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub struct WindowsBackend {
    hwnd: isize,
    thread: Option<JoinHandle<()>>,
}

impl Backend for WindowsBackend {}

impl WindowsBackend {
    pub fn start(tx: EventSink, options: Options) -> io::Result<Self> {
        let (ready_tx, ready_rx) = mpsc::channel::<io::Result<isize>>();
        let thread = thread::Builder::new()
            .name("manykey-rawinput".into())
            .spawn(move || run_message_loop(tx, options, ready_tx))?;
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
    scope: Scope,
    include_injected: bool,
    release_on_unplug: bool,
    release_on_focus_loss: bool,
    /// For `Scope::OwnProcessFocused`: does a window of this process have the
    /// foreground right now? Always true under `Scope::Global`.
    focused: bool,
    /// Keys we've reported as pressed and not yet released, so that unplugging
    /// a keyboard mid-keypress can release them. (Pure bookkeeping of events
    /// we already saw: nothing here asks the OS about key state.)
    held: HashMap<(KeyboardId, u32), Key>,
}

impl State {
    fn track(&mut self, ev: &KeyEvent) {
        let k = (ev.keyboard, ev.native_code);
        match ev.state {
            KeyState::Released => {
                self.held.remove(&k);
            }
            _ => {
                self.held.insert(k, ev.key);
            }
        }
    }

    /// Foreground window changed. Losing focus releases whatever was held, so
    /// the consumer isn't left with keys that went up while it wasn't looking.
    fn set_focused(&mut self, focused: bool) {
        if self.scope != Scope::OwnProcessFocused || focused == self.focused {
            return;
        }
        if !focused && self.release_on_focus_loss {
            let held: Vec<_> = self.held.iter().map(|(k, v)| (*k, *v)).collect();
            for ((kb, native), key) in held {
                self.release(kb, native, key);
            }
        }
        self.focused = focused;
    }

    fn release(&mut self, keyboard: KeyboardId, native_code: u32, key: Key) {
        self.held.remove(&(keyboard, native_code));
        self.tx.send(Event::Key(KeyEvent { keyboard, key, native_code, state: KeyState::Released, origin: EventOrigin::Synthesized }));
    }
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
        let name = friendly_name(handle, &path);
        self.tx.send(Event::KeyboardAdded(KeyboardInfo {
            id,
            name,
            path,
            vendor_id,
            product_id,
            location: None,
        }));
        id
    }
}

/// Best human-readable name we can get: the HID product string, else a label
/// built from the device path's bus and hardware id.
fn friendly_name(handle: HANDLE, path: &str) -> String {
    if handle.is_null() {
        // SendInput() and tools like AutoHotkey inject input with no device.
        return "Injected input (no physical device)".to_string();
    }
    if let Some(n) = hid_product_string(path) {
        return n;
    }
    if let Some(n) = registry_device_name(path) {
        return n;
    }
    // "\\?\ACPI#MSFT0001#..." -> "Keyboard (ACPI MSFT0001)"
    let mut parts = path.trim_start_matches(r"\\?\").split('#');
    match (parts.next(), parts.next()) {
        (Some(bus), Some(hw)) if !bus.is_empty() => format!("Keyboard ({bus} {hw})"),
        _ => "Keyboard".to_string(),
    }
}

/// Windows' own name for the device (what Device Manager shows), from
/// `HKLM\\SYSTEM\\CurrentControlSet\\Enum\\<bus>\\<hwid>\\<instance>`.
fn registry_device_name(path: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    // `\\?\ACPI#MSFT0001#4&10e4b83d&0#{guid}` -> `ACPI\MSFT0001\4&10e4b83d&0`
    let trimmed = path.trim_start_matches(r"\\?\");
    let instance = trimmed.rsplit_once("#{").map_or(trimmed, |(a, _)| a);
    let subkey: Vec<u16> = format!("SYSTEM\\CurrentControlSet\\Enum\\{}", instance.replace('#', "\\"))
        .encode_utf16()
        .chain(Some(0))
        .collect();
    for value in ["FriendlyName", "DeviceDesc"] {
        let vname: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
        let mut buf = [0u16; 256];
        let mut bytes = (buf.len() * 2) as u32;
        let rc = unsafe {
            RegGetValueW(HKEY_LOCAL_MACHINE, subkey.as_ptr(), vname.as_ptr(), RRF_RT_REG_SZ, null_mut(), buf.as_mut_ptr().cast(), &mut bytes)
        };
        if rc != 0 {
            continue;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let s = String::from_utf16_lossy(&buf[..end]);
        // DeviceDesc looks like "@keyboard.inf,%pnp0303.devicedesc%;Standard PS/2 Keyboard".
        let s = s.rsplit(';').next().unwrap_or(&s).trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

fn hid_product_string(path: &str) -> Option<String> {
    use windows_sys::Win32::Devices::HumanInterfaceDevice::HidD_GetProductString;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    unsafe {
        // Zero access rights are enough to query descriptors, and avoid
        // conflicting with whoever else has the keyboard open.
        let h = CreateFileW(wide.as_ptr(), 0, FILE_SHARE_READ | FILE_SHARE_WRITE, null(), OPEN_EXISTING, 0, null_mut());
        if h == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut buf = [0u16; 128];
        let ok = HidD_GetProductString(h, buf.as_mut_ptr().cast(), (buf.len() * 2) as u32);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let s = String::from_utf16_lossy(&buf[..end]).trim().to_string();
        (!s.is_empty()).then_some(s)
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

fn run_message_loop(tx: EventSink, options: Options, ready: mpsc::Sender<io::Result<isize>>) {
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

        if !register_keyboards(hwnd) {
            let _ = ready.send(Err(io::Error::last_os_error()));
            DestroyWindow(hwnd);
            return;
        }

        let focused_now = options.scope == Scope::Global || foreground_is_ours(GetForegroundWindow());
        STATE.with(|s| {
            let mut st = State {
                tx,
                by_handle: HashMap::new(),
                by_path: HashMap::new(),
                next_id: 0,
                scope: options.scope,
                include_injected: options.include_injected,
                release_on_unplug: options.release_on_unplug,
                release_on_focus_loss: options.release_on_focus_loss,
                focused: focused_now,
                held: HashMap::new(),
            };
            // Announce keyboards that are already connected.
            for h in enumerate_keyboards() {
                st.id_for(h);
            }
            *s.borrow_mut() = Some(st);
        });
        // Raw input registration is per process per device class, so another
        // library in this process (e.g. winit) can silently take it over.
        // Only if the caller opted in, re-check periodically and take it back.
        if options.reclaim_registration {
            SetTimer(hwnd, REASSERT_TIMER, 500, None);
        }
        // Focus changes arrive as events (no polling), and only when asked for.
        let focus_hook = if options.scope == Scope::OwnProcessFocused {
            Some(SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                null_mut(),
                Some(foreground_changed),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            ))
        } else {
            None
        };
        let _ = ready.send(Ok(hwnd as isize));

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if let Some(h) = focus_hook {
            UnhookWinEvent(h);
        }
        STATE.with(|s| *s.borrow_mut() = None);
    }
}

fn foreground_is_ours(hwnd: HWND) -> bool {
    if hwnd.is_null() {
        return false;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    pid == unsafe { GetCurrentProcessId() }
}

unsafe extern "system" fn foreground_changed(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    let ours = foreground_is_ours(hwnd);
    STATE.with(|s| {
        if let Some(st) = s.borrow_mut().as_mut() {
            st.set_focused(ours);
        }
    });
}

const REASSERT_TIMER: usize = 1;

unsafe fn register_keyboards(hwnd: HWND) -> bool {
    let rid = RAWINPUTDEVICE {
        usUsagePage: 0x01, // generic desktop
        usUsage: 0x06,     // keyboard
        dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
        hwndTarget: hwnd,
    };
    RegisterRawInputDevices(&rid, 1, size_of::<RAWINPUTDEVICE>() as u32) != 0
}

/// Is `hwnd` still this process's registered target for keyboard raw input?
unsafe fn still_registered(hwnd: HWND) -> bool {
    let sz = size_of::<RAWINPUTDEVICE>() as u32;
    let mut n = 0u32;
    GetRegisteredRawInputDevices(null_mut(), &mut n, sz);
    if n == 0 {
        return false;
    }
    let mut list: Vec<RAWINPUTDEVICE> = vec![zeroed(); n as usize];
    if GetRegisteredRawInputDevices(list.as_mut_ptr(), &mut n, sz) == u32::MAX {
        return true; // can't tell; don't thrash
    }
    list.iter().take(n as usize).any(|d| d.usUsagePage == 1 && d.usUsage == 6 && d.hwndTarget == hwnd)
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
                            // Unplugged mid-keypress: release what it was holding
                            // (if the caller wants that), and forget it either way.
                            let held: Vec<_> = st.held.iter().filter(|((kb, _), _)| *kb == id).map(|(k, v)| (*k, *v)).collect();
                            for ((kb, native), key) in held {
                                if st.release_on_unplug {
                                    st.release(kb, native, key);
                                } else {
                                    st.held.remove(&(kb, native));
                                }
                            }
                            st.tx.send(Event::KeyboardRemoved(id));
                        }
                    }
                    // Arrivals are announced lazily on their first key event.
                }
            });
            0
        }
        WM_TIMER if wparam == REASSERT_TIMER => {
            if !still_registered(hwnd) {
                register_keyboards(hwnd);
            }
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
            if !st.focused || (raw.header.hDevice.is_null() && !st.include_injected) {
                return;
            }
            let keyboard = st.id_for(raw.header.hDevice);
            let ev = KeyEvent { keyboard, key: from_windows_scancode(code), native_code: code, state, origin: EventOrigin::Native };
            st.track(&ev);
            st.tx.send(Event::Key(ev));
        }
    });
}
