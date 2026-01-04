#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub(crate) use windows::*;

#[cfg(not(windows))]
mod other;

#[cfg(not(windows))]
pub use other::PlatformType;
#[cfg(not(windows))]
pub(crate) use other::*;

use crate::unsafe_send::Sendable;

#[derive(Debug, Clone)]
pub enum NixDisplay {
    X11(Sendable<*mut std::os::raw::c_void>),
    Wayland(Sendable<*mut std::os::raw::c_void>),
}
