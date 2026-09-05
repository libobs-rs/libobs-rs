use std::{
    collections::{HashMap, HashSet},
    ffi::CStr,
    fmt::Debug,
    sync::{Arc, Mutex, MutexGuard, RwLock, TryLockError},
    thread,
    time::{Duration, Instant},
};

use crossbeam_channel::RecvTimeoutError;

use crate::{
    data::{
        audio::{AudioEncoderSlot, AudioMixIndex},
        object::ObsObjectTrait,
    },
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

    /// Compatibility helper: the index here is an output encoder slot, not the encoder's source mix.
    pub fn with_audio_encoder(mut self, slot: usize, encoder: Arc<ObsAudioEncoder>) -> Self {
        self.audio_encoders.insert(slot, encoder);
        self
    }

    pub fn with_audio_encoder_at_slot(
        mut self,
        slot: AudioEncoderSlot,
        encoder: Arc<ObsAudioEncoder>,
    ) -> Self {
        self.audio_encoders.insert(slot.index(), encoder);
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

    pub fn without_audio_encoder(mut self, slot: usize) -> Self {
        self.audio_encoders.remove(&slot);
        self
    }

    pub fn without_audio_encoder_at_slot(mut self, slot: AudioEncoderSlot) -> Self {
        self.audio_encoders.remove(&slot.index());
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
        fn new(output: OutputInfo, runtime: ObsRuntime) -> Result<Self, ObsError>
        where
            Self: Sized;

        fn video_encoder_slot(&self) -> &Arc<RwLock<Option<Arc<ObsVideoEncoder>>>>;
        fn audio_encoder_slots(&self) -> &Arc<RwLock<HashMap<usize, Arc<ObsAudioEncoder>>>>;
        fn service_slot(&self) -> &Arc<RwLock<Option<Arc<ObsServiceRef>>>>;
        fn configuration_lock(&self) -> &Arc<Mutex<()>>;
        fn lifecycle_lock(&self) -> &Arc<Mutex<()>>;
    }
}

/// Default deadline used by blocking lifecycle operations.
///
/// A timeout only stops the Rust caller from waiting; libobs does not provide cancellation for
/// these requests and native completion may still arrive later.
pub const DEFAULT_OUTPUT_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn operation_timeout(operation: &str, timeout: Duration) -> ObsError {
    ObsError::OutputOperationTimeout {
        operation: operation.to_owned(),
        timeout_ms: timeout.as_millis().min(u64::MAX as u128) as u64,
    }
}

pub(crate) fn remaining(
    deadline: Instant,
    operation: &str,
    timeout: Duration,
) -> Result<Duration, ObsError> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| operation_timeout(operation, timeout))
}

pub(crate) fn lock_until<'a>(
    lock: &'a Mutex<()>,
    deadline: Instant,
    operation: &str,
    timeout: Duration,
) -> Result<MutexGuard<'a, ()>, ObsError> {
    loop {
        match lock.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(err)) => {
                return Err(ObsError::LockError(err.to_string()));
            }
            Err(TryLockError::WouldBlock) => {
                let wait = remaining(deadline, operation, timeout)?;
                thread::sleep(wait.min(Duration::from_millis(1)));
            }
        }
    }
}

pub(crate) fn wait_for_completion<T>(
    receiver: &crate::signals::SignalReceiver<T>,
    deadline: Instant,
    operation: &str,
    timeout: Duration,
) -> Result<T, ObsError> {
    let wait = remaining(deadline, operation, timeout)?;
    receiver.recv_timeout(wait).map_err(|err| match err {
        RecvTimeoutError::Timeout => operation_timeout(operation, timeout),
        RecvTimeoutError::Disconnected => ObsError::OutputCompletionDisconnected {
            operation: operation.to_owned(),
        },
    })
}

#[allow(private_bounds)]
pub trait ObsOutputTrait:
    ObsOutputTraitSealed + ObsObjectTrait<Native = *mut libobs::obs_output_t>
{
    fn signals(&self) -> &Arc<ObsOutputSignals>;

    #[doc(hidden)]
    fn native_output_flags(&self) -> Result<u32, ObsError> {
        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (output_ptr), move || {
            // Safety: the managed output handle remains alive for the actor call.
            unsafe { libobs::obs_output_get_flags(output_ptr.get_ptr()) }
        })
    }

    /// Returns the video encoder currently attached to this output, if any.
    fn attached_video_encoder(&self) -> Result<Option<Arc<ObsVideoEncoder>>, ObsError> {
        let curr = self
            .video_encoder_slot()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))?;

        Ok(curr.clone())
    }

    /// Returns the audio encoder attached at the requested mixer index, if any.
    /// Compatibility helper: `slot` is the output attachment slot, not an audio mix.
    fn attached_audio_encoder(
        &self,
        slot: usize,
    ) -> Result<Option<Arc<ObsAudioEncoder>>, ObsError> {
        self.audio_encoder_slots()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))
            .map(|encoders| encoders.get(&slot).cloned())
    }

    fn attached_audio_encoder_at_slot(
        &self,
        slot: AudioEncoderSlot,
    ) -> Result<Option<Arc<ObsAudioEncoder>>, ObsError> {
        self.attached_audio_encoder(slot.index())
    }

    /// Returns a snapshot of all managed audio encoder attachments keyed by mixer index.
    fn attached_audio_encoders(&self) -> Result<HashMap<usize, Arc<ObsAudioEncoder>>, ObsError> {
        self.audio_encoder_slots()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))
            .map(|encoders| encoders.clone())
    }

    /// Returns the streaming service attached to this output, if any.
    fn attached_service(&self) -> Result<Option<Arc<ObsServiceRef>>, ObsError> {
        self.service_slot()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))
            .map(|service| service.clone())
    }

    #[deprecated = "Use attached_video_encoder"]
    fn get_current_video_encoder(&self) -> Result<Option<Arc<ObsVideoEncoder>>, ObsError> {
        self.attached_video_encoder()
    }

    #[deprecated = "Use attached_audio_encoder"]
    fn get_current_audio_encoder(
        &self,
        mixer_idx: usize,
    ) -> Result<Option<Arc<ObsAudioEncoder>>, ObsError> {
        self.attached_audio_encoder(mixer_idx)
    }

    #[deprecated = "Use attached_audio_encoders"]
    fn get_current_audio_encoders(&self) -> Result<HashMap<usize, Arc<ObsAudioEncoder>>, ObsError> {
        self.attached_audio_encoders()
    }

    #[deprecated = "Use attached_service"]
    fn get_current_service(&self) -> Result<Option<Arc<ObsServiceRef>>, ObsError> {
        self.attached_service()
    }

    /// Returns an owned, coherent snapshot of the currently managed encoder/service wiring.
    fn current_composition(&self) -> Result<ObsOutputComposition, ObsError> {
        let _configuration = self
            .configuration_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        Ok(ObsOutputComposition {
            video_encoder: self.attached_video_encoder()?,
            audio_encoders: self.attached_audio_encoders()?,
            service: self.attached_service()?,
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
        let flags = self.native_output_flags()?;
        ensure_output_supports_service(self, flags)?;
        self.runtime().ensure_same_runtime(service.runtime())?;

        let mut slot = self
            .service_slot()
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
        let flags = self.native_output_flags()?;
        if composition.video_encoder.is_some() {
            ensure_output_supports_video_encoder(self, flags)?;
        }
        if !composition.audio_encoders.is_empty() {
            ensure_output_supports_audio_encoder(self, flags)?;
        }
        if composition.service.is_some() {
            ensure_output_supports_service(self, flags)?;
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
            .video_encoder_slot()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        let mut audio_slots = self
            .audio_encoder_slots()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        let mut service_slot = self
            .service_slot()
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
                mixer_indices,
                flags
            ),
            move || unsafe {
                // Safety: every non-null pointer is retained by a managed handle captured for the
                // actor call. Null is the documented libobs detach value for these setters.
                // Only call setters supported by the concrete output: libobs warns (and third-party
                // plugins may do worse) when service/encoder setters are used on the wrong category.
                if output_supports_video_encoder(flags) {
                    libobs::obs_output_set_video_encoder(
                        output_ptr.get_ptr(),
                        video_ptr
                            .as_ref()
                            .map_or(std::ptr::null_mut(), |encoder| encoder.get_ptr()),
                    );
                }
                if output_supports_audio_encoder(flags) {
                    for mixer_idx in mixer_indices {
                        libobs::obs_output_set_audio_encoder(
                            output_ptr.get_ptr(),
                            audio_ptrs
                                .get(&mixer_idx)
                                .map_or(std::ptr::null_mut(), |encoder| encoder.get_ptr()),
                            mixer_idx,
                        );
                    }
                }
                if output_supports_service(flags) {
                    libobs::obs_output_set_service(
                        output_ptr.get_ptr(),
                        service_ptr
                            .as_ref()
                            .map_or(std::ptr::null_mut(), |service| service.get_ptr()),
                    );
                }
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
        let flags = self.native_output_flags()?;
        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();
        let mut slot = self
            .video_encoder_slot()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        if output_supports_video_encoder(flags) {
            run_with_obs!(runtime, (output_ptr), move || unsafe {
                // Safety: the managed output handle remains valid for the actor call; null detaches.
                libobs::obs_output_set_video_encoder(output_ptr.get_ptr(), std::ptr::null_mut());
            })?;
        }
        *slot = None;
        Ok(())
    }

    /// Detaches the audio encoder at an output attachment slot while inactive.
    /// The historical parameter name was "mixer index"; it never selected the encoder's source mix.
    fn clear_audio_encoder(&self, mixer_idx: usize) -> Result<(), ObsError> {
        let _configuration = self
            .configuration_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        validate_audio_mixer(mixer_idx)?;
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }
        let flags = self.native_output_flags()?;
        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();
        let mut slots = self
            .audio_encoder_slots()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        if output_supports_audio_encoder(flags) {
            run_with_obs!(runtime, (output_ptr), move || unsafe {
                // Safety: the managed output handle remains valid for the actor call; null detaches.
                libobs::obs_output_set_audio_encoder(
                    output_ptr.get_ptr(),
                    std::ptr::null_mut(),
                    mixer_idx,
                );
            })?;
        }
        slots.remove(&mixer_idx);
        Ok(())
    }

    fn clear_audio_encoder_at_slot(&self, slot: AudioEncoderSlot) -> Result<(), ObsError> {
        self.clear_audio_encoder(slot.index())
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
        let flags = self.native_output_flags()?;
        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();
        let mut slot = self
            .service_slot()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        if output_supports_service(flags) {
            run_with_obs!(runtime, (output_ptr), move || unsafe {
                // Safety: the managed output handle remains valid for the actor call; null detaches.
                libobs::obs_output_set_service(output_ptr.get_ptr(), std::ptr::null_mut());
            })?;
        }
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
        ensure_output_supports_video_encoder(self, self.native_output_flags()?)?;

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
        ensure_output_supports_video_encoder(self, self.native_output_flags()?)?;
        self.runtime().ensure_same_runtime(encoder.runtime())?;

        let mut slot = self
            .video_encoder_slot()
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

    /// Compatibility helper that historically used one integer for both concepts.
    ///
    /// It creates an encoder reading native mix `index` and also attaches it at output slot
    /// `index`. New code should use [`Self::create_and_set_audio_encoder_for_mix_at_slot`].
    fn create_and_set_audio_encoder(
        &self,
        info: AudioEncoderInfo,
        index: usize,
    ) -> Result<Arc<ObsAudioEncoder>, ObsError> {
        let mix = AudioMixIndex::new(index)?;
        let slot = AudioEncoderSlot::new(index)?;
        self.create_and_set_audio_encoder_for_mix_at_slot(info, mix, slot)
    }

    /// Creates an encoder for `mix` and attaches it independently at output `slot`.
    fn create_and_set_audio_encoder_for_mix_at_slot(
        &self,
        info: AudioEncoderInfo,
        mix: AudioMixIndex,
        slot: AudioEncoderSlot,
    ) -> Result<Arc<ObsAudioEncoder>, ObsError> {
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }
        ensure_output_supports_audio_encoder(self, self.native_output_flags()?)?;
        let audio_enc = ObsAudioEncoder::new_from_info_for_mix(info, mix, self.runtime().clone())?;
        self.set_audio_encoder_at_slot(audio_enc.clone(), slot)?;
        Ok(audio_enc)
    }

    /// Compatibility helper: `mixer_idx` is actually an output attachment slot.
    fn set_audio_encoder(
        &self,
        encoder: Arc<ObsAudioEncoder>,
        mixer_idx: usize,
    ) -> Result<(), ObsError> {
        self.set_audio_encoder_at_slot(encoder, AudioEncoderSlot::new(mixer_idx)?)
    }

    /// Attaches an existing audio encoder at an output slot. The encoder retains its own,
    /// independently selected [`AudioMixIndex`].
    fn set_audio_encoder_at_slot(
        &self,
        encoder: Arc<ObsAudioEncoder>,
        slot: AudioEncoderSlot,
    ) -> Result<(), ObsError> {
        let _configuration = self
            .configuration_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }
        ensure_output_supports_audio_encoder(self, self.native_output_flags()?)?;
        self.runtime().ensure_same_runtime(encoder.runtime())?;

        let mut slots = self
            .audio_encoder_slots()
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        let encoder_ptr = encoder.__native_handle();
        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();
        let slot_index = slot.index();
        run_with_obs!(runtime, (output_ptr, encoder_ptr), move || unsafe {
            // Safety: managed handles retain both objects and AudioEncoderSlot is range checked.
            libobs::obs_output_set_audio_encoder(
                output_ptr.get_ptr(),
                encoder_ptr.get_ptr(),
                slot_index,
            );
        })?;
        slots.insert(slot_index, encoder);
        Ok(())
    }

    /// Starts the output, wiring encoders to global contexts and invoking `obs_output_start`.
    ///
    /// Lifecycle requests are serialized per output. Starting an already-active output keeps the
    /// historical `OutputAlreadyActive` error so accidental duplicate starts remain visible.
    fn start(&self) -> Result<(), ObsError> {
        let _lifecycle = self
            .lifecycle_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        let _configuration = self
            .configuration_lock()
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        let vid_encoder_ptr = self
            .video_encoder_slot()
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))?
            .as_ref()
            .map(|enc| enc.__native_handle());

        let audio_encoder_pointers = self
            .audio_encoder_slots()
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
                        // Safety: vid_encoder_ptr is retained by the managed encoder handle.
                        libobs::obs_encoder_set_video(
                            vid_encoder_ptr.get_ptr(),
                            libobs::obs_get_video(),
                        );
                    }
                }
                for audio_encoder_ptr in audio_encoder_pointers {
                    unsafe {
                        // Safety: audio_encoder_ptr is retained by the managed encoder handle.
                        libobs::obs_encoder_set_audio(
                            audio_encoder_ptr.get_ptr(),
                            libobs::obs_get_audio(),
                        );
                    }
                }

                unsafe {
                    // Safety: output_ptr is retained by the managed output handle.
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
                // Safety: output_ptr is retained by the managed output handle.
                libobs::obs_output_get_last_error(output_ptr.get_ptr())
            };

            if err.is_null() {
                return "Unknown error".to_string();
            }

            unsafe { CStr::from_ptr(err) }.to_string_lossy().to_string()
        })?;

        Err(ObsError::OutputStartFailure(Some(err)))
    }

    /// Pauses/resumes with an explicit completion deadline.
    ///
    /// Repeating the state already in effect is idempotent and returns immediately. Concurrent
    /// lifecycle requests are serialized so one request cannot consume another request's signal.
    /// A timeout does **not** cancel the native pause request; query [`Self::is_paused`] before
    /// retrying.
    fn set_paused_with_timeout(
        &self,
        should_pause: bool,
        timeout: Duration,
    ) -> Result<(), ObsError> {
        if self.runtime().is_actor_thread() {
            return Err(ObsError::RuntimeReentrantBlocking);
        }
        let operation = if should_pause { "pause" } else { "unpause" };
        let started = Instant::now();
        let deadline = started.checked_add(timeout).unwrap_or(started);
        let _lifecycle = lock_until(self.lifecycle_lock(), deadline, operation, timeout)?;

        if !self.is_active()? {
            return Err(ObsError::OutputPauseFailure(Some(
                "Output is not active.".to_string(),
            )));
        }
        if self.is_paused()? == should_pause {
            return Ok(());
        }

        // Subscribe before initiating the operation so a synchronous/very-fast callback cannot be
        // lost between the native call and subscription setup.
        let receiver = if should_pause {
            self.signals().on_pause()?
        } else {
            self.signals().on_unpause()?
        };

        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();
        let res = run_with_obs!(runtime, (output_ptr), move || unsafe {
            // Safety: output_ptr is retained by the managed output handle.
            libobs::obs_output_pause(output_ptr.get_ptr(), should_pause)
        })?;

        if !res {
            let runtime = self.runtime().clone();
            let err = run_with_obs!(runtime, (output_ptr), move || {
                let err = unsafe {
                    // Safety: output_ptr is retained by the managed output handle.
                    libobs::obs_output_get_last_error(output_ptr.get_ptr())
                };
                if err.is_null() {
                    None
                } else {
                    Some(unsafe { CStr::from_ptr(err) }.to_string_lossy().to_string())
                }
            })?;
            return Err(ObsError::OutputPauseFailure(err));
        }

        wait_for_completion(&receiver, deadline, operation, timeout)?;
        Ok(())
    }

    /// Pauses the output with [`DEFAULT_OUTPUT_OPERATION_TIMEOUT`].
    fn pause(&self) -> Result<(), ObsError> {
        self.pause_with_timeout(DEFAULT_OUTPUT_OPERATION_TIMEOUT)
    }

    /// Pauses the output and waits at most `timeout` for native completion.
    fn pause_with_timeout(&self, timeout: Duration) -> Result<(), ObsError> {
        self.set_paused_with_timeout(true, timeout)
    }

    /// Resumes the output with [`DEFAULT_OUTPUT_OPERATION_TIMEOUT`].
    fn unpause(&self) -> Result<(), ObsError> {
        self.unpause_with_timeout(DEFAULT_OUTPUT_OPERATION_TIMEOUT)
    }

    /// Resumes the output and waits at most `timeout` for native completion.
    fn unpause_with_timeout(&self, timeout: Duration) -> Result<(), ObsError> {
        self.set_paused_with_timeout(false, timeout)
    }

    /// Stops the output with [`DEFAULT_OUTPUT_OPERATION_TIMEOUT`].
    ///
    /// Stop is idempotent: if the output is already inactive and not reconnecting, this returns
    /// `Ok(())`. A timeout only bounds this caller's wait; libobs may complete later. The managed
    /// output continues retaining its encoder/service graph. Re-query [`Self::is_active`] and
    /// [`Self::is_reconnecting`] before changing or releasing that graph.
    fn stop(&self) -> Result<(), ObsError> {
        self.stop_with_timeout(DEFAULT_OUTPUT_OPERATION_TIMEOUT)
    }

    /// Requests a graceful stop and waits at most one overall `timeout` for both the `stop` and
    /// `deactivate` completion signals. Time spent waiting behind another lifecycle/configuration
    /// operation consumes the same deadline.
    fn stop_with_timeout(&self, timeout: Duration) -> Result<(), ObsError> {
        self.stop_impl(timeout, false)
    }

    /// Requests libobs's force-stop recovery path and waits for normal stop/deactivate completion.
    ///
    /// `obs_output_force_stop` asks the output to stop immediately without waiting for queued data;
    /// it is appropriate after a graceful stop deadline when data loss is preferable to waiting.
    /// It is not cancellation and still uses the same bounded completion semantics here.
    fn force_stop(&self) -> Result<(), ObsError> {
        self.force_stop_with_timeout(DEFAULT_OUTPUT_OPERATION_TIMEOUT)
    }

    /// Force-stop variant with an explicit overall deadline.
    fn force_stop_with_timeout(&self, timeout: Duration) -> Result<(), ObsError> {
        self.stop_impl(timeout, true)
    }

    #[doc(hidden)]
    fn stop_impl(&self, timeout: Duration, force: bool) -> Result<(), ObsError> {
        if self.runtime().is_actor_thread() {
            return Err(ObsError::RuntimeReentrantBlocking);
        }
        let operation = if force { "force-stop" } else { "stop" };
        let started = Instant::now();
        let deadline = started.checked_add(timeout).unwrap_or(started);
        let _lifecycle = lock_until(self.lifecycle_lock(), deadline, operation, timeout)?;
        let _configuration = lock_until(self.configuration_lock(), deadline, operation, timeout)?;

        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();
        let (active, reconnecting) = run_with_obs!(runtime, (output_ptr), move || unsafe {
            // Safety: output_ptr is retained by the managed output handle.
            (
                libobs::obs_output_active(output_ptr.get_ptr()),
                libobs::obs_output_reconnecting(output_ptr.get_ptr()),
            )
        })?;

        if !active && !reconnecting {
            return Ok(());
        }

        // Both subscriptions are established before the request. This also covers outputs whose
        // stop/deactivate callbacks are emitted during the initiating actor call.
        let stop_receiver = self.signals().on_stop()?;
        let deactivate_receiver = self.signals().on_deactivate()?;

        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (output_ptr), move || unsafe {
            // Safety: output_ptr is retained by the managed output handle.
            if force {
                libobs::obs_output_force_stop(output_ptr.get_ptr());
            } else {
                libobs::obs_output_stop(output_ptr.get_ptr());
            }
        })?;

        let signal = wait_for_completion(&stop_receiver, deadline, operation, timeout)?;
        log::trace!("Received {operation} signal: {signal:?}");
        if signal != ObsOutputStopSignal::Success {
            return Err(ObsError::OutputStopFailure(Some(signal.to_string())));
        }

        wait_for_completion(&deactivate_receiver, deadline, operation, timeout)?;
        Ok(())
    }

    /// Returns whether the output is currently active.
    fn is_active(&self) -> Result<bool, ObsError> {
        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (output_ptr), move || unsafe {
            // Safety: output_ptr is retained by the managed output handle.
            libobs::obs_output_active(output_ptr.get_ptr())
        })
    }

    /// Returns whether the output is currently paused.
    fn is_paused(&self) -> Result<bool, ObsError> {
        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (output_ptr), move || unsafe {
            // Safety: output_ptr is retained by the managed output handle.
            libobs::obs_output_paused(output_ptr.get_ptr())
        })
    }

    /// Returns whether the output is currently reconnecting.
    fn is_reconnecting(&self) -> Result<bool, ObsError> {
        let output_ptr = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (output_ptr), move || unsafe {
            // Safety: output_ptr is retained by the managed output handle.
            libobs::obs_output_reconnecting(output_ptr.get_ptr())
        })
    }
}

fn output_supports_video_encoder(flags: u32) -> bool {
    flags & libobs::OBS_OUTPUT_ENCODED != 0 && flags & libobs::OBS_OUTPUT_VIDEO != 0
}

fn output_supports_audio_encoder(flags: u32) -> bool {
    flags & libobs::OBS_OUTPUT_ENCODED != 0 && flags & libobs::OBS_OUTPUT_AUDIO != 0
}

fn output_supports_service(flags: u32) -> bool {
    flags & libobs::OBS_OUTPUT_SERVICE != 0
}

fn unsupported_component<T: ObsOutputTrait + ?Sized>(output: &T, component: &str) -> ObsError {
    ObsError::OutputPipelineUnexpectedComponent {
        output_id: output.id().to_string(),
        component: component.to_owned(),
    }
}

fn ensure_output_supports_video_encoder<T: ObsOutputTrait + ?Sized>(
    output: &T,
    flags: u32,
) -> Result<(), ObsError> {
    output_supports_video_encoder(flags)
        .then_some(())
        .ok_or_else(|| unsupported_component(output, "a video encoder"))
}

fn ensure_output_supports_audio_encoder<T: ObsOutputTrait + ?Sized>(
    output: &T,
    flags: u32,
) -> Result<(), ObsError> {
    output_supports_audio_encoder(flags)
        .then_some(())
        .ok_or_else(|| unsupported_component(output, "audio encoders"))
}

fn ensure_output_supports_service<T: ObsOutputTrait + ?Sized>(
    output: &T,
    flags: u32,
) -> Result<(), ObsError> {
    output_supports_service(flags)
        .then_some(())
        .ok_or_else(|| unsupported_component(output, "a service"))
}

fn validate_audio_mixer(mixer_idx: usize) -> Result<(), ObsError> {
    if mixer_idx >= libobs::MAX_AUDIO_MIXES as usize {
        return Err(ObsError::AudioMixerIndexOutOfBounds {
            index: mixer_idx,
            max: (libobs::MAX_AUDIO_MIXES - 1) as usize,
        });
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
