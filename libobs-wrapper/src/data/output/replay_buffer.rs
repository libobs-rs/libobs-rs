//! Provides functionality for working with OBS replay buffers.
//!
//! This module extends the ObsOutputRef to provide replay buffer capabilities.
//! A replay buffer is a special type of output that continuously records
//! the last N seconds of content, allowing the user to save this buffer on demand. This must be configured. More documentation soon.
use std::{
    collections::HashMap,
    ffi::c_char,
    mem::MaybeUninit,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use libobs::{calldata_t, obs_output};

use crate::{
    data::{
        ObsData, output::{ObsOutputRef, ObsOutputSignals, ObsOutputTrait, ObsOutputTraitSealed, ReplayBufferOutput}
    },
    encoders::{audio::ObsAudioEncoder, video::ObsVideoEncoder},
    impl_signal_manager, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::Sendable,
    utils::{ObsError, ObsString, OutputInfo, calldata_free},
};

#[derive(Debug, Clone)]
/// A reference to an OBS output.
///
/// This struct represents an output in OBS, which is responsible for
/// outputting encoded audio and video data to a destination such as:
/// - A file (recording)
/// - A streaming service (RTMP, etc.)
/// - A replay buffer
///
/// The output is associated with video and audio encoders that convert
/// raw media to the required format before sending/storing.
pub struct ObsReplayOutputRef {
    /// Disconnect signals first
    pub(crate) replay_signal_manager: Arc<ObsReplayOutputSignals>,

    pub(crate) output: ObsOutputRef,
}

impl ObsOutputTraitSealed for ObsReplayOutputRef {
    fn new(mut output: OutputInfo, runtime: ObsRuntime) -> Result<Self, ObsError> {
        output.id = ObsString::new("replay_buffer");
        let output = ObsOutputRef::new(output, runtime.clone())?;

        let replay_signal_manager = ObsReplayOutputSignals::new(&output.as_ptr(), runtime)?;
        Ok(Self {
            replay_signal_manager: Arc::new(replay_signal_manager),
            output,
        })
    }
}

impl ObsOutputTrait for ObsReplayOutputRef {
    fn runtime(&self) -> &ObsRuntime {
        &self.output.runtime
    }

    fn signal_manager(&self) -> &Arc<ObsOutputSignals> {
        &self.output.signal_manager
    }

    fn settings(&self) -> &Arc<RwLock<Option<ObsData>>> {
        &self.output.settings
    }

    fn hotkey_data(&self) -> &Arc<RwLock<Option<ObsData>>> {
        &self.output.hotkey_data
    }

    fn video_encoder(&self) -> &Arc<RwLock<Option<Arc<ObsVideoEncoder>>>> {
        &self.output.curr_video_encoder
    }

    fn audio_encoders(&self) -> &Arc<RwLock<HashMap<usize, Arc<ObsAudioEncoder>>>> {
        &self.output.audio_encoders
    }

    fn as_ptr(&self) -> Sendable<*mut obs_output> {
        self.output.output.clone()
    }
}

impl_signal_manager!(|ptr| unsafe { libobs::obs_output_get_signal_handler(ptr) }, ObsReplayOutputSignals for ObsReplayOutputRef<*mut libobs::obs_output>, [
    "saved": {}
]);

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
        let output_ptr = self.as_ptr();

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
