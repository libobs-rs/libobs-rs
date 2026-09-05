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
    data::{
        audio::{AudioEncoderSlot, AudioMixIndex},
        output::{ObsOutputRef, ObsOutputTrait},
        ObsData, ObsDataSetters,
    },
    encoders::{ObsAudioEncoderType, ObsVideoEncoderType},
    settings::PropertyValue,
    utils::{ObsError, ObsPath, ObsString},
};

use super::configure::{set_reported, ConfigurationMode, OutputConfigurationReport};

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
    /// Require a hardware encoder (NVENC/AMF/QSV/etc.); no software fallback is allowed.
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
    video_encoder_candidates(capabilities, encoder)
        .into_iter()
        .next()
        .ok_or(ObsError::NoAvailableEncoders)
}

fn video_encoder_candidates(
    capabilities: &ObsCapabilities,
    encoder: &VideoEncoder,
) -> Vec<EncoderTypeInfo> {
    match encoder {
        VideoEncoder::Auto(codec) => capabilities
            .select_video_encoder()
            .codec(codec.codec_name())
            .prefer_hardware()
            .matches()
            .into_iter()
            .cloned()
            .collect(),
        VideoEncoder::Hardware { codec, .. } => capabilities
            .select_video_encoder()
            .codec(codec.codec_name())
            .require_hardware()
            .matches()
            .into_iter()
            .cloned()
            .collect(),
        VideoEncoder::X264(_) => capabilities
            .encoders()
            .iter()
            .filter(|candidate| candidate.id() == "obs_x264")
            .cloned()
            .collect(),
        VideoEncoder::Custom(encoder_type) => {
            let id = ObsString::from(encoder_type.clone()).to_string();
            capabilities
                .encoders()
                .iter()
                .filter(|candidate| candidate.id() == id)
                .cloned()
                .collect()
        }
    }
}

fn creation_failure_allows_fallback(error: &ObsError) -> bool {
    matches!(
        error,
        ObsError::NullPointer(_) | ObsError::NoAvailableEncoders | ObsError::SourceNotAvailable(_)
    )
}

fn start_failure_allows_encoder_fallback(error: &ObsError) -> bool {
    let ObsError::OutputStartFailure(Some(message)) = error else {
        return false;
    };
    let message = message.to_ascii_lowercase();
    if [
        "path",
        "permission",
        "disk",
        "space",
        "connect",
        "server",
        "auth",
        "stream key",
        "network",
        "url",
        "destination",
    ]
    .iter()
    .any(|token| message.contains(token))
    {
        return false;
    }
    ["encoder", "nvenc", "amf", "qsv", "vaapi", "videotoolbox"]
        .iter()
        .any(|token| message.contains(token))
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

/// One audio track in a simple recording. The encoder reads `mix` and is attached to `slot`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimpleAudioTrack {
    pub mix: AudioMixIndex,
    pub slot: AudioEncoderSlot,
}

impl SimpleAudioTrack {
    pub const fn new(mix: AudioMixIndex, slot: AudioEncoderSlot) -> Self {
        Self { mix, slot }
    }
}

/// Unified output settings
#[derive(Debug, Clone)]
pub struct OutputSettings {
    name: ObsString,
    video_bitrate: u32,
    audio_bitrate: u32,
    video_encoder: VideoEncoder,
    audio_encoder: AudioEncoder,
    audio_tracks: Vec<SimpleAudioTrack>,
    custom_encoder_settings: Option<String>,
    path: ObsPath,
    format: OutputFormat,
    custom_muxer_settings: Option<String>,
    /// Quality for CRF-based x264 encoding, on a 0–100 scale.
    crf: Option<u32>,
    configuration_mode: ConfigurationMode,
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

#[derive(Debug, Clone)]
pub struct SimpleOutputBuilder {
    settings: OutputSettings,
    context: ObsContext,
}

/// Simple recording output paired with the effective plugin/settings report used to build it.
#[derive(Debug)]
pub struct ConfiguredSimpleOutput {
    output: ObsOutputRef,
    report: OutputConfigurationReport,
}

impl ConfiguredSimpleOutput {
    pub fn output(&self) -> &ObsOutputRef {
        &self.output
    }

    pub fn report(&self) -> &OutputConfigurationReport {
        &self.report
    }

    pub fn into_output(self) -> ObsOutputRef {
        self.output
    }
}

/// Managed simple recording that can conservatively retry eligible encoder initialization/start
/// failures using the remaining compatible candidates. Destination/auth/path errors are never
/// retried as encoder fallbacks.
#[derive(Debug)]
pub struct ManagedSimpleOutput {
    template: SimpleOutputBuilder,
    configured: ConfiguredSimpleOutput,
    next_candidate: usize,
}

impl ManagedSimpleOutput {
    pub fn output(&self) -> &ObsOutputRef {
        self.configured.output()
    }

    pub fn report(&self) -> &OutputConfigurationReport {
        self.configured.report()
    }

    pub fn start(&mut self) -> Result<(), ObsError> {
        loop {
            let plugin_id = self.configured.report.video_encoder_plugin_id.clone();
            match self.configured.output.start() {
                Ok(()) => {
                    self.configured.report.attempts.push(super::EncoderAttempt {
                        plugin_id,
                        stage: super::EncoderAttemptStage::Start,
                        outcome: super::EncoderAttemptOutcome::Succeeded,
                    });
                    return Ok(());
                }
                Err(error) => {
                    let eligible = start_failure_allows_encoder_fallback(&error);
                    self.configured.report.attempts.push(super::EncoderAttempt {
                        plugin_id: plugin_id.clone(),
                        stage: super::EncoderAttemptStage::Start,
                        outcome: super::EncoderAttemptOutcome::Failed {
                            error: error.to_string(),
                            fallback_eligible: eligible,
                        },
                    });
                    if !eligible
                        || self.configured.output.is_active().unwrap_or(true)
                        || self.configured.output.is_reconnecting().unwrap_or(true)
                    {
                        return Err(error);
                    }

                    let prior_attempts = self.configured.report.attempts.clone();
                    let prior_reasons = self.configured.report.fallback_reasons.clone();
                    let (mut replacement, next_candidate) = self
                        .template
                        .clone()
                        .build_with_report_from_candidate(self.next_candidate)?;
                    replacement.report.attempts.splice(0..0, prior_attempts);
                    replacement
                        .report
                        .fallback_reasons
                        .splice(0..0, prior_reasons);
                    replacement.report.fallback_reasons.push(format!(
                        "encoder '{}' failed to start; retrying eligible backend '{}'",
                        plugin_id, replacement.report.video_encoder_plugin_id
                    ));
                    self.configured = replacement;
                    self.next_candidate = next_candidate;
                }
            }
        }
    }

    pub fn stop(&self) -> Result<(), ObsError> {
        self.configured.output.stop()
    }
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
                audio_tracks: vec![SimpleAudioTrack::new(
                    AudioMixIndex::new(0).expect("OBS always exposes mix 0"),
                    AudioEncoderSlot::new(0).expect("OBS always exposes audio slot 0"),
                )],
                custom_encoder_settings: None,
                path: path.into(),
                format: OutputFormat::default(),
                custom_muxer_settings: None,
                crf: None,
                configuration_mode: ConfigurationMode::Permissive,
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

    /// Prefer hardware for the requested codec while allowing software fallback.
    pub fn prefer_hardware_encoder(self, codec: HardwareCodec) -> Self {
        self.auto_video_encoder(codec)
    }

    /// Sets the video encoder to x264.
    pub fn x264_encoder(mut self, preset: X264Preset) -> Self {
        self.settings.video_encoder = VideoEncoder::X264(preset);
        self
    }

    /// Requires a generic hardware encoder. If no hardware backend is available, build fails.
    pub fn hardware_encoder(mut self, codec: HardwareCodec, preset: HardwarePreset) -> Self {
        self.settings.video_encoder = VideoEncoder::Hardware { codec, preset };
        self
    }

    /// Alias spelling out the no-software-fallback policy.
    pub fn require_hardware_encoder(self, codec: HardwareCodec, preset: HardwarePreset) -> Self {
        self.hardware_encoder(codec, preset)
    }

    /// Selects one explicit backend by OBS encoder type. No automatic backend substitution occurs.
    pub fn explicit_video_encoder(mut self, encoder: ObsVideoEncoderType) -> Self {
        self.settings.video_encoder = VideoEncoder::Custom(encoder);
        self
    }

    /// Requires all requested generic encoder settings to be accepted exactly by the selected
    /// backend. In permissive mode (the default), unsupported/rejected generic choices retain the
    /// plugin default and are surfaced in [`OutputConfigurationReport`].
    pub fn configuration_mode(mut self, mode: ConfigurationMode) -> Self {
        self.settings.configuration_mode = mode;
        self
    }

    pub fn strict_configuration(self) -> Self {
        self.configuration_mode(ConfigurationMode::Strict)
    }

    /// Adds an encoded recording track. The source mix and output attachment slot are independent.
    ///
    /// The builder starts with mix 0 at slot 0 for compatibility. A common layout keeps that
    /// combined track and adds dedicated desktop/microphone mixes at slots 1 and 2.
    pub fn audio_track(mut self, mix: AudioMixIndex, slot: AudioEncoderSlot) -> Self {
        self.settings
            .audio_tracks
            .retain(|track| track.slot != slot);
        self.settings
            .audio_tracks
            .push(SimpleAudioTrack::new(mix, slot));
        self
    }

    /// Removes the compatibility default combined track (mix 0 / slot 0).
    pub fn without_default_audio_track(mut self) -> Self {
        self.settings
            .audio_tracks
            .retain(|track| track.slot.index() != 0);
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
    /// This compatibility entry point discards the structured configuration report.
    pub fn build(self) -> Result<ObsOutputRef, ObsError> {
        Ok(self.build_with_report()?.into_output())
    }

    /// Builds the graph and returns the effective plugin/settings configuration report.
    pub fn build_with_report(self) -> Result<ConfiguredSimpleOutput, ObsError> {
        self.build_with_report_from_candidate(0)
            .map(|(configured, _)| configured)
    }

    /// Builds the managed variant that can retry eligible initialization/start failures.
    pub fn build_managed(self) -> Result<ManagedSimpleOutput, ObsError> {
        let template = self.clone();
        let (configured, next_candidate) = self.build_with_report_from_candidate(0)?;
        Ok(ManagedSimpleOutput {
            template,
            configured,
            next_candidate,
        })
    }

    fn build_with_report_from_candidate(
        self,
        candidate_start: usize,
    ) -> Result<(ConfiguredSimpleOutput, usize), ObsError> {
        let output_id = match self.settings.format {
            OutputFormat::HybridMP4 => "mp4_output",
            OutputFormat::HybridMov => "mov_output",
            _ => "ffmpeg_muxer",
        };

        let capabilities = self.context.capabilities()?;
        let video_candidates =
            video_encoder_candidates(&capabilities, &self.settings.video_encoder);
        let preferred_video_type = video_candidates
            .get(candidate_start)
            .cloned()
            .ok_or(ObsError::NoAvailableEncoders)?;
        let audio_type = audio_encoder_descriptor(&capabilities, &self.settings.audio_encoder)?;

        let mut compatibility = OutputCompatibilityRequest::new()
            .output_id(output_id)
            .require_output_capabilities(
                OutputCapabilities::ENCODED | OutputCapabilities::VIDEO | OutputCapabilities::AUDIO,
            );
        if let Some(codec) = preferred_video_type.codec() {
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

        let mut report = OutputConfigurationReport {
            output_plugin_id: output_type.id().to_owned(),
            video_encoder_plugin_id: preferred_video_type.id().to_owned(),
            audio_encoder_plugin_ids: self
                .settings
                .audio_tracks
                .iter()
                .map(|_| audio_type.id().to_owned())
                .collect(),
            ..Default::default()
        };
        if matches!(self.settings.video_encoder, VideoEncoder::Auto(_))
            && !preferred_video_type.is_likely_hardware_accelerated()
        {
            report.fallback_reasons.push(format!(
                "preferred hardware encoder was unavailable; selected software backend '{}'",
                preferred_video_type.id()
            ));
        }

        let mut output_settings = output_type.default_settings_mut()?;
        output_settings.set_string("path", self.settings.path.clone().build())?;
        if let Some(ref muxer_settings) = self.settings.custom_muxer_settings {
            output_settings.set_string("muxer_settings", muxer_settings.as_str())?;
        }

        let mut selected_video = None;
        let mut selected_index = candidate_start;
        let mut last_creation_error = None;
        for (index, video_type) in video_candidates.iter().enumerate().skip(candidate_start) {
            let mut candidate_report = OutputConfigurationReport::default();
            let mut video_settings = video_type.default_settings_mut()?;
            self.configure_video_encoder(video_type, &mut video_settings, &mut candidate_report)?;
            match self.context.create_video_encoder(
                video_type,
                format!("{}_video_encoder", self.settings.name),
                Some(video_settings),
            ) {
                Ok(encoder) => {
                    report.video_encoder_plugin_id = video_type.id().to_owned();
                    report.settings.extend(candidate_report.settings);
                    report.attempts.push(super::EncoderAttempt {
                        plugin_id: video_type.id().to_owned(),
                        stage: super::EncoderAttemptStage::Creation,
                        outcome: super::EncoderAttemptOutcome::Succeeded,
                    });
                    if index > candidate_start {
                        report.fallback_reasons.push(format!(
                            "earlier encoder creation failed; selected '{}'",
                            video_type.id()
                        ));
                    }
                    selected_video = Some(encoder);
                    selected_index = index;
                    break;
                }
                Err(error) => {
                    let eligible = creation_failure_allows_fallback(&error);
                    report.attempts.push(super::EncoderAttempt {
                        plugin_id: video_type.id().to_owned(),
                        stage: super::EncoderAttemptStage::Creation,
                        outcome: super::EncoderAttemptOutcome::Failed {
                            error: error.to_string(),
                            fallback_eligible: eligible,
                        },
                    });
                    if !eligible {
                        return Err(error);
                    }
                    last_creation_error = Some(error);
                }
            }
        }
        let video_encoder = selected_video
            .ok_or_else(|| last_creation_error.unwrap_or(ObsError::NoAvailableEncoders))?;

        if self.settings.audio_tracks.is_empty() {
            return Err(ObsError::InvalidOperation(
                "simple recording requires at least one audio track".into(),
            ));
        }

        let mut audio_encoders = Vec::with_capacity(self.settings.audio_tracks.len());
        for track in &self.settings.audio_tracks {
            let mut audio_settings = audio_type.default_settings_mut()?;
            let audio_schema = audio_type.settings_schema_for(&audio_settings)?;
            set_reported(
                &audio_schema,
                &mut audio_settings,
                "audio_encoder",
                "rate_control",
                PropertyValue::String("CBR".into()),
                self.settings.configuration_mode,
                &mut report,
            )?;
            set_reported(
                &audio_schema,
                &mut audio_settings,
                "audio_encoder",
                "bitrate",
                PropertyValue::Integer(i64::from(self.settings.audio_bitrate)),
                self.settings.configuration_mode,
                &mut report,
            )?;
            let encoder = self.context.create_audio_encoder_for_mix(
                &audio_type,
                format!(
                    "{}_audio_mix{}_slot{}",
                    self.settings.name,
                    track.mix.index(),
                    track.slot.index()
                ),
                Some(audio_settings),
                track.mix,
            )?;
            audio_encoders.push((*track, encoder));
        }

        let mut pipeline = self
            .context
            .output_pipeline(
                &output_type,
                self.settings.name.clone(),
                Some(output_settings),
            )
            .video_encoder(video_encoder);
        for (track, encoder) in audio_encoders {
            pipeline = pipeline.audio_encoder_at_slot(track.slot, encoder);
        }

        Ok((
            ConfiguredSimpleOutput {
                output: pipeline.build()?.into_output(),
                report,
            },
            selected_index + 1,
        ))
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
        report: &mut OutputConfigurationReport,
    ) -> Result<(), ObsError> {
        let schema = encoder_type.settings_schema_for(settings)?;
        let use_crf = self.settings.crf.is_some() && Self::uses_x264(encoder_type);
        if use_crf {
            set_reported(
                &schema,
                settings,
                "video_encoder",
                "rate_control",
                PropertyValue::String("CRF".into()),
                self.settings.configuration_mode,
                report,
            )?;
            if let Some(quality) = self.settings.crf {
                set_reported(
                    &schema,
                    settings,
                    "video_encoder",
                    "crf",
                    PropertyValue::Integer(Self::x264_crf_from_quality(quality)),
                    self.settings.configuration_mode,
                    report,
                )?;
            }
        } else {
            set_reported(
                &schema,
                settings,
                "video_encoder",
                "rate_control",
                PropertyValue::String("CBR".into()),
                self.settings.configuration_mode,
                report,
            )?;
            set_reported(
                &schema,
                settings,
                "video_encoder",
                "bitrate",
                PropertyValue::Integer(i64::from(self.settings.video_bitrate)),
                self.settings.configuration_mode,
                report,
            )?;
            if self.settings.crf.is_some() {
                let reason = format!(
                    "CRF is x264-specific; selected backend '{}' retained its bitrate/default rate control",
                    encoder_type.id()
                );
                report.fallback_reasons.push(reason.clone());
                log::warn!("{reason}");
                if self.settings.configuration_mode == ConfigurationMode::Strict {
                    return Err(ObsError::InvalidOperation(reason));
                }
            }
        }
        if let Some(preset) = self.get_encoder_preset(&self.settings.video_encoder) {
            set_reported(
                &schema,
                settings,
                "video_encoder",
                "preset",
                PropertyValue::String(preset.into()),
                self.settings.configuration_mode,
                report,
            )?;
        }
        if let Some(ref custom) = self.settings.custom_encoder_settings {
            set_reported(
                &schema,
                settings,
                "video_encoder",
                "x264opts",
                PropertyValue::String(custom.clone()),
                self.settings.configuration_mode,
                report,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn start_fallback_is_conservative_about_destination_failures() {
        for message in [
            "invalid output path",
            "permission denied opening file",
            "failed to connect to server",
            "authentication failed",
            "invalid stream key",
        ] {
            assert!(!start_failure_allows_encoder_fallback(
                &ObsError::OutputStartFailure(Some(message.into()))
            ));
        }
    }

    #[test]
    fn start_fallback_accepts_backend_initialization_failures() {
        for message in [
            "NVENC encoder failed to initialize",
            "QSV encoder unavailable",
            "AMF encoder initialization failed",
            "video encoder could not start",
        ] {
            assert!(start_failure_allows_encoder_fallback(
                &ObsError::OutputStartFailure(Some(message.into()))
            ));
        }
    }

    #[test]
    fn unrelated_errors_do_not_trigger_creation_fallback() {
        assert!(creation_failure_allows_fallback(&ObsError::NullPointer(
            None
        )));
        assert!(!creation_failure_allows_fallback(
            &ObsError::InvalidOperation("bad destination".into())
        ));
    }
}
