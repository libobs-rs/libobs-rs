use std::{
    collections::{HashMap, HashSet},
    ffi::CStr,
    fmt::Debug,
    sync::{Arc, Mutex, RwLock},
};

use crate::{
    data::object::ObsObjectTrait,
    encoders::{audio::ObsAudioEncoder, video::ObsVideoEncoder},
    enums::ObsOutputStopSignal,
    macros::trait_with_optional_send_sync,
    run_with_obs,
    runtime::ObsRuntime,
    services::ObsServiceRef,
    utils::{AudioEncoderInfo, ObsError, OutputInfo, VideoEncoderInfo},
};

use super::ObsOutputSignals;

#[derive(Clone, Debug, Default)]
/// Desired encoder/service wiring for an OBS output.
///
/// Applying a composition replaces the complete managed wiring in one actor command: an
/// omitted video encoder or service is detached, and audio mixer slots not present in
/// `audio_encoders` are cleared. All handles are validated for runtime affinity before any
/// native state is changed.
pub struct ObsOutputComposition {
    video_encoder: Option<Arc<ObsVideoEncoder>>,
    audio_encoders: HashMap<usize, Arc<ObsAudioEncoder>>,
    service: Option<Arc<ObsServiceRef>>,
}

impl ObsOutputComposition {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_video_encoder(mut self, encoder: Arc<ObsVideoEncoder>) -> Self {
        self.video_encoder = Some(encoder);
        self
    }

    pub fn with_audio_encoder(mut self, mixer_idx: usize, encoder: Arc<ObsAudioEncoder>) -> Self {
        self.audio_encoders.insert(mixer_idx, encoder);
        self
    }

    pub fn with_service(mut self, service: Arc<ObsServiceRef>) -> Self {
        self.service = Some(service);
        self
    }

    pub fn without_video_encoder(mut self) -> Self {
        self.video_encoder = None;
        self
    }

    pub fn without_audio_encoder(mut self, mixer_idx: usize) -> Self {
        self.audio_encoders.remove(&mixer_idx);
        self
    }

    pub fn without_service(mut self) -> Self {
        self.service = None;
        self
    }

    pub fn video_encoder(&self) -> Option<&Arc<ObsVideoEncoder>> {
        self.video_encoder.as_ref()
    }

    pub fn audio_encoders(&self) -> &HashMap<usize, Arc<ObsAudioEncoder>> {
        &self.audio_encoders
    }

    pub fn service(&self) -> Option<&Arc<ObsServiceRef>> {
        self.service.as_ref()
    }
}

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
pub trait ObsOutputTrait:
    ObsOutputTraitSealed + ObsObjectTrait<Native = *mut libobs::obs_output_t>
{
    fn signals(&self) -> &Arc<ObsOutputSignals>;

    fn video_encoder(&self) -> &Arc<RwLock<Option<Arc<ObsVideoEncoder>>>>;
    fn audio_encoders(&self) -> &Arc<RwLock<HashMap<usize, Arc<ObsAudioEncoder>>>>;
    fn service(&self) -> &Arc<RwLock<Option<Arc<ObsServiceRef>>>>;
    fn configuration_lock(&self) -> &Arc<Mutex<()>>;

    /// Returns the current video encoder attached to this output, if any.
    fn get_current_video_encoder(&self) -> Result<Option<Arc<ObsVideoEncoder>>, ObsError> {
        let curr = self
            .video_encoder()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))?;

        Ok(curr.clone())
    }

    /// Returns the audio encoder attached at the requested mixer index, if any.
    fn get_current_audio_encoder(
        &self,
        mixer_idx: usize,
    ) -> Result<Option<Arc<ObsAudioEncoder>>, ObsError> {
        self.audio_encoders()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))
            .map(|encoders| encoders.get(&mixer_idx).cloned())
    }

    /// Returns a snapshot of all managed audio encoder attachments keyed by mixer index.
    fn get_current_audio_encoders(&self) -> Result<HashMap<usize, Arc<ObsAudioEncoder>>, ObsError> {
        self.audio_encoders()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))
            .map(|encoders| encoders.clone())
    }

    /// Returns the streaming service attached to this output, if any.
    fn get_current_service(&self) -> Result<Option<Arc<ObsServiceRef>>, ObsError> {
        self.service()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))
            .map(|service| service.clone())
    }

    /// Returns an owned snapshot of the currently managed encoder/service wiring.
    fn current_composition(&self) -> Result<ObsOutputComposition, ObsError> {
        Ok(ObsOutputComposition {
            video_encoder: self.get_current_video_encoder()?,
            audio_encoders: self.get_current_audio_encoders()?,
            service: self.get_current_service()?,
        })
    }

    /// Attaches a streaming service to this output. Fails while the output is active.
    fn set_service(&self, service: Arc<ObsServiceRef>) -> Result<(), ObsError> {
        let _configuration = self
            .configuration_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }
        self.runtime().ensure_same_runtime(service.runtime())?;

        let mut slot = self
            .service()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        let output_ptr = self.__native_handle();
        let service_ptr = service.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (output_ptr, service_ptr), move || {
            // Safety: both managed handles retain their native objects for the actor call.
            unsafe { libobs::obs_output_set_service(output_ptr.get_ptr(), service_ptr.get_ptr()) };
        })?;

        slot.replace(service);
        Ok(())
    }

    /// Replaces the complete managed output wiring in one actor command.
    ///
    /// This is the preferred API when constructing an output from discovered capabilities: it
    /// validates all runtime affinities before touching libobs, detaches omitted components, and
    /// updates the Rust-side ownership graph only after the native calls complete.
    fn apply_composition(&self, composition: ObsOutputComposition) -> Result<(), ObsError> {
        let _configuration = self
            .configuration_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        for mixer_idx in composition.audio_encoders.keys() {
            validate_audio_mixer(*mixer_idx)?;
        }
        if let Some(encoder) = composition.video_encoder.as_ref() {
            self.runtime().ensure_same_runtime(encoder.runtime())?;
        }
        for encoder in composition.audio_encoders.values() {
            self.runtime().ensure_same_runtime(encoder.runtime())?;
        }
        if let Some(service) = composition.service.as_ref() {
            self.runtime().ensure_same_runtime(service.runtime())?;
        }

        // Take all locks before changing native state so poisoned Rust-side state cannot leave
        // libobs and the managed ownership graph out of sync.
        let mut video_slot = self
            .video_encoder()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        let mut audio_slots = self
            .audio_encoders()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        let mut service_slot = self
            .service()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;

        let output_ptr = self.__native_handle();
        let video_ptr = composition
            .video_encoder
            .as_ref()
            .map(|encoder| encoder.__native_handle());
        let service_ptr = composition
            .service
            .as_ref()
            .map(|service| service.__native_handle());
        let audio_ptrs = composition
            .audio_encoders
            .iter()
            .map(|(mixer_idx, encoder)| (*mixer_idx, encoder.__native_handle()))
            .collect::<HashMap<_, _>>();
        let mixer_indices = audio_slots
            .keys()
            .copied()
            .chain(audio_ptrs.keys().copied())
            .collect::<HashSet<_>>();
        let runtime = self.runtime().clone();

        run_with_obs!(
            runtime,
            (
                output_ptr,
                video_ptr,
                service_ptr,
                audio_ptrs,
                mixer_indices
            ),
            move || unsafe {
                // Safety: every non-null pointer is retained by a managed handle captured for the
                // actor call. Null is the documented libobs detach value for these setters.
                libobs::obs_output_set_video_encoder(
                    output_ptr.get_ptr(),
                    video_ptr
                        .as_ref()
                        .map_or(std::ptr::null_mut(), |encoder| encoder.get_ptr()),
                );
                for mixer_idx in mixer_indices {
                    libobs::obs_output_set_audio_encoder(
                        output_ptr.get_ptr(),
                        audio_ptrs
                            .get(&mixer_idx)
                            .map_or(std::ptr::null_mut(), |encoder| encoder.get_ptr()),
                        mixer_idx,
                    );
                }
                libobs::obs_output_set_service(
                    output_ptr.get_ptr(),
                    service_ptr
                        .as_ref()
                        .map_or(std::ptr::null_mut(), |service| service.get_ptr()),
                );
            }
        )?;

        *video_slot = composition.video_encoder;
        *audio_slots = composition.audio_encoders;
        *service_slot = composition.service;
        Ok(())
    }

    /// Detaches the current video encoder while the output is inactive.
    fn clear_video_encoder(&self) -> Result<(), ObsError> {
        let _configuration = self
            .configuration_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }
        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();
        let mut slot = self
            .video_encoder()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        run_with_obs!(runtime, (output_ptr), move || unsafe {
            // Safety: the managed output handle remains valid for the actor call; null detaches.
            libobs::obs_output_set_video_encoder(output_ptr.get_ptr(), std::ptr::null_mut());
        })?;
        *slot = None;
        Ok(())
    }

    /// Detaches the audio encoder at `mixer_idx` while the output is inactive.
    fn clear_audio_encoder(&self, mixer_idx: usize) -> Result<(), ObsError> {
        let _configuration = self
            .configuration_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        validate_audio_mixer(mixer_idx)?;
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }
        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();
        let mut slots = self
            .audio_encoders()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        run_with_obs!(runtime, (output_ptr), move || unsafe {
            // Safety: the managed output handle remains valid for the actor call; null detaches.
            libobs::obs_output_set_audio_encoder(
                output_ptr.get_ptr(),
                std::ptr::null_mut(),
                mixer_idx,
            );
        })?;
        slots.remove(&mixer_idx);
        Ok(())
    }

    /// Detaches the current streaming service while the output is inactive.
    fn clear_service(&self) -> Result<(), ObsError> {
        let _configuration = self
            .configuration_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }
        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();
        let mut slot = self
            .service()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        run_with_obs!(runtime, (output_ptr), move || unsafe {
            // Safety: the managed output handle remains valid for the actor call; null detaches.
            libobs::obs_output_set_service(output_ptr.get_ptr(), std::ptr::null_mut());
        })?;
        *slot = None;
        Ok(())
    }

    /// Creates and attaches a new video encoder to this output.
    ///
    /// Fails if the output is active.
    fn create_and_set_video_encoder(
        &self,
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
    fn set_video_encoder(&self, encoder: Arc<ObsVideoEncoder>) -> Result<(), ObsError> {
        let _configuration = self
            .configuration_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }
        self.runtime().ensure_same_runtime(encoder.runtime())?;

        let mut slot = self
            .video_encoder()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        let output_ptr = self.__native_handle();
        let encoder_ptr = encoder.__native_handle();
        let runtime = self.runtime().clone();

        run_with_obs!(runtime, (output_ptr, encoder_ptr), move || {
            unsafe {
                // Safety: This is safe because we are only using smart pointers.
                libobs::obs_output_set_video_encoder(output_ptr.get_ptr(), encoder_ptr.get_ptr());
            }
        })?;

        slot.replace(encoder);

        Ok(())
    }

    /// Creates and attaches a new audio encoder for the given mixer index. Fails if output active.
    fn create_and_set_audio_encoder(
        &self,
        info: AudioEncoderInfo,
        mixer_idx: usize,
    ) -> Result<Arc<ObsAudioEncoder>, ObsError> {
        validate_audio_mixer(mixer_idx)?;
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
        &self,
        encoder: Arc<ObsAudioEncoder>,
        mixer_idx: usize,
    ) -> Result<(), ObsError> {
        let _configuration = self
            .configuration_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        validate_audio_mixer(mixer_idx)?;
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        self.runtime().ensure_same_runtime(encoder.runtime())?;

        let mut slots = self
            .audio_encoders()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        let encoder_ptr = encoder.__native_handle();
        let output_ptr = self.__native_handle();
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

        slots.insert(mixer_idx, encoder);

        Ok(())
    }

    /// Starts the output, wiring encoders to global contexts and invoking obs_output_start.
    /// Returns an error with last OBS message when start fails.
    fn start(&self) -> Result<(), ObsError> {
        let _configuration = self
            .configuration_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        let vid_encoder_ptr = self
            .video_encoder()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))?
            .as_ref()
            .map(|enc| enc.__native_handle());

        let audio_encoder_pointers = self
            .audio_encoders()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))?
            .values()
            .map(|enc| enc.__native_handle())
            .collect::<Vec<_>>();

        let output_ptr = self.__native_handle();
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
        if self.runtime().is_actor_thread() {
            return Err(ObsError::RuntimeReentrantBlocking);
        }
        if !self.is_active()? {
            return Err(ObsError::OutputPauseFailure(Some(
                "Output is not active.".to_string(),
            )));
        }

        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();

        let rx = if should_pause {
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
    fn stop(&self) -> Result<(), ObsError> {
        if self.runtime().is_actor_thread() {
            return Err(ObsError::RuntimeReentrantBlocking);
        }
        let _configuration = self
            .configuration_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        let output_ptr = self.__native_handle();
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

        let rx = self.signals().on_stop()?;
        let rx_deactivate = self.signals().on_deactivate()?;

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
        let output_ptr = self.__native_handle();
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

fn validate_audio_mixer(mixer_idx: usize) -> Result<(), ObsError> {
    if mixer_idx >= libobs::MAX_AUDIO_MIXES as usize {
        return Err(ObsError::InvalidOperation(format!(
            "Audio mixer index {mixer_idx} is out of bounds (max {})",
            libobs::MAX_AUDIO_MIXES - 1
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_audio_mixer;

    #[test]
    fn audio_mixer_indices_are_bounded_before_ffi() {
        assert!(validate_audio_mixer(0).is_ok());
        assert!(validate_audio_mixer((libobs::MAX_AUDIO_MIXES - 1) as usize).is_ok());
        assert!(validate_audio_mixer(libobs::MAX_AUDIO_MIXES as usize).is_err());
    }
}
