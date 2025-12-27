use std::sync::{Arc, RwLock};
use std::{ffi::CStr, ptr};

use getters0::Getters;
use libobs::obs_output;

use crate::enums::ObsOutputStopSignal;
use crate::runtime::ObsRuntime;
use crate::unsafe_send::Sendable;
use crate::utils::{AudioEncoderInfo, OutputInfo, VideoEncoderInfo};
use crate::{impl_obs_drop, impl_signal_manager, run_with_obs};

use crate::{
    encoders::{audio::ObsAudioEncoder, video::ObsVideoEncoder},
    utils::{ObsError, ObsString},
};

use super::ObsData;

mod replay_buffer;
pub use replay_buffer::*;

#[derive(Debug)]
struct _ObsOutputDropGuard {
    output: Sendable<*mut obs_output>,
    runtime: ObsRuntime,
}

impl_obs_drop!(_ObsOutputDropGuard, (output), move || unsafe {
    libobs::obs_output_release(output);
});

#[derive(Debug, Getters, Clone)]
#[skip_new]
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
pub struct ObsOutputRef {
    /// Disconnect signals first
    pub(crate) signal_manager: Arc<ObsOutputSignals>,

    /// Settings for the output
    pub(crate) settings: Arc<RwLock<Option<ObsData>>>,

    /// Hotkey configuration data for the output
    pub(crate) hotkey_data: Arc<RwLock<Option<ObsData>>>,

    /// Video encoders attached to this output
    #[get_mut]
    pub(crate) curr_video_encoder: Arc<RwLock<Option<Arc<ObsVideoEncoder>>>>,

    /// Audio encoders attached to this output
    #[get_mut]
    pub(crate) audio_encoders: Arc<RwLock<Option<Arc<ObsAudioEncoder>>>>,

    /// Pointer to the underlying OBS output
    #[skip_getter]
    pub(crate) output: Sendable<*mut obs_output>,

    /// The type identifier of this output
    pub(crate) id: ObsString,

    /// The unique name of this output
    pub(crate) name: ObsString,

    #[skip_getter]
    pub(crate) runtime: ObsRuntime,

    /// RAII guard that ensures proper cleanup when the output is dropped
    #[skip_getter]
    _drop_guard: Arc<_ObsOutputDropGuard>,
}

impl ObsOutputRef {
    /// Creates a new output reference from the given output info and runtime.
    ///
    /// # Arguments
    /// * `output` - The output information containing ID, name, and optional settings
    /// * `runtime` - The OBS runtime instance
    ///
    /// # Returns
    /// A Result containing the new ObsOutputRef or an error
    pub(crate) fn new(output: OutputInfo, runtime: ObsRuntime) -> Result<Self, ObsError> {
        let (output, id, name, settings, hotkey_data) = runtime.run_with_obs_result(|| {
            let OutputInfo {
                id,
                name,
                settings,
                hotkey_data,
            } = output;

            let settings_ptr = match settings.as_ref() {
                Some(x) => x.as_ptr(),
                None => Sendable(ptr::null_mut()),
            };

            let hotkey_data_ptr = match hotkey_data.as_ref() {
                Some(x) => x.as_ptr(),
                None => Sendable(ptr::null_mut()),
            };

            let output = unsafe {
                libobs::obs_output_create(
                    id.as_ptr().0,
                    name.as_ptr().0,
                    settings_ptr.0,
                    hotkey_data_ptr.0,
                )
            };

            (Sendable(output), id, name, settings, hotkey_data)
        })?;

        if output.0.is_null() {
            return Err(ObsError::NullPointer);
        }

        let signal_manager = ObsOutputSignals::new(&output, runtime.clone())?;
        Ok(Self {
            settings: Arc::new(RwLock::new(settings)),
            hotkey_data: Arc::new(RwLock::new(hotkey_data)),

            curr_video_encoder: Arc::new(RwLock::new(None)),
            audio_encoders: Arc::new(RwLock::new(None)),

            output: output.clone(),
            id,
            name,

            _drop_guard: Arc::new(_ObsOutputDropGuard {
                output,
                runtime: runtime.clone(),
            }),

            runtime,
            signal_manager: Arc::new(signal_manager),
        })
    }

    /// Returns the current video encoder attached to this output, if any.
    pub fn get_current_video_encoder(&self) -> Result<Option<Arc<ObsVideoEncoder>>, ObsError> {
        let curr = self
            .curr_video_encoder
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))?;

        Ok(curr.clone())
    }

    /// Creates and attaches a new audio encoder to this output.
    ///
    /// This method creates a new audio encoder using the provided information
    ///  and attaches it to this output at the specified mixer index.
    ///
    /// # Arguments
    /// * `info` - Information for creating the audio encoder
    /// * `mixer_idx` - The mixer index to use (typically 0 for primary audio)
    ///
    /// # Returns
    /// A Result containing an Arc-wrapped ObsAudioEncoder or an error
    pub fn create_and_set_video_encoder(
        &mut self,
        info: VideoEncoderInfo,
    ) -> Result<Arc<ObsVideoEncoder>, ObsError> {
        // Fail early before creating the encoder if the output is active
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        let video_enc = ObsVideoEncoder::new_from_info(info, self.runtime.clone())?;

        self.set_video_encoder(video_enc.clone())?;
        Ok(video_enc)
    }

    /// Attaches an existing video encoder to this output.
    ///
    /// # Arguments
    /// * `encoder` - The video encoder to attach
    ///
    /// # Returns
    /// A Result indicating success or an error
    pub fn set_video_encoder(&mut self, encoder: Arc<ObsVideoEncoder>) -> Result<(), ObsError> {
        if encoder.encoder.0.is_null() {
            return Err(ObsError::NullPointer);
        }

        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        let output = self.output.clone();
        let encoder_ptr = encoder.as_ptr();

        run_with_obs!(self.runtime, (output, encoder_ptr), move || unsafe {
            libobs::obs_output_set_video_encoder(output, encoder_ptr);
        })?;

        self.curr_video_encoder
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?
            .replace(encoder);

        Ok(())
    }

    /// Updates the settings of this output.
    ///
    /// Note: This can only be done when the output is not active.
    ///
    /// # Arguments
    /// * `settings` - The new settings to apply
    ///
    /// # Returns
    /// A Result indicating success or an error
    pub fn update_settings(&mut self, settings: ObsData) -> Result<(), ObsError> {
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        let settings_ptr = settings.as_ptr();
        let output = self.output.clone();

        run_with_obs!(self.runtime, (output, settings_ptr), move || unsafe {
            libobs::obs_output_update(output, settings_ptr)
        })?;

        self.settings
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?
            .replace(settings);
        Ok(())
    }

    /// Creates and attaches a new audio encoder to this output.
    ///
    /// This method creates a new audio encoder using the provided information,
    /// sets up the audio handler, and attaches it to this output at the specified mixer index.
    ///
    /// # Arguments
    /// * `info` - Information for creating the audio encoder
    /// * `mixer_idx` - The mixer index to use (typically 0 for primary audio)
    /// * `handler` - The audio output handler
    ///
    /// # Returns
    /// A Result containing an Arc-wrapped ObsAudioEncoder or an error
    pub fn create_and_set_audio_encoder(
        &mut self,
        info: AudioEncoderInfo,
        mixer_idx: usize,
    ) -> Result<Arc<ObsAudioEncoder>, ObsError> {
        // Fail early before creating the encoder if the output is active
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        let audio_enc = ObsAudioEncoder::new_from_info(info, mixer_idx, self.runtime.clone())?;
        self.set_audio_encoder(audio_enc.clone(), mixer_idx)?;
        Ok(audio_enc)
    }

    /// Attaches an existing audio encoder to this output at the specified mixer index.
    ///
    /// # Arguments
    /// * `encoder` - The audio encoder to attach
    /// * `mixer_idx` - The mixer index to use (typically 0 for primary audio)
    ///
    /// # Returns
    /// A Result indicating success or an error
    pub fn set_audio_encoder(
        &mut self,
        encoder: Arc<ObsAudioEncoder>,
        mixer_idx: usize,
    ) -> Result<(), ObsError> {
        if encoder.encoder.0.is_null() {
            return Err(ObsError::NullPointer);
        }

        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        let encoder_ptr = encoder.encoder.clone();
        let output_ptr = self.output.clone();
        run_with_obs!(self.runtime, (output_ptr, encoder_ptr), move || unsafe {
            libobs::obs_output_set_audio_encoder(output_ptr, encoder_ptr, mixer_idx)
        })?;

        //TODO don't drop audio encoders if multiple are set
        self.audio_encoders
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))?
            .replace(encoder);

        Ok(())
    }

    /// Starts the output.
    ///
    /// This begins the encoding and streaming/recording process.
    ///
    /// # Returns
    /// A Result indicating success or an error (e.g., if the output is already active)
    pub fn start(&self) -> Result<(), ObsError> {
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        // Set the video and audio encoders before starting (similar to https://github.com/obsproject/obs-studio/blob/0b1229632063a13dfd26cf1cd9dd43431d8c68f6/frontend/utility/SimpleOutput.cpp#L552)
        let vid_encoder_ptr = self
            .curr_video_encoder
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))?
            .as_ref()
            .map(|enc| enc.as_ptr())
            .unwrap_or(Sendable(ptr::null_mut()));

        let audio_encoder_ptr = self
            .audio_encoders
            .read()
            .map_err(|e| ObsError::LockError(e.to_string()))?
            .as_ref()
            .map(|enc| enc.encoder.clone())
            .unwrap_or(Sendable(ptr::null_mut()));

        let output_ptr = self.output.clone();
        let res = run_with_obs!(
            self.runtime,
            (output_ptr, vid_encoder_ptr, audio_encoder_ptr),
            move || unsafe {
                libobs::obs_encoder_set_video(vid_encoder_ptr, libobs::obs_get_video());
                libobs::obs_encoder_set_audio(audio_encoder_ptr, libobs::obs_get_audio());
                libobs::obs_output_start(output_ptr)
            }
        )?;

        if res {
            return Ok(());
        }

        let err = run_with_obs!(self.runtime, (output_ptr), move || unsafe {
            Sendable(libobs::obs_output_get_last_error(output_ptr))
        })?;

        if err.0.is_null() {
            return Err(ObsError::OutputStartFailure(None));
        }

        let c_str = unsafe { CStr::from_ptr(err.0) };
        let err_str = c_str.to_str().ok().map(|x| x.to_string());

        Err(ObsError::OutputStartFailure(err_str))
    }

    /// This pauses or resumes the given output, and waits until the output is fully paused.
    ///
    /// # Arguments
    ///
    /// * `pause` - `true` to pause the output, `false` to resume the output.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - The output was paused or resumed successfully.
    /// * `Err(ObsError::OutputPauseFailure(Some(String)))` - The output failed to pause or resume.
    pub fn pause(&self, pause: bool) -> Result<(), ObsError> {
        if !self.is_active()? {
            return Err(ObsError::OutputPauseFailure(Some(
                "Output is not active.".to_string(),
            )));
        }

        let output_ptr = self.output.clone();

        let mut rx = if pause {
            self.signal_manager.on_pause()?
        } else {
            self.signal_manager.on_unpause()?
        };

        let res = run_with_obs!(self.runtime, (output_ptr), move || unsafe {
            libobs::obs_output_pause(output_ptr, pause)
        })?;

        if res {
            rx.blocking_recv().map_err(|_| ObsError::NoSenderError)?;

            Ok(())
        } else {
            let err = run_with_obs!(self.runtime, (output_ptr), move || unsafe {
                Sendable(libobs::obs_output_get_last_error(output_ptr))
            })?;

            let c_str = unsafe { CStr::from_ptr(err.0) };
            let err_str = c_str.to_str().ok().map(|x| x.to_string());

            Err(ObsError::OutputPauseFailure(err_str))
        }
    }

    /// Stops the output.
    ///
    /// This ends the encoding and streaming/recording process.
    /// The method waits for a stop signal and returns the result.
    ///
    /// # Returns
    /// A Result indicating success or an error with details about why stopping failed
    //TODO There should be some kind of "wait" for other methods to finish, generally we don't want to have multiple different methods calling methods
    pub fn stop(&mut self) -> Result<(), ObsError> {
        let output_ptr = self.output.clone();
        let output_active = run_with_obs!(self.runtime, (output_ptr), move || unsafe {
            libobs::obs_output_active(output_ptr)
        })?;

        if !output_active {
            return Err(ObsError::OutputStopFailure(Some(
                "Output is not active.".to_string(),
            )));
        }

        let mut rx = self.signal_manager.on_stop()?;
        let mut rx_deactivate = self.signal_manager.on_deactivate()?;

        run_with_obs!(self.runtime, (output_ptr), move || unsafe {
            libobs::obs_output_stop(output_ptr)
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

    pub fn is_active(&self) -> Result<bool, ObsError> {
        let output_ptr = self.output.clone();
        let output_active = run_with_obs!(self.runtime, (output_ptr), move || unsafe {
            libobs::obs_output_active(output_ptr)
        })?;

        Ok(output_active)
    }

    pub fn as_ptr(&self) -> Sendable<*mut obs_output> {
        self.output.clone()
    }
}

impl_signal_manager!(|ptr| unsafe { libobs::obs_output_get_signal_handler(ptr) }, ObsOutputSignals for ObsOutputRef<*mut libobs::obs_output>, [
    "start": {},
    "stop": {code: crate::enums::ObsOutputStopSignal},
    "pause": {},
    "unpause": {},
    "starting": {},
    "stopping": {},
    "activate": {},
    "deactivate": {},
    "reconnect": {},
    "reconnect_success": {},
    /// Only available for replay buffer outputs
    "saved": {}
]);
