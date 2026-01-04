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

use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, RwLock},
};

pub use calldata::*;
pub use error::*;
pub use info::*;
pub use initialization::NixDisplay;
#[cfg(target_os = "linux")]
pub use initialization::PlatformType;
pub use modules::ObsModules;
pub use obs_string::*;
pub use path::*;

pub const ENCODER_HIDE_FLAGS: u32 =
    libobs::OBS_ENCODER_CAP_DEPRECATED | libobs::OBS_ENCODER_CAP_INTERNAL;

/// Internal function to free calldata structs, same implementation as libobs
///
/// # Safety
/// Only call this function with a valid calldata pointer and ensure that
/// this function runs within the OBS Runtime.
#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
pub(crate) unsafe fn calldata_free(data: *mut libobs::calldata_t) {
    if !(*data).fixed {
        libobs::bfree((*data).stack as *mut _);
    }
}

/// This should be implemented for any struct that releases OBS resources when dropped
pub trait ObsDropGuard: Debug {}

pub(crate) type GeneralTraitHashMap<T, K> = Arc<RwLock<HashMap<Arc<Box<T>>, K>>>;
