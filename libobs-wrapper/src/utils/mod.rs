mod error;
mod info;
pub(crate) mod initialization;
mod obs_string;
mod path;

#[cfg(target_os = "linux")]
pub(crate) mod linux;

#[cfg(test)]
mod obs_string_tests;

#[cfg(test)]
mod path_tests;

mod modules;

mod calldata;

pub use calldata::*;
pub use error::*;
pub use info::*;
pub use initialization::NixDisplay;
pub use modules::ObsModules;
pub use obs_string::*;
pub use path::*;

pub const ENCODER_HIDE_FLAGS: u32 =
    libobs::OBS_ENCODER_CAP_DEPRECATED | libobs::OBS_ENCODER_CAP_INTERNAL;

/// Internal function to free calldata structs, same implementation as libobs
pub(crate) unsafe fn calldata_free(data: *mut libobs::calldata_t) {
    if !(*data).fixed {
        libobs::bfree((*data).stack as *mut _);
    }
}
