#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(super) use linux::*;

#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(not(target_os = "linux"))]
pub(super) use unsupported::*;
