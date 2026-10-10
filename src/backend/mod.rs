use crate::{Backend, BackendKind, EventSink, Options};
use std::io;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

/// What `BackendKind::Auto` means on this platform.
fn platform_default() -> Option<BackendKind> {
    if cfg!(windows) {
        Some(BackendKind::RawInput)
    } else if cfg!(target_os = "linux") {
        Some(BackendKind::Evdev)
    } else {
        None
    }
}

fn unavailable(kind: BackendKind) -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, format!("the {} backend is not available on this platform", kind.name()))
}

pub(crate) fn start_default(sink: EventSink, options: Options) -> io::Result<Box<dyn Backend>> {
    let kind = match options.backend {
        BackendKind::Auto => {
            platform_default().ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "no backend for this OS"))?
        }
        explicit => explicit,
    };
    match kind {
        #[cfg(windows)]
        BackendKind::RawInput => Ok(Box::new(windows::WindowsBackend::start(sink, options)?)),
        #[cfg(target_os = "linux")]
        BackendKind::Evdev => Ok(Box::new(linux::LinuxBackend::start(sink, &options)?)),
        other => {
            let _ = (sink, options);
            Err(unavailable(other))
        }
    }
}
