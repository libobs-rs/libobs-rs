//! Provides functionality for working with OBS replay buffers.
//!
//! This module extends the ObsOutputRef to provide replay buffer capabilities.
//! A replay buffer is a special type of output that continuously records
//! the last N seconds of content, allowing the user to save this buffer on demand. This must be configured. More documentation soon.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use libobs::obs_output;

use crate::{
    data::{
        output::{
            ObsOutputRef, ObsOutputSignals, ObsOutputTrait, ObsOutputTraitSealed,
            ReplayBufferOutput,
        },
        ObsData,
    },
    encoders::{audio::ObsAudioEncoder, video::ObsVideoEncoder},
    impl_signal_manager, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::Sendable,
    utils::{ObsCalldataExt, ObsError, ObsString, OutputInfo},
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
pub struct ObsReplayBufferOutputRef {
    /// Disconnect signals first
    pub(crate) replay_signal_manager: Arc<ObsReplayOutputSignals>,

    pub(crate) output: ObsOutputRef,
}

impl ObsOutputTraitSealed for ObsReplayBufferOutputRef {
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

impl ObsReplayBufferOutputRef {
    pub fn replay_signals(&self) -> &Arc<ObsReplayOutputSignals> {
        &self.replay_signal_manager
    }
}

impl ObsOutputTrait for ObsReplayBufferOutputRef {
    fn name(&self) -> ObsString {
        self.output.name.clone()
    }

    fn id(&self) -> ObsString {
        self.output.id.clone()
    }

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
impl ReplayBufferOutput for ObsReplayBufferOutputRef {
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
        log::trace!("Saving replay buffer...");
        let output_ptr = self.as_ptr();

        log::trace!("Getting procedure handler for replay buffer output...");
        let proc_handler = run_with_obs!(self.runtime().clone(), (output_ptr), move || {
            let ph = unsafe { libobs::obs_output_get_proc_handler(output_ptr) };
            if ph.is_null() {
                return Err(ObsError::OutputSaveBufferFailure(
                    "Failed to get proc handler.".to_string(),
                ));
            }
            Ok(Sendable(ph))
        })??;

        log::trace!("Calling 'save' procedure on replay buffer output...");
        self.runtime().call_proc_handler(&proc_handler, "save")?;

        log::trace!("Waiting for 'saved' signal from replay buffer output...");
        self.replay_signals()
            .on_saved()?
            .blocking_recv()
            .map_err(|_e| {
                ObsError::OutputSaveBufferFailure(
                    "Failed to receive saved replay buffer path.".to_string(),
                )
            })?;

        log::trace!("Retrieving last replay path from replay buffer output...");
        let mut calldata = self
            .runtime()
            .call_proc_handler(&proc_handler, "get_last_replay")?;

        log::trace!("Extracting path from calldata...");
        let path = calldata.get_string("path")?;
        let path = PathBuf::from(path);

        Ok(path.into_boxed_path())
    }
}
