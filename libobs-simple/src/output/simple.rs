//! Simple output builder for OBS.
//!
//! This module provides a simplified interface for configuring OBS outputs
//! based on the SimpleOutput implementation from OBS Studio.
//!
//! # Example
//!
//! ```no_run
//! use libobs_simple::output::simple::{SimpleOutputBuilder, X264Preset};
//! use libobs_wrapper::{data::video::ObsVideoInfoBuilder, utils::{ObsPath, StartupInfo}};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let context = StartupInfo::new()
//!         .set_video_info(ObsVideoInfoBuilder::new().build())
//!         .start()?;
//!
//!     let _output = SimpleOutputBuilder::new(context, "recording", ObsPath::new("./recording.mp4"))
//!         .video_bitrate(6000)
//!         .audio_bitrate(160)
//!         .x264_encoder(X264Preset::VeryFast)
//!         .build()?;
//!
//!     Ok(())
//! }
//! ```

use libobs_wrapper::{
    capabilities::{
        EncoderTypeInfo, ObsCapabilities, OutputCapabilities, OutputCompatibilityRequest,
    },
    context::ObsContext,
    data::{output::ObsOutputRef, ObsData, ObsDataSetters},
    encoders::{ObsAudioEncoderType, ObsVideoEncoderType},
    settings::PropertyValue,
    utils::{ObsError, ObsPath, ObsString},
};

use super::configure::set_if_supported;

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
    /// Automatically select an encoder for this codec, preferring hardware and falling back
    /// to a software implementation when the current OBS installation has no hardware option.
    Auto(HardwareCodec),
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

impl HardwareCodec {
    fn codec_name(self) -> &'static str {
        match self {
            Self::H264 => "h264",
            Self::HEVC => "hevc",
            Self::AV1 => "av1",
        }
    }
}

fn video_encoder_descriptor(
    capabilities: &ObsCapabilities,
    encoder: &VideoEncoder,
) -> Result<EncoderTypeInfo, ObsError> {
    match encoder {
        VideoEncoder::Auto(codec) | VideoEncoder::Hardware { codec, .. } => capabilities
            .select_video_encoder()
            .codec(codec.codec_name())
            .prefer_hardware()
            .best_available()
            .cloned()
            .ok_or(ObsError::NoAvailableEncoders),
        VideoEncoder::X264(_) => capabilities
            .encoders()
            .iter()
            .find(|candidate| candidate.id() == "obs_x264")
            .cloned()
            .ok_or(ObsError::NoAvailableEncoders),
        VideoEncoder::Custom(encoder_type) => {
            let id = ObsString::from(encoder_type.clone()).to_string();
            capabilities
                .encoders()
                .iter()
                .find(|candidate| candidate.id() == id)
                .cloned()
                .ok_or(ObsError::NoAvailableEncoders)
        }
    }
}

pub(crate) fn select_video_encoder_type(
    context: &ObsContext,
    encoder: &VideoEncoder,
) -> Result<ObsVideoEncoderType, ObsError> {
    let capabilities = context.capabilities()?;
    let descriptor = video_encoder_descriptor(&capabilities, encoder)?;
    Ok(ObsVideoEncoderType::from(descriptor.id()))
}

fn audio_encoder_descriptor(
    capabilities: &ObsCapabilities,
    encoder: &AudioEncoder,
) -> Result<EncoderTypeInfo, ObsError> {
    if let AudioEncoder::Custom(encoder_type) = encoder {
        let id = ObsString::from(encoder_type.clone()).to_string();
        return capabilities
            .encoders()
            .iter()
            .find(|candidate| candidate.id() == id)
            .cloned()
            .ok_or(ObsError::NoAvailableEncoders);
    }

    let codec = match encoder {
        AudioEncoder::AAC => "aac",
        AudioEncoder::Opus => "opus",
        AudioEncoder::Custom(_) => unreachable!(),
    };
    capabilities
        .select_audio_encoder()
        .codec(codec)
        .best_available()
        .cloned()
        .ok_or(ObsError::NoAvailableEncoders)
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
    /// Quality for CRF-based x264 encoding, on a 0–100 scale.
    crf: Option<u32>,
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

    /// Sets a quality target for x264 encoding on a 0–100 scale.
    ///
    /// A value of 100 maps to x264 CRF 0 (highest quality), while 0 maps to
    /// CRF 51 (lowest quality). Hardware encoders retain bitrate-based control.
    pub fn with_crf(mut self, crf: u32) -> Self {
        self.crf = Some(crf.min(100));
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
                video_encoder: VideoEncoder::Auto(HardwareCodec::H264),
                audio_encoder: AudioEncoder::AAC,
                custom_encoder_settings: None,
                path: path.into(),
                format: OutputFormat::default(),
                custom_muxer_settings: None,
                crf: None,
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

    /// Uses automatic codec-based encoder selection. Hardware implementations are preferred,
    /// while software encoders remain a transparent fallback.
    pub fn auto_video_encoder(mut self, codec: HardwareCodec) -> Self {
        self.settings.video_encoder = VideoEncoder::Auto(codec);
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

    /// Sets a quality target for x264 encoding on a 0–100 scale.
    ///
    /// A value of 100 maps to x264 CRF 0 (highest quality), while 0 maps to
    /// CRF 51 (lowest quality). Hardware encoders retain bitrate-based control.
    pub fn crf(mut self, crf: u32) -> Self {
        self.settings.crf = Some(crf.min(100));
        self
    }

    /// Builds a validated recording graph using the concrete capabilities available at runtime.
    pub fn build(self) -> Result<ObsOutputRef, ObsError> {
        let output_id = match self.settings.format {
            OutputFormat::HybridMP4 => "mp4_output",
            OutputFormat::HybridMov => "mov_output",
            _ => "ffmpeg_muxer",
        };

        let capabilities = self.context.capabilities()?;
        let video_type = video_encoder_descriptor(&capabilities, &self.settings.video_encoder)?;
        let audio_type = audio_encoder_descriptor(&capabilities, &self.settings.audio_encoder)?;

        let mut compatibility = OutputCompatibilityRequest::new()
            .output_id(output_id)
            .require_output_capabilities(
                OutputCapabilities::ENCODED | OutputCapabilities::VIDEO | OutputCapabilities::AUDIO,
            );
        if let Some(codec) = video_type.codec() {
            compatibility = compatibility.video_codec(codec);
        }
        if let Some(codec) = audio_type.codec() {
            compatibility = compatibility.audio_codec(codec);
        }
        let output_type = capabilities
            .best_output_plan(&compatibility)
            .map_err(|report| ObsError::NoCompatibleOutputGraph {
                summary: report.summary(),
            })?
            .output()
            .clone();

        let mut output_settings = output_type.default_settings_mut()?;
        output_settings.set_string("path", self.settings.path.clone().build())?;
        if let Some(ref muxer_settings) = self.settings.custom_muxer_settings {
            output_settings.set_string("muxer_settings", muxer_settings.as_str())?;
        }

        let mut video_settings = video_type.default_settings_mut()?;
        self.configure_video_encoder(&video_type, &mut video_settings)?;
        let video_encoder = self.context.create_video_encoder(
            &video_type,
            format!("{}_video_encoder", self.settings.name),
            Some(video_settings),
        )?;

        let mut audio_settings = audio_type.default_settings_mut()?;
        let audio_schema = audio_type.settings_schema_for(&audio_settings)?;
        set_if_supported(
            &audio_schema,
            &mut audio_settings,
            "rate_control",
            PropertyValue::String("CBR".into()),
        )?;
        set_if_supported(
            &audio_schema,
            &mut audio_settings,
            "bitrate",
            PropertyValue::Integer(i64::from(self.settings.audio_bitrate)),
        )?;
        let audio_encoder = self.context.create_audio_encoder(
            &audio_type,
            format!("{}_audio_encoder", self.settings.name),
            Some(audio_settings),
            0,
        )?;

        Ok(self
            .context
            .output_pipeline(
                &output_type,
                self.settings.name.clone(),
                Some(output_settings),
            )
            .video_encoder(video_encoder)
            .audio_encoder(0, audio_encoder)
            .build()?
            .into_output())
    }

    fn get_encoder_preset(&self, encoder: &VideoEncoder) -> Option<&str> {
        match encoder {
            VideoEncoder::Auto(_) => None,
            VideoEncoder::X264(preset) => Some(preset.as_str()),
            VideoEncoder::Hardware { preset, .. } => Some(preset.as_str()),
            VideoEncoder::Custom(_) => None,
        }
    }

    fn x264_crf_from_quality(quality: u32) -> i64 {
        (100u32.saturating_sub(quality.min(100)) * 51 / 100) as i64
    }

    fn uses_x264(encoder_type: &EncoderTypeInfo) -> bool {
        encoder_type.id() == "obs_x264"
    }

    fn configure_video_encoder(
        &self,
        encoder_type: &EncoderTypeInfo,
        settings: &mut ObsData,
    ) -> Result<(), ObsError> {
        let schema = encoder_type.settings_schema_for(settings)?;
        let use_crf = self.settings.crf.is_some() && Self::uses_x264(encoder_type);
        if use_crf {
            set_if_supported(
                &schema,
                settings,
                "rate_control",
                PropertyValue::String("CRF".into()),
            )?;
            if let Some(quality) = self.settings.crf {
                set_if_supported(
                    &schema,
                    settings,
                    "crf",
                    PropertyValue::Integer(Self::x264_crf_from_quality(quality)),
                )?;
            }
        } else {
            set_if_supported(
                &schema,
                settings,
                "rate_control",
                PropertyValue::String("CBR".into()),
            )?;
            set_if_supported(
                &schema,
                settings,
                "bitrate",
                PropertyValue::Integer(i64::from(self.settings.video_bitrate)),
            )?;
            if self.settings.crf.is_some() {
                log::warn!(
                    "CRF is only supported by the selected x264 encoder; using CBR at {} Kbps instead",
                    self.settings.video_bitrate
                );
            }
        }
        if let Some(preset) = self.get_encoder_preset(&self.settings.video_encoder) {
            set_if_supported(
                &schema,
                settings,
                "preset",
                PropertyValue::String(preset.into()),
            )?;
        }
        if let Some(ref custom) = self.settings.custom_encoder_settings {
            set_if_supported(
                &schema,
                settings,
                "x264opts",
                PropertyValue::String(custom.clone()),
            )?;
        }
        Ok(())
    }
}
