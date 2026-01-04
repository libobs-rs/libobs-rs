use std::{ffi::c_void, sync::Mutex};

use lazy_static::lazy_static;

#[cfg(feature = "dialog_crash_handler")]
pub mod dialog;

/// Trait for handling OBS crashes.
/// This is called whenever OBS encounters a fatal error and crashes.
/// Implementors can define custom behavior for crash handling,
/// such as logging the error, showing a dialog, or sending reports.
///
/// **MAKE SURE** that the `handle_crash` function does the least amount of work possible,
/// as it is called in a crash context where many resources may be unavailable.
pub trait ObsCrashHandler: Send {
    /// Handles an OBS crash with the given message.
    /// YOU MUST MAKE SURE that this function does the least amount of work possible!
    fn handle_crash(&self, message: String);
}

pub struct ConsoleCrashHandler {
    _private: (),
}

impl Default for ConsoleCrashHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleCrashHandler {
    pub fn new() -> Self {
        Self { _private: () }
    }
}
impl ObsCrashHandler for ConsoleCrashHandler {
    fn handle_crash(&self, message: String) {
        #[cfg(not(feature = "logging_crash_handler"))]
        eprintln!("OBS crashed: {}", message);
        #[cfg(feature = "logging_crash_handler")]
        log::error!("OBS crashed: {}", message);
    }
}

lazy_static! {
    /// We are using this as global variable because there can only be one obs context
    pub static ref CRASH_HANDLER: Mutex<Box<dyn ObsCrashHandler>> = {
        #[cfg(feature="dialog_crash_handler")]
        {
            Mutex::new(Box::new(dialog::DialogCrashHandler::new()))
        }
        #[cfg(not(feature="dialog_crash_handler"))]
        {
            Mutex::new(Box::new(ConsoleCrashHandler::new()))
        }
    };
}

/// # Safety
/// This function is unsafe because it is called from C code in a crash context.
/// You MUST ensure that the function does the least amount of work possible.
pub(crate) unsafe extern "C" fn main_crash_handler<V>(
    format: *const std::os::raw::c_char,
    args: *mut V,
    _params: *mut c_void,
) {
    let res = vsprintf::vsprintf(format, args);
    if res.is_err() {
        eprintln!("Failed to format crash handler message");
        return;
    }

    let res = res.unwrap();
    CRASH_HANDLER.lock().unwrap().handle_crash(res);
}
