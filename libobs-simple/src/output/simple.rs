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

/// Preset for hardware encoders (NVENC, AMF, QSV, VAAPI, and VideoToolbox).
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
    /// Hardware encoder, with the backend chosen from the encoders registered at runtime.
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
    /// Quality for CRF-based x264 encoding, on a 0–100 scale.
    crf: Option<u32>,
}

impl OutputSettings {
    /// Sets the video bitrate in Kbps.
    pub fn with_video_bitrate(mut self, bitrate: u32) -> Self {
        self.video_bitrate = bitrate;
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
    /// The builder will choose an available backend at runtime.
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

    /// Sets a quality target for x264 encoding on a 0–100 scale.
    ///
    /// A value of 100 maps to x264 CRF 0 (highest quality), while 0 maps to
    /// CRF 51 (lowest quality). Hardware encoders retain bitrate-based control.
    pub fn crf(mut self, crf: u32) -> Self {
        self.settings.crf = Some(crf.min(100));
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

        self.configure_video_encoder(&mut video_settings, &video_encoder_type)?;

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
                let mut available = Vec::new();
                for builder in self.context.available_video_encoders()? {
                    available.push((
                        builder.get_encoder_id().clone(),
                        builder.get_encoder_codec()?,
                        builder.get_encoder_display_name()?,
                    ));
                }

                // OBS' mac-videotoolbox plugin registers the IDs returned by
                // VTCopyVideoEncoderList(), so the exact ID is machine/OS specific.
                // Match the dynamic ID and libobs codec metadata instead of baking
                // in an Intel or Apple-Silicon encoder constant.
                if let Some(videotoolbox) = Self::select_videotoolbox_encoder(*codec, &available) {
                    return Ok(videotoolbox);
                }

                for candidate in Self::hardware_candidates(*codec) {
                    if available.iter().any(|(id, _, _)| id == &candidate) {
                        return Ok(candidate);
                    }
                }

                // A hardware request is best-effort. x264 remains the portable
                // fallback, but its settings must be configured as x264 below.
                Ok(ObsVideoEncoderType::OBS_X264)
            }
        }
    }

    fn hardware_candidates(codec: HardwareCodec) -> Vec<ObsVideoEncoderType> {
        match codec {
            HardwareCodec::H264 => vec![
                ObsVideoEncoderType::OBS_NVENC_H264_TEX,
                ObsVideoEncoderType::H264_TEXTURE_AMF,
                ObsVideoEncoderType::OBS_QSV11_V2,
                // Linux VAAPI: prefer texture/zero-copy over the generic path.
                ObsVideoEncoderType::FFMPEG_VAAPI_TEX,
                ObsVideoEncoderType::FFMPEG_VAAPI,
                // software fallbacks for vendor SDKs
                ObsVideoEncoderType::OBS_NVENC_H264_SOFT,
                ObsVideoEncoderType::OBS_QSV11_SOFT_V2,
            ],
            HardwareCodec::HEVC => vec![
                ObsVideoEncoderType::OBS_NVENC_HEVC_TEX,
                ObsVideoEncoderType::H265_TEXTURE_AMF,
                ObsVideoEncoderType::OBS_QSV11_HEVC,
                ObsVideoEncoderType::HEVC_FFMPEG_VAAPI_TEX,
                ObsVideoEncoderType::HEVC_FFMPEG_VAAPI,
                ObsVideoEncoderType::OBS_NVENC_HEVC_SOFT,
                ObsVideoEncoderType::OBS_QSV11_HEVC_SOFT,
            ],
            HardwareCodec::AV1 => vec![
                ObsVideoEncoderType::OBS_NVENC_AV1_TEX,
                ObsVideoEncoderType::AV1_TEXTURE_AMF,
                ObsVideoEncoderType::OBS_QSV11_AV1,
                ObsVideoEncoderType::AV1_FFMPEG_VAAPI_TEX,
                ObsVideoEncoderType::AV1_FFMPEG_VAAPI,
                ObsVideoEncoderType::OBS_NVENC_AV1_SOFT,
                ObsVideoEncoderType::OBS_QSV11_AV1_SOFT,
            ],
        }
    }

    fn codec_name(codec: HardwareCodec) -> &'static str {
        match codec {
            HardwareCodec::H264 => "h264",
            HardwareCodec::HEVC => "hevc",
            HardwareCodec::AV1 => "av1",
        }
    }

    fn is_videotoolbox_encoder(encoder: &ObsVideoEncoderType) -> bool {
        matches!(
            encoder,
            ObsVideoEncoderType::Other(id) if id.to_ascii_lowercase().contains("videotoolbox")
        )
    }

    fn videotoolbox_hardware_rank(id: &str, display_name: Option<&str>) -> u8 {
        let id = id.to_ascii_lowercase();
        let display_name = display_name.unwrap_or_default().to_ascii_lowercase();
        // OBS receives an explicit IsHardwareAccelerated bit from VideoToolbox,
        // but libobs' generic encoder metadata API does not expose that type-data
        // field. Use both dynamically enumerated ID and display-name signals only
        // for ranking; neither is used as a hard-coded candidate ID.
        if id.contains(".ave.")
            || id.ends_with(".gva")
            || id.contains(".hardware")
            || display_name.contains("(hw)")
            || display_name.contains("hardware")
        {
            2
        } else if id.contains("software")
            || id.ends_with(".vcp")
            || id.ends_with(".sw")
            || display_name.contains("(sw)")
            || display_name.contains("software")
        {
            0
        } else {
            // libobs does not expose VideoToolbox's IsHardwareAccelerated bit
            // through generic encoder metadata. Treat unknown IDs as unknown
            // rather than silently satisfying an explicit Hardware request with
            // an encoder that could be software-only.
            1
        }
    }

    fn select_videotoolbox_encoder(
        codec: HardwareCodec,
        available: &[(ObsVideoEncoderType, Option<String>, Option<String>)],
    ) -> Option<ObsVideoEncoderType> {
        if matches!(codec, HardwareCodec::AV1) {
            return None;
        }
        let codec_name = Self::codec_name(codec);
        available
            .iter()
            .filter_map(|(encoder, advertised_codec, display_name)| {
                let ObsVideoEncoderType::Other(id) = encoder else {
                    return None;
                };
                if !id.to_ascii_lowercase().contains("videotoolbox")
                    || advertised_codec
                        .as_deref()
                        .is_none_or(|value| !value.eq_ignore_ascii_case(codec_name))
                {
                    return None;
                }
                let rank = Self::videotoolbox_hardware_rank(id, display_name.as_deref());
                // Only accept positive hardware evidence. Unknown future IDs
                // safely fall back to another hardware backend or x264.
                if rank < 2 {
                    return None;
                }
                Some((rank, encoder.clone()))
            })
            .max_by_key(|(rank, _)| *rank)
            .map(|(_, encoder)| encoder)
    }

    fn hardware_preset_to_x264(preset: HardwarePreset) -> &'static str {
        match preset {
            HardwarePreset::Speed => "veryfast",
            HardwarePreset::Balanced => "medium",
            HardwarePreset::Quality => "slow",
        }
    }

    fn selected_encoder_preset(
        &self,
        selected_encoder: &ObsVideoEncoderType,
    ) -> Option<&'static str> {
        match self.settings.video_encoder {
            VideoEncoder::X264(preset) => Some(preset.as_str()),
            VideoEncoder::Hardware { preset, .. } => {
                if *selected_encoder == ObsVideoEncoderType::OBS_X264 {
                    Some(Self::hardware_preset_to_x264(preset))
                } else if Self::is_videotoolbox_encoder(selected_encoder) {
                    // mac-videotoolbox has no generic `preset` property.
                    None
                } else {
                    Some(preset.as_str())
                }
            }
            VideoEncoder::Custom(_) => None,
        }
    }

    fn uses_x264_options(selected_encoder: &ObsVideoEncoderType) -> bool {
        *selected_encoder == ObsVideoEncoderType::OBS_X264
    }

    fn x264_crf_from_quality(quality: u32) -> i64 {
        (100u32.saturating_sub(quality.min(100)) * 51 / 100) as i64
    }

    fn configure_video_encoder(
        &self,
        settings: &mut ObsData,
        selected_encoder: &ObsVideoEncoderType,
    ) -> Result<(), ObsError> {
        // VideoToolbox universally exposes ABR. CBR is only exposed by some
        // Apple-Silicon/macOS combinations, so ABR is the portable bitrate mode.
        let use_x264_crf = self.settings.crf.is_some() && Self::uses_x264_options(selected_encoder);
        let rate_control = if use_x264_crf {
            "CRF"
        } else if Self::is_videotoolbox_encoder(selected_encoder) {
            "ABR"
        } else {
            "CBR"
        };
        settings.set_string("rate_control", rate_control)?;
        if let Some(quality) = self.settings.crf.filter(|_| use_x264_crf) {
            settings.set_int("crf", Self::x264_crf_from_quality(quality))?;
        } else {
            if self.settings.crf.is_some() {
                log::warn!(
                    "CRF is only supported by the selected x264 encoder; using {rate_control} at {} Kbps instead",
                    self.settings.video_bitrate
                );
            }
            settings.set_int("bitrate", self.settings.video_bitrate as i64)?;
        }

        if let Some(preset) = self.selected_encoder_preset(selected_encoder) {
            settings.set_string("preset", preset)?;
        }

        // `x264opts` is an x264-only property. Hardware selection can fall
        // back to x264, so key this off the actual selected encoder rather than
        // the requested encoder mode.
        if Self::uses_x264_options(selected_encoder) {
            if let Some(ref custom) = self.settings.custom_encoder_settings {
                settings.set_string("x264opts", custom.as_str())?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardware_fallback_presets_map_to_valid_x264_presets() {
        assert_eq!(
            SimpleOutputBuilder::hardware_preset_to_x264(HardwarePreset::Speed),
            "veryfast"
        );
        assert_eq!(
            SimpleOutputBuilder::hardware_preset_to_x264(HardwarePreset::Balanced),
            "medium"
        );
        assert_eq!(
            SimpleOutputBuilder::hardware_preset_to_x264(HardwarePreset::Quality),
            "slow"
        );
    }

    #[test]
    fn videotoolbox_selection_requires_matching_codec_and_prefers_hardware() {
        let available = vec![
            (
                ObsVideoEncoderType::Other("com.apple.videotoolbox.videoencoder.h264".into()),
                Some("h264".into()),
                Some("Apple H.264 (SW)".into()),
            ),
            (
                ObsVideoEncoderType::Other("com.apple.videotoolbox.videoencoder.ave.hevc".into()),
                Some("hevc".into()),
                Some("Apple HEVC (AVE)".into()),
            ),
            (
                ObsVideoEncoderType::Other("com.apple.videotoolbox.videoencoder.ave.avc".into()),
                Some("h264".into()),
                Some("Apple H.264 (HW)".into()),
            ),
        ];

        assert_eq!(
            SimpleOutputBuilder::select_videotoolbox_encoder(HardwareCodec::H264, &available),
            Some(ObsVideoEncoderType::Other(
                "com.apple.videotoolbox.videoencoder.ave.avc".into()
            ))
        );
        assert_eq!(
            SimpleOutputBuilder::select_videotoolbox_encoder(HardwareCodec::HEVC, &available),
            Some(ObsVideoEncoderType::Other(
                "com.apple.videotoolbox.videoencoder.ave.hevc".into()
            ))
        );
        assert_eq!(
            SimpleOutputBuilder::select_videotoolbox_encoder(HardwareCodec::AV1, &available),
            None
        );
        let software_only = vec![(
            ObsVideoEncoderType::Other("com.apple.videotoolbox.videoencoder.h264".into()),
            Some("h264".into()),
            Some("Apple H.264 (SW)".into()),
        )];
        assert_eq!(
            SimpleOutputBuilder::select_videotoolbox_encoder(HardwareCodec::H264, &software_only),
            None
        );
    }

    #[test]
    fn unknown_videotoolbox_encoder_does_not_satisfy_hardware_request() {
        let unknown = vec![(
            ObsVideoEncoderType::Other("com.apple.videotoolbox.videoencoder.future.avc".into()),
            Some("h264".into()),
            Some("Localized encoder name".into()),
        )];

        assert_eq!(
            SimpleOutputBuilder::select_videotoolbox_encoder(HardwareCodec::H264, &unknown),
            None
        );
    }

    #[test]
    fn x264_options_are_keyed_to_actual_selected_encoder() {
        assert!(SimpleOutputBuilder::uses_x264_options(
            &ObsVideoEncoderType::OBS_X264
        ));
        assert!(!SimpleOutputBuilder::uses_x264_options(
            &ObsVideoEncoderType::FFMPEG_VAAPI_TEX
        ));
        assert!(!SimpleOutputBuilder::uses_x264_options(
            &ObsVideoEncoderType::Other("com.apple.videotoolbox.videoencoder.ave.avc".into())
        ));
    }

    #[test]
    fn x264_crf_quality_scale_is_clamped_and_inverted() {
        assert_eq!(SimpleOutputBuilder::x264_crf_from_quality(100), 0);
        assert_eq!(SimpleOutputBuilder::x264_crf_from_quality(0), 51);
        assert_eq!(SimpleOutputBuilder::x264_crf_from_quality(150), 0);
    }

    #[test]
    fn vaapi_candidates_prefer_texture_variants() {
        let h264 = SimpleOutputBuilder::hardware_candidates(HardwareCodec::H264);
        assert!(
            h264.iter()
                .position(|id| id == &ObsVideoEncoderType::FFMPEG_VAAPI_TEX)
                < h264
                    .iter()
                    .position(|id| id == &ObsVideoEncoderType::FFMPEG_VAAPI)
        );

        let hevc = SimpleOutputBuilder::hardware_candidates(HardwareCodec::HEVC);
        assert!(
            hevc.iter()
                .position(|id| id == &ObsVideoEncoderType::HEVC_FFMPEG_VAAPI_TEX)
                < hevc
                    .iter()
                    .position(|id| id == &ObsVideoEncoderType::HEVC_FFMPEG_VAAPI)
        );

        let av1 = SimpleOutputBuilder::hardware_candidates(HardwareCodec::AV1);
        assert!(
            av1.iter()
                .position(|id| id == &ObsVideoEncoderType::AV1_FFMPEG_VAAPI_TEX)
                < av1
                    .iter()
                    .position(|id| id == &ObsVideoEncoderType::AV1_FFMPEG_VAAPI)
        );
    }
}
