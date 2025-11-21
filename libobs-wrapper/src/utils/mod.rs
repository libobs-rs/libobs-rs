mod error;
mod info;
pub(crate) mod initialization;
mod obs_string;
mod path;
pub mod traits;

#[cfg(target_os = "linux")]
pub(crate) mod linux;

#[cfg(test)]
mod obs_string_tests;

#[cfg(test)]
mod path_tests;

mod modules;

pub use error::*;
pub use info::*;
pub use modules::ObsModules;
pub use obs_string::*;
pub use path::*;

pub const ENCODER_HIDE_FLAGS: u32 =
    libobs::OBS_ENCODER_CAP_DEPRECATED | libobs::OBS_ENCODER_CAP_INTERNAL;
