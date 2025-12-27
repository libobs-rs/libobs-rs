//! Provides functionality for working with OBS replay buffers.
//!
//! This module extends the ObsOutputRef to provide replay buffer capabilities.
//! A replay buffer is a special type of output that continuously records
//! the last N seconds of content, allowing the user to save this buffer on demand. This must be configured. More documentation soon.
use std::{
    ffi::c_char,
    mem::MaybeUninit,
    path::{Path, PathBuf},
};

use libobs::calldata_t;

use crate::{
    run_with_obs,
    utils::{calldata_free, ObsError, ObsString},
};

use super::ObsOutputRef;

/// Defines functionality specific to replay buffer outputs.
///
/// This trait provides methods for working with replay buffers in OBS,
/// which are special outputs that continuously record content and allow
/// on-demand saving of recent footage.
pub trait ReplayBufferOutput {
    /// Saves the current replay buffer content to disk.
    ///
    /// This method triggers the replay buffer to save its content to a file
    /// and returns the path to the saved file.
    ///
    /// # Returns
    /// * `Result<Box<Path>, ObsError>` - On success, returns the path to the saved
    ///   replay file. On failure, returns an error describing what went wrong.
    fn save_buffer(&self) -> Result<Box<Path>, ObsError>;
}

/// Implementation of the ReplayBufferOutput trait for ObsOutputRef.
///
/// This implementation allows any ObsOutputRef configured as a replay buffer
/// to save its content to disk via a simple API call.
impl ReplayBufferOutput for ObsOutputRef {
    /// Saves the current replay buffer content to disk.
    ///
    /// # Implementation Details
    /// This method:
    /// 1. Accesses the OBS procedure handler for the output
    /// 2. Calls the "save" procedure to trigger saving the replay
    /// 3. Calls the "get_last_replay" procedure to retrieve the saved file path
    /// 4. Extracts the path string from the calldata and returns it
    ///
    /// # Returns
    /// * `Ok(Box<Path>)` - The path to the saved replay file
    /// * `Err(ObsError)` - Various errors that might occur during the saving process:
    ///   - Failure to get procedure handler
    ///   - Failure to call "save" procedure
    ///   - Failure to call "get_last_replay" procedure
    ///   - Failure to extract the path from calldata
    fn save_buffer(&self) -> Result<Box<Path>, ObsError> {
        let output_ptr = self.output.clone();

        let is_proper_output_type = self.id().to_string() == "replay_buffer";
        if !is_proper_output_type {
            return Err(ObsError::OutputSaveBufferFailure(
                "Output is not a replay buffer output.".to_string(),
            ));
        }

        run_with_obs!(self.runtime, (output_ptr), move || {
            let ph = unsafe { libobs::obs_output_get_proc_handler(output_ptr) };
            if ph.is_null() {
                return Err(ObsError::OutputSaveBufferFailure(
                    "Failed to get proc handler.".to_string(),
                ));
            }

            let name = ObsString::new("save");
            let mut calldata = MaybeUninit::<calldata_t>::zeroed();
            let call_success =
                unsafe { libobs::proc_handler_call(ph, name.as_ptr().0, calldata.as_mut_ptr()) };

            if !call_success {
                return Err(ObsError::OutputSaveBufferFailure(
                    "Failed to call proc handler.".to_string(),
                ));
            }

            unsafe {
                calldata_free(calldata.as_mut_ptr());
            }
            Ok(())
        })??;

        self.signal_manager()
            .on_saved()?
            .blocking_recv()
            .map_err(|_e| {
                ObsError::OutputSaveBufferFailure(
                    "Failed to receive saved replay buffer path.".to_string(),
                )
            })?;

        let path = run_with_obs!(self.runtime, (output_ptr), move || {
            let ph = unsafe { libobs::obs_output_get_proc_handler(output_ptr) };
            if ph.is_null() {
                return Err(ObsError::OutputSaveBufferFailure(
                    "Failed to get proc handler.".to_string(),
                ));
            }

            let func_get = ObsString::new("get_last_replay");
            let mut last_replay_calldata = unsafe {
                let mut calldata = MaybeUninit::<calldata_t>::zeroed();
                let success =
                    libobs::proc_handler_call(ph, func_get.as_ptr().0, calldata.as_mut_ptr());

                if !success {
                    return Err(ObsError::OutputSaveBufferFailure(
                        "Failed to call get_last_replay.".to_string(),
                    ));
                }

                calldata.assume_init()
            };

            let path_get = ObsString::new("path");

            let mut s = MaybeUninit::<*const c_char>::uninit();

            let res = unsafe {
                libobs::calldata_get_string(
                    &last_replay_calldata,
                    path_get.as_ptr().0,
                    s.as_mut_ptr(),
                )
            };
            if !res {
                unsafe { calldata_free(&mut last_replay_calldata) };
                return Err(ObsError::OutputSaveBufferFailure(
                    "Failed to get path from last replay.".to_string(),
                ));
            }

            let s: *const c_char = unsafe { s.assume_init() };
            if s.is_null() {
                unsafe { calldata_free(&mut last_replay_calldata) };
                return Err(ObsError::OutputSaveBufferFailure(
                    "Failed to get path from last replay.".to_string(),
                ));
            }

            let path = unsafe { std::ffi::CStr::from_ptr(s) }
                .to_str()
                .map_err(|_e| {
                    ObsError::OutputSaveBufferFailure(
                        "Failed to convert path CStr to str.".to_string(),
                    )
                });

            if let Err(e) = path {
                unsafe { calldata_free(&mut last_replay_calldata) };
                return Err(e);
            }

            unsafe { calldata_free(&mut last_replay_calldata) };
            Ok(PathBuf::from(path.unwrap()))
        })??;

        Ok(path.into_boxed_path())
    }
}
