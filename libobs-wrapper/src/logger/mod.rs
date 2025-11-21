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
    pub static ref LOGGER: Mutex<Box<dyn ObsLogger>> = Mutex::new(Box::new(ConsoleLogger::new()));
}

pub(crate) unsafe extern "C" fn extern_log_callback<V>(
    log_level: i32,
    msg: *const i8,
    args: *mut V,
    _params: *mut c_void,
) {
    let level = ObsLogLevel::from_i32(log_level);
    if level.is_none() {
        eprintln!("Couldn't find log level {}", log_level);
        return;
    }

    let level = level.unwrap();

    let formatted = vsprintf(msg, args);
    if formatted.is_err() {
        eprintln!("Failed to format log message");
        return;
    }

    let mut logger = LOGGER.lock().unwrap();

    logger.log(level, formatted.unwrap());
}

pub trait ObsLogger
where
    Self: Send + Debug,
{
    fn log(&mut self, level: ObsLogLevel, msg: String);
}

pub(crate) fn internal_log_global(level: ObsLogLevel, msg: String) {
    let mut logger = LOGGER.lock().unwrap();
    logger.log(level, msg);
}
