use std::{
    collections::HashMap,
    ffi::CStr,
    fmt::Debug,
    sync::{Arc, RwLock},
};

use crate::{
    data::object::ObsObjectTrait,
    encoders::{audio::ObsAudioEncoder, video::ObsVideoEncoder},
    enums::ObsOutputStopSignal,
    macros::trait_with_optional_send_sync,
    run_with_obs,
    runtime::ObsRuntime,
    utils::{AudioEncoderInfo, ObsError, OutputInfo, VideoEncoderInfo},
};

use super::ObsOutputSignals;

trait_with_optional_send_sync! {
    pub(crate) trait ObsOutputTraitSealed: Debug {
        /// Creates a new output reference from the given output info and runtime.
        ///
        /// # Arguments
        /// * `output` - The output information containing ID, name, and optional settings
        /// * `runtime` - The OBS runtime instance
        ///
        /// # Returns
        /// A Result containing the new ObsOutputRef or an error
        fn new(output: OutputInfo, runtime: ObsRuntime) -> Result<Self, ObsError>
        where
            Self: Sized;
    }
}

#[allow(private_bounds)]
pub trait ObsOutputTrait: ObsOutputTraitSealed + ObsObjectTrait<*mut libobs::obs_output_t> {
    fn signals(&self) -> &Arc<ObsOutputSignals>;

    fn video_encoder(&self) -> &Arc<RwLock<Option<Arc<ObsVideoEncoder>>>>;
    fn audio_encoders(&self) -> &Arc<RwLock<HashMap<usize, Arc<ObsAudioEncoder>>>>;

    /// Returns the current video encoder attached to this output, if any.
    fn get_current_video_encoder(&self) -> Result<Option<Arc<ObsVideoEncoder>>, ObsError> {
        let curr = self
            .video_encoder()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))?;

        Ok(curr.clone())
    }

    /// Creates and attaches a new video encoder to this output.
    ///
    /// Fails if the output is active.
    fn create_and_set_video_encoder(
        &mut self,
        info: VideoEncoderInfo,
    ) -> Result<Arc<ObsVideoEncoder>, ObsError> {
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        let video_enc = ObsVideoEncoder::new_from_info(info, self.runtime().clone())?;

        self.set_video_encoder(video_enc.clone())?;
        Ok(video_enc)
    }

    /// Attaches an existing video encoder to this output.
    ///
    /// Fails if the output is active.
    fn set_video_encoder(&mut self, encoder: Arc<ObsVideoEncoder>) -> Result<(), ObsError> {
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        let output_ptr = self.as_ptr();
        let encoder_ptr = encoder.as_ptr();
        let runtime = self.runtime().clone();

        run_with_obs!(runtime, (output_ptr, encoder_ptr), move || {
            unsafe {
                // Safety: This is safe because we are only using smart pointers.
                libobs::obs_output_set_video_encoder(output_ptr.get_ptr(), encoder_ptr.get_ptr());
            }
        })?;

        self.video_encoder()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?
            .replace(encoder);

        Ok(())
    }

    /// Creates and attaches a new audio encoder for the given mixer index. Fails if output active.
    fn create_and_set_audio_encoder(
        &mut self,
        info: AudioEncoderInfo,
        mixer_idx: usize,
    ) -> Result<Arc<ObsAudioEncoder>, ObsError> {
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        let audio_enc = ObsAudioEncoder::new_from_info(info, mixer_idx, self.runtime().clone())?;
        self.set_audio_encoder(audio_enc.clone(), mixer_idx)?;
        Ok(audio_enc)
    }

    /// Attaches an existing audio encoder to this output at the mixer index.
    ///
    /// Fails if the output is active.
    fn set_audio_encoder(
        &mut self,
        encoder: Arc<ObsAudioEncoder>,
        mixer_idx: usize,
    ) -> Result<(), ObsError> {
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        let encoder_ptr = encoder.as_ptr();
        let output_ptr = self.as_ptr();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (output_ptr, encoder_ptr), move || {
            unsafe {
                // Safety: This is safe because we are only using smart pointers.
                libobs::obs_output_set_audio_encoder(
                    output_ptr.get_ptr(),
                    encoder_ptr.get_ptr(),
                    mixer_idx,
                );
            }
        })?;

        self.audio_encoders()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?
            .insert(mixer_idx, encoder);

        Ok(())
    }

    /// Starts the output, wiring encoders to global contexts and invoking obs_output_start.
    /// Returns an error with last OBS message when start fails.
    fn start(&self) -> Result<(), ObsError> {
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        let vid_encoder_ptr = self
            .video_encoder()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))?
            .as_ref()
            .map(|enc| enc.as_ptr());

        let audio_encoder_pointers = self
            .audio_encoders()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))?
            .values()
            .map(|enc| enc.as_ptr())
            .collect::<Vec<_>>();

        let output_ptr = self.as_ptr();
        let runtime = self.runtime().clone();
        let res = run_with_obs!(
            runtime,
            (output_ptr, vid_encoder_ptr, audio_encoder_pointers),
            move || {
                if let Some(vid_encoder_ptr) = vid_encoder_ptr {
                    unsafe {
                        // Safety: vid_encoder_ptr is valid because of SmartPointer
                        libobs::obs_encoder_set_video(
                            vid_encoder_ptr.get_ptr(),
                            libobs::obs_get_video(),
                        );
                    }
                }
                for audio_encoder_ptr in audio_encoder_pointers {
                    unsafe {
                        // Safety: audio_encoder_ptr is valid because of SmartPointer
                        libobs::obs_encoder_set_audio(
                            audio_encoder_ptr.get_ptr(),
                            libobs::obs_get_audio(),
                        );
                    }
                }

                unsafe {
                    // Safety: output_ptr is valid because of SmartPointer
                    libobs::obs_output_start(output_ptr.get_ptr())
                }
            }
        )?;

        if res {
            return Ok(());
        }

        let runtime = self.runtime().clone();
        let err = run_with_obs!(runtime, (output_ptr), move || {
            let err = unsafe {
                // Safety: The output pointer must be valid because of SmartPointer
                libobs::obs_output_get_last_error(output_ptr.get_ptr())
            };

            if err.is_null() {
                return "Unknown error".to_string();
            }

            let err = unsafe { CStr::from_ptr(err) };

            let err = err.to_string_lossy().to_string();
            err
        })?;

        Err(ObsError::OutputStartFailure(Some(err)))
    }

    fn set_paused(&self, should_pause: bool) -> Result<(), ObsError> {
        if !self.is_active()? {
            return Err(ObsError::OutputPauseFailure(Some(
                "Output is not active.".to_string(),
            )));
        }

        let output_ptr = self.as_ptr();
        let runtime = self.runtime().clone();

        let mut rx = if should_pause {
            self.signals().on_pause()?
        } else {
            self.signals().on_unpause()?
        };

        let res = run_with_obs!(runtime, (output_ptr), move || {
            unsafe {
                // Safety: output_ptr is valid because of SmartPointer
                libobs::obs_output_pause(output_ptr.get_ptr(), should_pause)
            }
        })?;

        if res {
            rx.blocking_recv().map_err(|_| ObsError::NoSenderError)?;

            Ok(())
        } else {
            let runtime = self.runtime().clone();
            let err = run_with_obs!(runtime, (output_ptr), move || {
                let err = unsafe {
                    // Safety: output_ptr is valid because of SmartPointer
                    libobs::obs_output_get_last_error(output_ptr.get_ptr())
                };

                if err.is_null() {
                    return None;
                }

                let err = unsafe { CStr::from_ptr(err) };
                let err = err.to_string_lossy().to_string();

                Some(err)
            })?;

            Err(ObsError::OutputPauseFailure(err))
        }
    }

    /// Pauses or resumes the output and waits for the pause/unpause signal.
    fn pause(&self) -> Result<(), ObsError> {
        self.set_paused(true)
    }

    fn unpause(&self) -> Result<(), ObsError> {
        self.set_paused(false)
    }

    /// Stops the output and waits for stop and deactivate signals.
    fn stop(&mut self) -> Result<(), ObsError> {
        let output_ptr = self.as_ptr();
        let runtime = self.runtime().clone();
        let output_active = run_with_obs!(runtime, (output_ptr), move || {
            unsafe {
                // Safety: output_ptr is valid because of SmartPointer
                libobs::obs_output_active(output_ptr.get_ptr())
            }
        })?;

        if !output_active {
            return Err(ObsError::OutputStopFailure(Some(
                "Output is not active.".to_string(),
            )));
        }

        let mut rx = self.signals().on_stop()?;
        let mut rx_deactivate = self.signals().on_deactivate()?;

        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (output_ptr), move || {
            unsafe {
                // Safety: output_ptr is valid because of SmartPointer
                libobs::obs_output_stop(output_ptr.get_ptr())
            }
        })?;

        let signal = rx.blocking_recv().map_err(|_| ObsError::NoSenderError)?;

        log::trace!("Received stop signal: {:?}", signal);
        if signal != ObsOutputStopSignal::Success {
            return Err(ObsError::OutputStopFailure(Some(signal.to_string())));
        }

        rx_deactivate
            .blocking_recv()
            .map_err(|_| ObsError::NoSenderError)?;

        Ok(())
    }

    /// Returns whether the output is currently active.
    fn is_active(&self) -> Result<bool, ObsError> {
        let output_ptr = self.as_ptr();
        let runtime = self.runtime().clone();
        let output_active = run_with_obs!(runtime, (output_ptr), move || {
            unsafe {
                // Safety: output_ptr is valid because of SmartPointer
                libobs::obs_output_active(output_ptr.get_ptr())
            }
        })?;

        Ok(output_active)
    }
}
