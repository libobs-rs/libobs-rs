//! Provides functionality for working with OBS replay buffers.
//!
//! This module extends the ObsOutputRef to provide replay buffer capabilities.
//! A replay buffer is a special type of output that continuously records
//! the last N seconds of content, allowing the user to save this buffer on demand. This must be configured. More documentation soon.
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    data::{
        object::ObsObjectTrait,
        output::{ObsOutputRef, ObsOutputTraitSealed},
    },
    forward_obs_object_impl, forward_obs_output_impl, impl_signal_manager, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::{Sendable, SmartPointerSendable},
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

forward_obs_object_impl!(ObsReplayBufferOutputRef, output, *mut libobs::obs_output);
forward_obs_output_impl!(ObsReplayBufferOutputRef, output);

impl_signal_manager!(|ptr: SmartPointerSendable<*mut libobs::obs_output>| {
    unsafe {
        // Safety: Again, it carries a reference of the drop guard so we must have a valid pointer
        libobs::obs_output_get_signal_handler(ptr.get_ptr())
    }
}, ObsReplayOutputSignals for ObsReplayOutputRef<*mut libobs::obs_output>, [
    "saved": {}
]);

impl ObsReplayBufferOutputRef {
    pub fn replay_signals(&self) -> &Arc<ObsReplayOutputSignals> {
        &self.replay_signal_manager
    }
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
    pub fn save_buffer(&self) -> Result<Box<Path>, ObsError> {
        log::trace!("Saving replay buffer...");
        let output_ptr = self.as_ptr();

        log::trace!("Getting procedure handler for replay buffer output...");
        let proc_handler = run_with_obs!(self.runtime().clone(), (output_ptr), move || {
            // Safety: At this point, output_ptr MUST be a valid pointer as we haven't released the output yet.
            let ph = unsafe { libobs::obs_output_get_proc_handler(output_ptr.get_ptr()) };
            if ph.is_null() {
                return Err(ObsError::OutputSaveBufferFailure(
                    "Failed to get proc handler.".to_string(),
                ));
            }
            Ok(Sendable(ph))
        })??;

        log::trace!("Calling 'save' procedure on replay buffer output...");
        // Safety: we know that the proc handler is valid because we got it from OBS earlier
        unsafe { self.runtime().call_proc_handler(&proc_handler, "save")? };

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
        // Safety: We know that the proc handler is valid because we got it from OBS earlier
        let mut calldata = unsafe {
            self.runtime()
                .call_proc_handler(&proc_handler, "get_last_replay")?
        };

        log::trace!("Extracting path from calldata...");
        let path = calldata.get_string("path")?;
        let path = PathBuf::from(path);

        Ok(path.into_boxed_path())
    }
}
