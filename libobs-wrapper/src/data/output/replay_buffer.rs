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
/// This struct is used specifically for the replay buffer to manage saving the buffer to a file
/// and configuring special settings, which are specific to the replay buffer
///
/// The output is associated with video and audio encoders that convert
/// raw media to the required format before sending/storing.
pub struct ObsReplayBufferOutputRef {
    /// Disconnect signals first
    replay_signal_manager: Arc<ObsReplayOutputSignals>,

    output: ObsOutputRef,
}

impl ObsOutputTraitSealed for ObsReplayBufferOutputRef {
    fn new(mut output: OutputInfo, runtime: ObsRuntime) -> Result<Self, ObsError> {
        output.id = ObsString::new("replay_buffer");
        let output = ObsOutputRef::new(output, runtime.clone())?;

        let replay_signal_manager =
            ObsReplayOutputSignals::new(&output.__native_handle(), runtime)?;
        Ok(Self {
            replay_signal_manager: Arc::new(replay_signal_manager),
            output,
        })
    }

    fn video_encoder_slot(
        &self,
    ) -> &std::sync::Arc<
        std::sync::RwLock<Option<std::sync::Arc<crate::encoders::video::ObsVideoEncoder>>>,
    > {
        self.output.video_encoder_slot()
    }

    fn audio_encoder_slots(
        &self,
    ) -> &std::sync::Arc<
        std::sync::RwLock<
            std::collections::HashMap<
                usize,
                std::sync::Arc<crate::encoders::audio::ObsAudioEncoder>,
            >,
        >,
    > {
        self.output.audio_encoder_slots()
    }

    fn service_slot(
        &self,
    ) -> &std::sync::Arc<std::sync::RwLock<Option<std::sync::Arc<crate::services::ObsServiceRef>>>>
    {
        self.output.service_slot()
    }

    fn configuration_lock(&self) -> &std::sync::Arc<std::sync::Mutex<()>> {
        self.output.configuration_lock()
    }
}

forward_obs_object_impl!(ObsReplayBufferOutputRef, output, *mut libobs::obs_output);
forward_obs_output_impl!(ObsReplayBufferOutputRef, output);

impl_signal_manager!(|ptr: SmartPointerSendable<*mut libobs::obs_output>| {
    unsafe {
        // Safety: Again, it carries a reference of the drop guard so we must have a valid pointer
        libobs::obs_output_get_signal_handler(ptr.get_ptr())
    }
}, ObsReplayOutputSignals for *mut libobs::obs_output, [
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
        if self.runtime().is_actor_thread() {
            return Err(ObsError::RuntimeReentrantBlocking);
        }
        log::trace!("Saving replay buffer...");
        let output_ptr = self.__native_handle();

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
