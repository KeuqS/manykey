#![doc = include_str!("../README.md")]

mod backend;
mod key;
mod queue;

pub mod capi;
pub mod fake;

pub use key::{from_evdev, from_windows_scancode, Key};
pub use queue::EventSink;

use std::io;
use std::sync::Arc;
use std::time::Duration;

/// Default event queue capacity.
pub const DEFAULT_CAPACITY: usize = 4096;

/// Which backend reads the keyboards: the piece that talks to one operating
/// system mechanism. See the README's "Backends" section for what each needs
/// and costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum BackendKind {
    /// The best available backend for this operating system.
    #[default]
    Auto,
    /// Windows Raw Input.
    RawInput,
    /// Linux evdev (`/dev/input`).
    Evdev,
}

impl BackendKind {
    /// Stable lowercase name: `auto`, `raw-input`, `evdev`.
    pub fn name(self) -> &'static str {
        match self {
            BackendKind::Auto => "auto",
            BackendKind::RawInput => "raw-input",
            BackendKind::Evdev => "evdev",
        }
    }

    pub fn from_name(name: &str) -> Option<BackendKind> {
        [BackendKind::Auto, BackendKind::RawInput, BackendKind::Evdev].into_iter().find(|k| k.name() == name)
    }
}

/// Whose keystrokes the listener reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Scope {
    /// Every keystroke on every keyboard, whichever program has focus. This is
    /// what a background tool (macro helper, overlay) needs.
    #[default]
    Global,
    /// Only while a window of **this process** is the foreground window; events
    /// are dropped otherwise, and keys still held when focus is lost are
    /// released first. What a game or app that only cares about its own input
    /// wants.
    ///
    /// This is a courtesy, not a security boundary: the OS still lets the
    /// process observe input while unfocused; the library just doesn't pass it
    /// on. Currently Windows only; other platforms return an error.
    OwnProcessFocused,
}

/// Settings for [`Listener::with_options`]. `Options::default()` is what
/// [`Listener::new`] uses.
#[derive(Clone, Debug)]
pub struct Options {
    /// Which backend to use. Default: [`BackendKind::Auto`]. Naming a backend
    /// that isn't available on this platform is an error.
    pub backend: BackendKind,
    /// Events the queue holds before discarding the oldest (see [`Event::Overflow`]).
    pub capacity: usize,
    /// Whose keystrokes to report. Default: [`Scope::Global`].
    pub scope: Scope,
    /// Windows: if another component of this process (a windowing library, a
    /// game engine) takes over raw keyboard input registration, take it back
    /// (checked twice a second). Off by default, because it overrides the host
    /// and may break whatever relied on that registration; turn it on if you
    /// see no events inside a program that has its own window. Ignored on
    /// other platforms.
    pub reclaim_registration: bool,
    /// Windows: also report input injected by software (`SendInput`,
    /// AutoHotkey, remote-desktop and accessibility tools), which has no
    /// physical device and shows up as one extra pseudo-keyboard. Off by
    /// default so `KeyboardAdded` means a real device.
    pub include_injected: bool,
    /// When a keyboard is unplugged while holding keys, emit a `Released` for
    /// each of them just before `KeyboardRemoved`, so your held-key state stays
    /// correct. Turn it off if you'd rather handle that yourself, or if you
    /// need every event you receive to come from the OS. Default: `true`.
    pub release_on_unplug: bool,
    /// With [`Scope::OwnProcessFocused`]: when your process loses focus, emit a
    /// `Released` for every key still held, so nothing is left stuck. If you
    /// turn this off, keys held at that moment never get a release from us
    /// (their real releases are dropped while you're unfocused). Has no effect
    /// under `Scope::Global`. Default: `true`.
    pub release_on_focus_loss: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            backend: BackendKind::Auto,
            release_on_unplug: true,
            release_on_focus_loss: true,
            capacity: DEFAULT_CAPACITY,
            scope: Scope::Global,
            reclaim_registration: false,
            include_injected: false,
        }
    }
}

/// Opaque keyboard identifier, stable for the lifetime of the process.
///
/// If the same device is unplugged and replugged it usually gets the same id
/// back (matched by OS device identity), but ids are not persisted across runs.
/// [`KeyboardInfo`] gives you what the OS reports so you can *attempt* to
/// recognise a keyboard across runs; identical keyboards can report identical
/// names and IDs, and `location` is not available everywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyboardId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyboardInfo {
    pub id: KeyboardId,
    /// Human-readable name where the OS provides one (Linux), otherwise the
    /// device path (Windows).
    pub name: String,
    /// OS device path, e.g. `\\?\HID#VID_...` or `/dev/input/event3`.
    pub path: String,
    /// USB vendor/product IDs when known. Two identical keyboards share these;
    /// tell them apart with `location`.
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    /// Where the device is attached, when the OS says (Linux `phys`).
    pub location: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyState {
    Pressed,
    Released,
    /// Auto-repeat. **Platform difference:** reported only on Linux. On Windows
    /// a held key's repeats arrive as further `Pressed` events, so code that
    /// must tell a fresh press from a repeat should track held keys itself.
    Repeat,
}

/// Where a [`KeyEvent`] came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum EventOrigin {
    /// The operating system reported this key event.
    #[default]
    Native,
    /// manykey generated this event to keep your view of the keyboards
    /// consistent (a release for a key still held when its keyboard was
    /// unplugged, or when focus was lost). No physical key action happened at
    /// this moment. See `Options::release_on_unplug` and
    /// `Options::release_on_focus_loss`.
    Synthesized,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub keyboard: KeyboardId,
    /// Physical key; `Key::UNKNOWN` if this backend has no mapping for it.
    pub key: Key,
    /// Platform-native scancode, kept so unmapped keys aren't lost.
    /// Windows: set-1 make code with `0xE000` OR'd in for extended keys.
    /// Linux: evdev `KEY_*` code.
    pub native_code: u32,
    pub state: KeyState,
    /// Whether the OS reported this or manykey generated it.
    pub origin: EventOrigin,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    KeyboardAdded(KeyboardInfo),
    KeyboardRemoved(KeyboardId),
    Key(KeyEvent),
    /// The queue overflowed and events were discarded before this point.
    ///
    /// **Consumers that track held keys must treat this as "my key state may
    /// be wrong"**: clear it (a lost release would otherwise leave a key
    /// stuck down). `dropped` is the cumulative discard count. Releases are
    /// the last key events to be discarded, so this is rare in practice.
    Overflow {
        dropped: u64,
    },
}

/// A platform input source. Dropping it stops event delivery.
pub trait Backend: Send {}

pub struct Listener {
    pub(crate) shared: Arc<queue::Shared>,
    pub(crate) _backend: Box<dyn Backend>,
}

impl Listener {
    /// Start listening with the default backend for this OS and default
    /// [`Options`].
    pub fn new() -> io::Result<Self> {
        Self::with_options(Options::default())
    }

    /// Start listening with the default backend for this OS.
    pub fn with_options(options: Options) -> io::Result<Self> {
        Self::with_backend(options.capacity, move |sink| backend::start_default(sink, options))
    }

    /// Use a custom backend. `start` receives the sink the backend feeds.
    pub fn with_backend(
        capacity: usize,
        start: impl FnOnce(EventSink) -> io::Result<Box<dyn Backend>>,
    ) -> io::Result<Self> {
        let shared = queue::Shared::new(capacity)?;
        let backend = start(EventSink(shared.clone()))?;
        Ok(Self::from_shared(shared, backend))
    }

    /// Next event if one is ready.
    pub fn try_recv(&self) -> Option<Event> {
        self.shared.try_pop()
    }

    /// Block until an event arrives.
    pub fn recv(&self) -> Event {
        self.shared.pop_timeout(None).expect("untimed wait returned no event")
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Option<Event> {
        self.shared.pop_timeout(Some(timeout))
    }

    /// Blocking iterator over events (never ends).
    pub fn iter(&self) -> impl Iterator<Item = Event> + '_ {
        std::iter::repeat_with(|| self.recv())
    }

    /// An OS handle that is readable/signalled whenever events are pending, for
    /// integrating with a host event loop. Linux: an eventfd (use with
    /// `poll`/`epoll`; don't read it yourself). Windows: an event `HANDLE` for
    /// `WaitForMultipleObjects`. -1 on platforms without one.
    pub fn wait_handle(&self) -> isize {
        self.shared.raw_handle()
    }

    /// Key/other events discarded because the queue was full.
    pub fn dropped_events(&self) -> u64 {
        self.shared.dropped()
    }
}
