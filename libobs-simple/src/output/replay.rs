//! Replay buffer builder for OBS.
//!
//! This module provides a simplified interface for configuring OBS replay buffers.
//! A replay buffer continuously records the last N seconds of content, allowing
//! on-demand saving of recent footage.
//!
//! # Example
//!
//! ```no_run
//! use libobs_simple::output::replay::ReplayBufferBuilder;
//! use libobs_wrapper::{context::ObsContext, utils::StartupInfo, data::video::ObsVideoInfoBuilder};
//!
//! #[tokio::main]
//! async fn main() {
//!     let context = StartupInfo::new()
//!         .set_video_info(
//!             ObsVideoInfoBuilder::new()
//!                 // Configure video info as needed
//!                 .build()
//!         ).start()
//!         .unwrap();
//!     
//!     let replay = ReplayBufferBuilder::new(context, "my_replay")
//!         .max_time_sec(30)
//!         .max_size_mb(1000)
//!         .format("%CCYY-%MM-%DD %hh-%mm-%ss")
//!         .extension("mp4")
//!         .build()
//!         .unwrap();
//!
//!     // Configure video and audio encoders on the replay buffer
//!     // Start the replay buffer
//!     // Call replay.save_buffer() when you want to save the buffer
//!
//!     println!("Replay buffer created!");
//! }
//! ```

use libobs_wrapper::{
    context::ObsContext,
    data::{
        output::{ObsOutputTrait, ObsReplayBufferOutputRef},
        ObsData, ObsDataGetters, ObsDataSetters,
    },
    encoders::{ObsAudioEncoderType, ObsContextEncoders, ObsVideoEncoderType},
    utils::{AudioEncoderInfo, ObsError, ObsPath, ObsString, OutputInfo, VideoEncoderInfo},
};

use super::simple::{AudioEncoder, HardwareCodec, HardwarePreset, VideoEncoder, X264Preset};

/// Settings for replay buffer output
#[derive(Debug)]
pub struct ReplayBufferSettings {
    name: ObsString,
    /// Maximum duration to keep in buffer (seconds)
    max_time_sec: i64,
    /// Maximum buffer size (megabytes)
    max_size_mb: i64,
    /// Filename format string (e.g., "%CCYY-%MM-%DD %hh-%mm-%ss")
    format: ObsString,
    /// File extension (e.g., "mp4", "mkv")
    extension: ObsString,
    /// Allow spaces in filenames
    allow_spaces: bool,
    video_encoder: VideoEncoder,
    audio_encoder: AudioEncoder,
    video_bitrate: u32,
    audio_bitrate: u32,
    directory: ObsPath,
    custom_encoder_settings: Option<String>,
}

impl ReplayBufferSettings {
    /// Sets the maximum time to keep in buffer (seconds).
    pub fn with_max_time_sec(mut self, seconds: i64) -> Self {
        self.max_time_sec = seconds;
        self
    }

    /// Sets the maximum buffer size (megabytes).
    pub fn with_max_size_mb(mut self, megabytes: i64) -> Self {
        self.max_size_mb = megabytes;
        self
    }

    /// Sets the filename format string.
    pub fn with_format<S: Into<ObsString>>(mut self, format: S) -> Self {
        self.format = format.into();
        self
    }

    /// Sets the file extension.
    pub fn with_extension<S: Into<ObsString>>(mut self, extension: S) -> Self {
        self.extension = extension.into();
        self
    }

    /// Sets whether to allow spaces in filenames.
    pub fn with_allow_spaces(mut self, allow: bool) -> Self {
        self.allow_spaces = allow;
        self
    }

    /// Sets the video bitrate in Kbps.
    pub fn with_video_bitrate(mut self, bitrate: u32) -> Self {
        self.video_bitrate = bitrate;
        self
    }

    /// Sets the audio bitrate in Kbps.
    pub fn with_audio_bitrate(mut self, bitrate: u32) -> Self {
        self.audio_bitrate = bitrate;
        self
    }

    /// Sets the video encoder to use x264 software encoding.
    pub fn with_x264_encoder(mut self, preset: X264Preset) -> Self {
        self.video_encoder = VideoEncoder::X264(preset);
        self
    }

    /// Sets the video encoder to use a generic hardware encoder for the given codec.
    pub fn with_hardware_encoder(mut self, codec: HardwareCodec, preset: HardwarePreset) -> Self {
        self.video_encoder = VideoEncoder::Hardware { codec, preset };
        self
    }

    /// Sets a custom video encoder.
    pub fn with_custom_video_encoder(mut self, encoder: ObsVideoEncoderType) -> Self {
        self.video_encoder = VideoEncoder::Custom(encoder);
        self
    }

    /// Sets custom encoder settings.
    pub fn with_custom_encoder_settings<S: Into<String>>(mut self, settings: S) -> Self {
        self.custom_encoder_settings = Some(settings.into());
        self
    }

    /// Sets the audio encoder.
    pub fn with_audio_encoder(mut self, encoder: AudioEncoder) -> Self {
        self.audio_encoder = encoder;
        self
    }
}

/// Builder for replay buffer outputs
#[derive(Debug)]
pub struct ReplayBufferBuilder {
    settings: ReplayBufferSettings,
    context: ObsContext,
}

/// Extension trait for ObsContext to create replay buffer builders
pub trait ObsContextReplayExt {
    fn replay_buffer_builder<T: Into<ObsString>, K: Into<ObsPath>>(
        &self,
        name: T,
        directory_path: K,
    ) -> ReplayBufferBuilder;
}

impl ObsContextReplayExt for ObsContext {
    fn replay_buffer_builder<T: Into<ObsString>, K: Into<ObsPath>>(
        &self,
        name: T,
        directory_path: K,
    ) -> ReplayBufferBuilder {
        ReplayBufferBuilder::new(self.clone(), name, directory_path)
    }
}

impl ReplayBufferBuilder {
    /// Creates a new ReplayBufferBuilder with default settings.
    pub fn new<T: Into<ObsString>, K: Into<ObsPath>>(
        context: ObsContext,
        name: T,
        directory_path: K,
    ) -> Self {
        ReplayBufferBuilder {
            settings: ReplayBufferSettings {
                name: name.into(),
                max_time_sec: 15,
                max_size_mb: 500,
                format: "%CCYY-%MM-%DD %hh-%mm-%ss".into(),
                extension: "mp4".into(),
                directory: directory_path.into(),
                allow_spaces: true,
                video_bitrate: 6000,
                audio_bitrate: 160,
                video_encoder: VideoEncoder::X264(X264Preset::VeryFast),
                audio_encoder: AudioEncoder::AAC,
                custom_encoder_settings: None,
            },
            context,
        }
    }

    /// Sets the replay buffer settings.
    pub fn settings(mut self, settings: ReplayBufferSettings) -> Self {
        self.settings = settings;
        self
    }

    /// Sets the maximum time to keep in buffer (seconds).
    pub fn max_time_sec(mut self, seconds: i64) -> Self {
        self.settings.max_time_sec = seconds;
        self
    }

    /// Sets the maximum buffer size (megabytes).
    pub fn max_size_mb(mut self, megabytes: i64) -> Self {
        self.settings.max_size_mb = megabytes;
        self
    }

    /// Sets the filename format string.
    pub fn format<S: Into<ObsString>>(mut self, format: S) -> Self {
        self.settings.format = format.into();
        self
    }

    /// Sets the file extension.
    pub fn extension<S: Into<ObsString>>(mut self, extension: S) -> Self {
        self.settings.extension = extension.into();
        self
    }

    /// Sets whether to allow spaces in filenames.
    pub fn allow_spaces(mut self, allow: bool) -> Self {
        self.settings.allow_spaces = allow;
        self
    }

    /// Sets the video bitrate in Kbps.
    pub fn video_bitrate(mut self, bitrate: u32) -> Self {
        self.settings.video_bitrate = bitrate;
        self
    }

    /// Sets the audio bitrate in Kbps.
    pub fn audio_bitrate(mut self, bitrate: u32) -> Self {
        self.settings.audio_bitrate = bitrate;
        self
    }

    /// Sets the video encoder to x264.
    pub fn x264_encoder(mut self, preset: X264Preset) -> Self {
        self.settings.video_encoder = VideoEncoder::X264(preset);
        self
    }

    /// Sets the video encoder to a generic hardware encoder.
    pub fn hardware_encoder(mut self, codec: HardwareCodec, preset: HardwarePreset) -> Self {
        self.settings.video_encoder = VideoEncoder::Hardware { codec, preset };
        self
    }

    /// Builds and returns the configured replay buffer output.
    pub fn build(mut self) -> Result<ObsReplayBufferOutputRef, ObsError> {
        if self.settings.max_size_mb <= 0 {
            return Err(ObsError::InvalidOperation(
                "max_size_mb must be greater than 0".into(),
            ));
        }

        if self.settings.max_time_sec <= 0 {
            return Err(ObsError::InvalidOperation(
                "max_time_sec must be greater than 0".into(),
            ));
        }

        // Create replay buffer settings
        let mut output_settings = self.context.data()?;
        output_settings.set_int("max_time_sec", self.settings.max_time_sec)?;
        output_settings.set_int("max_size_mb", self.settings.max_size_mb)?;
        output_settings.set_string("format", self.settings.format.clone())?;
        output_settings.set_string("extension", self.settings.extension.clone())?;
        output_settings.set_string("directory", self.settings.directory.clone().build())?;
        output_settings.set_bool("allow_spaces", self.settings.allow_spaces)?;

        log::trace!(
            "Replay buffer output settings: {:?}",
            output_settings.get_json()
        );

        // Create the replay buffer output
        let output_info = OutputInfo::new(
            "replay_buffer",
            self.settings.name.clone(),
            Some(output_settings),
            None,
        );

        let mut output = self.context.replay_buffer(output_info)?;

        // Create and configure video encoder (with hardware fallback)
        let video_encoder_type = self.select_video_encoder_type(&self.settings.video_encoder)?;
        let mut video_settings = self.context.data()?;

        self.configure_video_encoder(&mut video_settings)?;

        let video_encoder_info = VideoEncoderInfo::new(
            video_encoder_type,
            format!("{}_video_encoder", self.settings.name),
            Some(video_settings),
            None,
        );

        output.create_and_set_video_encoder(video_encoder_info)?;

        // Create and configure audio encoder
        let audio_encoder_type = match &self.settings.audio_encoder {
            AudioEncoder::AAC => ObsAudioEncoderType::FFMPEG_AAC,
            AudioEncoder::Opus => ObsAudioEncoderType::FFMPEG_OPUS,
            AudioEncoder::Custom(encoder_type) => encoder_type.clone(),
        };

        log::trace!("Selected audio encoder: {:?}", audio_encoder_type);
        let mut audio_settings = self.context.data()?;
        audio_settings.set_string("rate_control", "CBR")?;
        audio_settings.set_int("bitrate", self.settings.audio_bitrate as i64)?;

        let audio_encoder_info = AudioEncoderInfo::new(
            audio_encoder_type,
            format!("{}_audio_encoder", self.settings.name),
            Some(audio_settings),
            None,
        );

        log::trace!("Creating audio encoder with info: {:?}", audio_encoder_info);
        output.create_and_set_audio_encoder(audio_encoder_info, 0)?;

        Ok(output)
    }

    fn select_video_encoder_type(
        &self,
        encoder: &VideoEncoder,
    ) -> Result<ObsVideoEncoderType, ObsError> {
        match encoder {
            VideoEncoder::X264(_) => Ok(ObsVideoEncoderType::OBS_X264),
            VideoEncoder::Custom(t) => Ok(t.clone()),
            VideoEncoder::Hardware { codec, .. } => {
                // Build preferred candidates for the requested codec
                let candidates = self.hardware_candidates(*codec);
                // Query available encoders
                let available = self
                    .context
                    .available_video_encoders()?
                    .into_iter()
                    .map(|b| b.get_encoder_id().clone())
                    .collect::<Vec<_>>();
                // Pick first preferred candidate that is available
                for cand in candidates {
                    if available.iter().any(|a| a == &cand) {
                        return Ok(cand);
                    }
                }
                // Fallback to x264 if no hardware encoder is available
                Ok(ObsVideoEncoderType::OBS_X264)
            }
        }
    }

    fn hardware_candidates(&self, codec: HardwareCodec) -> Vec<ObsVideoEncoderType> {
        match codec {
            HardwareCodec::H264 => vec![
                ObsVideoEncoderType::OBS_NVENC_H264_TEX,
                ObsVideoEncoderType::H264_TEXTURE_AMF,
                ObsVideoEncoderType::OBS_QSV11_V2,
                ObsVideoEncoderType::OBS_NVENC_H264_SOFT,
                ObsVideoEncoderType::OBS_QSV11_SOFT_V2,
            ],
            HardwareCodec::HEVC => vec![
                ObsVideoEncoderType::OBS_NVENC_HEVC_TEX,
                ObsVideoEncoderType::H265_TEXTURE_AMF,
                ObsVideoEncoderType::OBS_QSV11_HEVC,
                ObsVideoEncoderType::OBS_NVENC_HEVC_SOFT,
                ObsVideoEncoderType::OBS_QSV11_HEVC_SOFT,
            ],
            HardwareCodec::AV1 => vec![
                ObsVideoEncoderType::OBS_NVENC_AV1_TEX,
                ObsVideoEncoderType::AV1_TEXTURE_AMF,
                ObsVideoEncoderType::OBS_QSV11_AV1,
                ObsVideoEncoderType::OBS_NVENC_AV1_SOFT,
                ObsVideoEncoderType::OBS_QSV11_AV1_SOFT,
            ],
        }
    }

    fn get_encoder_preset(&self, encoder: &VideoEncoder) -> Option<&str> {
        match encoder {
            VideoEncoder::X264(preset) => Some(preset.as_str()),
            VideoEncoder::Hardware { preset, .. } => Some(preset.as_str()),
            VideoEncoder::Custom(_) => None,
        }
    }

    fn configure_video_encoder(&self, settings: &mut ObsData) -> Result<(), ObsError> {
        // Set rate control to CBR
        settings.set_string("rate_control", "CBR")?;
        settings.set_int("bitrate", self.settings.video_bitrate as i64)?;

        // Set preset if available
        if let Some(preset) = self.get_encoder_preset(&self.settings.video_encoder) {
            settings.set_string("preset", preset)?;
        }

        // Apply custom encoder settings if provided
        if let Some(ref custom) = self.settings.custom_encoder_settings {
            settings.set_string("x264opts", custom.as_str())?;
        }

        Ok(())
    }
}
