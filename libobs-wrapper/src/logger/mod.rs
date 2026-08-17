//! This module is used to provide a logger trait, which can be used to implement custom logging
//! for the libobs console output.

mod console;
mod file;
pub use console::ConsoleLogger;
pub use file::FileLogger;

use std::{fmt::Debug, os::raw::c_void, sync::Mutex};

use lazy_static::lazy_static;
use num_traits::FromPrimitive;
use vsprintf::vsprintf;

use crate::enums::ObsLogLevel;

lazy_static! {
    /// We are using this as global variable because there can only be one obs context
    pub(crate) static ref LOGGER: Mutex<Box<dyn ObsLogger>> = Mutex::new(Box::new(ConsoleLogger::new()));
}

/// # Safety
/// This function is unsafe because it is called from C code.
pub(crate) unsafe extern "C" fn extern_log_callback<V>(
    log_level: i32,
    msg: *const i8,
    args: *mut V,
    _params: *mut c_void,
) {
    let Some(level) = ObsLogLevel::from_i32(log_level) else {
        eprintln!("Couldn't find log level {}", log_level);
        return;
    };

    let Ok(formatted) = vsprintf(msg, args) else {
        eprintln!("Failed to format log message");
        return;
    };

    let mut logger = LOGGER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    logger.log(level, formatted);
}

pub trait ObsLogger
where
    Self: Send + Debug,
{
    fn log(&mut self, level: ObsLogLevel, msg: String);
}

pub(crate) fn internal_log_global(level: ObsLogLevel, msg: String) {
    let mut logger = LOGGER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    logger.log(level, msg);
}
