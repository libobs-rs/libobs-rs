//! Simple output builder for OBS.
//!
//! This module provides a simplified interface for configuring OBS outputs
//! based on the SimpleOutput implementation from OBS Studio.
//!
//! # Example
//!
//! # Example
//!
//! ```no_run
//! use libobs_simple::output::simple::{SimpleOutputBuilder, X264Preset};
//! use libobs_simple::quick_start::quick_start;
//! use libobs_wrapper::{context::ObsContext, utils::StartupInfo, data::video::ObsVideoInfoBuilder};
//!
//! #[tokio::main]
//! async fn main() {
//! let context = StartupInfo::new()
//!     .set_video_info(
//!           ObsVideoInfoBuilder::new()
//!             // Configure video info as need
//!             .build()
//!      ).start()
//!       .unwrap()
//!     
//!     let output = SimpleOutputBuilder::new(context, "./recording.mp4")
//!         .video_bitrate(6000)
//!         .audio_bitrate(160)
//!         .x264_encoder(X264Preset::VeryFast)
//!         .build()
//!         .unwrap();
//!
//!     // Add sources here (for more docs, look [this](https://github.com/libobs-rs/libobs-rs/blob/main/examples/monitor-capture/src/main.rs) example
//!
//!     println!("Output created!");
//! }
//! ```

use libobs_wrapper::{
    context::ObsContext,
    data::{
        output::{ObsOutputRef, ObsOutputTrait},
        ObsData, ObsDataSetters,
    },
    encoders::{ObsAudioEncoderType, ObsContextEncoders, ObsVideoEncoderType},
    utils::{AudioEncoderInfo, ObsError, ObsPath, ObsString, OutputInfo, VideoEncoderInfo},
};

/// Preset for x264 software encoder
#[derive(Debug, Clone, Copy)]
pub enum X264Preset {
    /// Ultrafast preset - lowest CPU usage, largest file size
    UltraFast,
    /// Superfast preset
    SuperFast,
    /// Veryfast preset (recommended default)
    VeryFast,
    /// Faster preset
    Faster,
    /// Fast preset - higher CPU usage, better quality
    Fast,
    /// Medium preset
    Medium,
    /// Slow preset
    Slow,
    /// Slower preset
    Slower,
}

impl X264Preset {
    pub fn as_str(&self) -> &'static str {
        match self {
            X264Preset::UltraFast => "ultrafast",
            X264Preset::SuperFast => "superfast",
            X264Preset::VeryFast => "veryfast",
            X264Preset::Faster => "faster",
            X264Preset::Fast => "fast",
            X264Preset::Medium => "medium",
            X264Preset::Slow => "slow",
            X264Preset::Slower => "slower",
        }
    }
}

/// Preset for hardware encoders (NVENC, AMD, QSV)
#[derive(Debug, Clone, Copy)]
pub enum HardwarePreset {
    /// Prioritize encoding speed over quality
    Speed,
    /// Balance between speed and quality
    Balanced,
    /// Prioritize quality over speed
    Quality,
}

impl HardwarePreset {
    pub fn as_str(&self) -> &'static str {
        match self {
            HardwarePreset::Speed => "speed",
            HardwarePreset::Balanced => "balanced",
            HardwarePreset::Quality => "quality",
        }
    }
}

/// Video encoder configuration
#[derive(Debug, Clone)]
pub enum VideoEncoder {
    /// x264 software encoder
    X264(X264Preset),
    /// Hardware encoder (NVENC/AMF/QSV), codec chosen generically at runtime
    Hardware {
        codec: HardwareCodec,
        preset: HardwarePreset,
    },
    /// Custom encoder by type
    Custom(ObsVideoEncoderType),
}

/// Target codec for generic hardware selection
#[derive(Debug, Clone, Copy)]
pub enum HardwareCodec {
    H264,
    HEVC,
    AV1,
}

/// Audio encoder configuration
#[derive(Debug, Clone)]
pub enum AudioEncoder {
    /// AAC audio encoder (ffmpeg)
    AAC,
    /// Opus audio encoder
    Opus,
    /// Custom audio encoder by type
    Custom(ObsAudioEncoderType),
}

/// Output format for file recording
#[derive(Debug, Clone, Copy, Default)]
pub enum OutputFormat {
    /// .flv
    FlashVideo,
    /// .mkv
    MatroskaVideo,
    /// .mp4
    Mpeg4,
    /// .mov
    QuickTime,
    /// .mp4 (hybrid)
    #[default]
    HybridMP4,
    /// .mov (hybrid)
    HybridMov,
    /// .mp4 (fragmented)
    FragmentedMP4,
    /// .mov (fragmented)
    FragmentedMOV,
    /// MPEG-TS .ts
    MpegTs,
}

/// Unified output settings
#[derive(Debug)]
pub struct OutputSettings {
    name: ObsString,
    video_bitrate: u32,
    audio_bitrate: u32,
    video_encoder: VideoEncoder,
    audio_encoder: AudioEncoder,
    custom_encoder_settings: Option<String>,
    path: ObsPath,
    format: OutputFormat,
    custom_muxer_settings: Option<String>,
}

impl OutputSettings {
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
    /// The builder will choose an available backend (NVENC/AMF/QSV) at runtime.
    pub fn with_hardware_encoder(mut self, codec: HardwareCodec, preset: HardwarePreset) -> Self {
        self.video_encoder = VideoEncoder::Hardware { codec, preset };
        self
    }

    /// Sets a custom video encoder.
    pub fn with_custom_video_encoder(mut self, encoder: ObsVideoEncoderType) -> Self {
        self.video_encoder = VideoEncoder::Custom(encoder);
        self
    }

    /// Sets custom x264 encoder settings.
    pub fn with_custom_settings<S: Into<String>>(mut self, settings: S) -> Self {
        self.custom_encoder_settings = Some(settings.into());
        self
    }

    /// Sets the output path.
    pub fn with_path<P: Into<ObsPath>>(mut self, path: P) -> Self {
        self.path = path.into();
        self
    }

    /// Sets the output format.
    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = format;
        self
    }

    /// Sets custom muxer settings.
    pub fn with_custom_muxer_settings<S: Into<String>>(mut self, settings: S) -> Self {
        self.custom_muxer_settings = Some(settings.into());
        self
    }

    /// Sets the audio encoder.
    pub fn with_audio_encoder(mut self, encoder: AudioEncoder) -> Self {
        self.audio_encoder = encoder;
        self
    }
}

#[derive(Debug)]
pub struct SimpleOutputBuilder {
    settings: OutputSettings,
    context: ObsContext,
}

pub trait ObsContextSimpleExt {
    fn simple_output_builder<K: Into<ObsPath>, T: Into<ObsString>>(
        &self,
        name: T,
        path: K,
    ) -> SimpleOutputBuilder;
}

impl ObsContextSimpleExt for ObsContext {
    fn simple_output_builder<K: Into<ObsPath>, T: Into<ObsString>>(
        &self,
        name: T,
        path: K,
    ) -> SimpleOutputBuilder {
        SimpleOutputBuilder::new(self.clone(), name, path)
    }
}

impl SimpleOutputBuilder {
    /// Creates a new SimpleOutputBuilder with default settings.
    pub fn new<K: Into<ObsPath>, T: Into<ObsString>>(
        context: ObsContext,
        name: T,
        path: K,
    ) -> Self {
        SimpleOutputBuilder {
            settings: OutputSettings {
                video_bitrate: 6000,
                audio_bitrate: 160,
                video_encoder: VideoEncoder::X264(X264Preset::VeryFast),
                audio_encoder: AudioEncoder::AAC,
                custom_encoder_settings: None,
                path: path.into(),
                format: OutputFormat::default(),
                custom_muxer_settings: None,
                name: name.into(),
            },
            context,
        }
    }

    /// Sets the output settings.
    pub fn settings(mut self, settings: OutputSettings) -> Self {
        self.settings = settings;
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

    /// Sets the output path.
    pub fn path<P: Into<ObsPath>>(mut self, path: P) -> Self {
        self.settings.path = path.into();
        self
    }

    /// Sets the output format.
    pub fn format(mut self, format: OutputFormat) -> Self {
        self.settings.format = format;
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

    /// Builds and returns the configured output.
    pub fn build(mut self) -> Result<ObsOutputRef, ObsError> {
        // Determine the output type based on format
        let output_id = match self.settings.format {
            OutputFormat::HybridMP4 => "mp4_output",
            OutputFormat::HybridMov => "mov_output",
            _ => "ffmpeg_muxer",
        };

        // Create output settings
        let mut output_settings = self.context.data()?;
        output_settings.set_string("path", self.settings.path.clone().build())?;

        if let Some(ref muxer_settings) = self.settings.custom_muxer_settings {
            output_settings.set_string("muxer_settings", muxer_settings.as_str())?;
        }

        // Create the output
        let output_info = OutputInfo::new(
            output_id,
            self.settings.name.clone(),
            Some(output_settings),
            None,
        );

        let mut output = self.context.output(output_info)?;

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
                // software fallbacks for vendor SDKs
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

        // Apply custom encoder settings if provided (mainly for x264)
        if let Some(ref custom) = self.settings.custom_encoder_settings {
            settings.set_string("x264opts", custom.as_str())?;
        }

        Ok(())
    }
}
