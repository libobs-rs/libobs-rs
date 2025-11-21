#[cfg(windows)]
pub(crate) mod windows;

#[cfg(target_os = "linux")]
pub(crate) mod linux;

mod traits;
pub use traits::*;
