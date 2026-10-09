use crate::{Backend, EventSink};
use std::io;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

pub(crate) fn start_default(sink: EventSink) -> io::Result<Box<dyn Backend>> {
    #[cfg(windows)]
    return Ok(Box::new(windows::WindowsBackend::start(sink)?));
    #[cfg(target_os = "linux")]
    return Ok(Box::new(linux::LinuxBackend::start(sink)?));
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = sink;
        Err(io::Error::new(io::ErrorKind::Unsupported, "no backend for this OS"))
    }
}
