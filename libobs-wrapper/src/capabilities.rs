//! Runtime capability discovery and compatibility planning for the loaded OBS installation.
//!
//! All enumeration and property inspection is executed on the OBS actor. Values returned from this
//! module are owned Rust snapshots; no callback-borrowed or temporary libobs pointer escapes into
//! safe downstream code.
//!
//! Use [`ObsCapabilities`] when the application cares about capabilities such as “H.264”, “AAC” or
//! “RTMP” rather than a specific plugin ID. Selectors expose candidate lists, while
//! [`ObsCapabilities::best_output_plan`] resolves a complete output/encoder choice and reports
//! structured rejection reasons when no combination works.
//!
//! Each source/output/encoder/service descriptor also exposes runtime property schemas and
//! form-ready settings snapshots through [`crate::settings`]. This is the plugin-generic path for
//! third-party OBS settings UIs.

use std::{
    collections::{HashMap, HashSet},
    ffi::CStr,
    os::raw::{c_char, c_void},
    ptr,
};

use crate::{
    context::ObsContext,
    data::{output::ObsOutputRef, ImmutableObsData, ObsData, ObsDataPointers},
    encoders::{audio::ObsAudioEncoder, video::ObsVideoEncoder},
    run_with_obs,
    runtime::ObsRuntime,
    scenes::{ObsSceneItemRef, ObsSceneRef},
    services::ObsServiceRef,
    settings::{SettingsSchema, SettingsSnapshot},
    sources::{ObsFilterRef, ObsSourceRef},
    unsafe_send::Sendable,
    utils::{ObjectInfo, ObsError, ObsString},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceKind {
    Input,
    Filter,
    Transition,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EncoderKind {
    Audio,
    Video,
    Unknown(i64),
}

bitflags::bitflags! {
    /// Capability flags reported by libobs for an encoder type.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct EncoderCapabilities: u32 {
        const DEPRECATED = libobs::OBS_ENCODER_CAP_DEPRECATED;
        const PASS_TEXTURE = libobs::OBS_ENCODER_CAP_PASS_TEXTURE;
        const DYNAMIC_BITRATE = libobs::OBS_ENCODER_CAP_DYN_BITRATE;
        const INTERNAL = libobs::OBS_ENCODER_CAP_INTERNAL;
        const ROI = libobs::OBS_ENCODER_CAP_ROI;
        const SCALING = libobs::OBS_ENCODER_CAP_SCALING;
    }
}

bitflags::bitflags! {
    /// Capability flags reported by libobs for an output type. Unknown future bits are retained.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct OutputCapabilities: u32 {
        const VIDEO = libobs::OBS_OUTPUT_VIDEO;
        const AUDIO = libobs::OBS_OUTPUT_AUDIO;
        const ENCODED = libobs::OBS_OUTPUT_ENCODED;
        const SERVICE = libobs::OBS_OUTPUT_SERVICE;
        const MULTI_TRACK_AUDIO = libobs::OBS_OUTPUT_MULTI_TRACK_AUDIO;
        const CAN_PAUSE = libobs::OBS_OUTPUT_CAN_PAUSE;
        const MULTI_TRACK_VIDEO = libobs::OBS_OUTPUT_MULTI_TRACK_VIDEO;
    }
}

#[derive(Clone, Debug)]
pub struct SourceTypeInfo {
    id: String,
    display_name: Option<String>,
    kind: SourceKind,
    output_flags: u32,
    runtime: ObsRuntime,
}

impl SourceTypeInfo {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    pub fn kind(&self) -> SourceKind {
        self.kind
    }

    pub fn output_flags(&self) -> u32 {
        self.output_flags
    }

    pub fn properties(&self) -> Result<Vec<PropertyMetadata>, ObsError> {
        source_properties(&self.runtime, &self.id)
    }

    pub fn settings_schema(&self) -> Result<SettingsSchema, ObsError> {
        Ok(SettingsSchema::new(self.properties()?))
    }

    /// Rebuilds the property tree after asking OBS to apply property-modified callbacks for
    /// the supplied settings. This reflects dynamic visibility, enabled state, and list items.
    pub fn settings_schema_for(&self, settings: &ObsData) -> Result<SettingsSchema, ObsError> {
        self.runtime.ensure_same_runtime(settings.runtime())?;
        Ok(SettingsSchema::new(source_properties_for_settings(
            &self.runtime,
            &self.id,
            settings,
        )?))
    }

    pub fn settings_snapshot_for(&self, settings: &ObsData) -> Result<SettingsSnapshot, ObsError> {
        let schema = self.settings_schema_for(settings)?;
        let defaults = self.default_settings()?;
        schema.snapshot(settings, defaults.as_ref())
    }

    pub fn default_settings(&self) -> Result<Option<ImmutableObsData>, ObsError> {
        default_settings(&self.runtime, &self.id, libobs::obs_get_source_defaults)
    }

    /// Returns an owned mutable copy of this type's defaults. If libobs reports no
    /// defaults, an empty settings object for the same runtime is returned.
    pub fn default_settings_mut(&self) -> Result<ObsData, ObsError> {
        mutable_default_settings(&self.runtime, &self.id, libobs::obs_get_source_defaults)
    }
}

#[derive(Clone, Debug)]
pub struct OutputTypeInfo {
    id: String,
    display_name: Option<String>,
    video_codecs: Vec<String>,
    audio_codecs: Vec<String>,
    protocols: Vec<String>,
    flags: u32,
    runtime: ObsRuntime,
}

impl OutputTypeInfo {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    pub fn video_codecs(&self) -> &[String] {
        &self.video_codecs
    }

    pub fn audio_codecs(&self) -> &[String] {
        &self.audio_codecs
    }

    pub fn protocols(&self) -> &[String] {
        &self.protocols
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub(crate) fn runtime(&self) -> &ObsRuntime {
        &self.runtime
    }

    pub fn capability_flags(&self) -> OutputCapabilities {
        OutputCapabilities::from_bits_retain(self.flags)
    }

    pub fn supports_video_codec(&self, codec: &str) -> bool {
        capability_list_supports(&self.video_codecs, codec)
    }

    pub fn supports_audio_codec(&self, codec: &str) -> bool {
        capability_list_supports(&self.audio_codecs, codec)
    }

    pub fn supports_protocol(&self, protocol: &str) -> bool {
        capability_list_supports(&self.protocols, protocol)
    }

    pub fn properties(&self) -> Result<Vec<PropertyMetadata>, ObsError> {
        type_properties(&self.runtime, &self.id, libobs::obs_get_output_properties)
    }

    pub fn settings_schema(&self) -> Result<SettingsSchema, ObsError> {
        Ok(SettingsSchema::new(self.properties()?))
    }

    pub fn settings_schema_for(&self, settings: &ObsData) -> Result<SettingsSchema, ObsError> {
        self.runtime.ensure_same_runtime(settings.runtime())?;
        Ok(SettingsSchema::new(type_properties_for_settings(
            &self.runtime,
            &self.id,
            libobs::obs_get_output_properties,
            settings,
        )?))
    }

    pub fn settings_snapshot_for(&self, settings: &ObsData) -> Result<SettingsSnapshot, ObsError> {
        let schema = self.settings_schema_for(settings)?;
        let defaults = self.default_settings()?;
        schema.snapshot(settings, defaults.as_ref())
    }

    pub fn default_settings(&self) -> Result<Option<ImmutableObsData>, ObsError> {
        default_settings(&self.runtime, &self.id, libobs::obs_output_defaults)
    }

    /// Returns a mutable settings object initialized from this output type's defaults.
    pub fn default_settings_mut(&self) -> Result<ObsData, ObsError> {
        mutable_default_settings(&self.runtime, &self.id, libobs::obs_output_defaults)
    }
}

#[derive(Clone, Debug)]
pub struct EncoderTypeInfo {
    id: String,
    display_name: Option<String>,
    kind: EncoderKind,
    codec: Option<String>,
    capabilities: u32,
    runtime: ObsRuntime,
}

impl EncoderTypeInfo {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    pub fn kind(&self) -> EncoderKind {
        self.kind
    }

    pub fn codec(&self) -> Option<&str> {
        self.codec.as_deref()
    }

    pub fn capabilities(&self) -> u32 {
        self.capabilities
    }

    pub fn capability_flags(&self) -> EncoderCapabilities {
        EncoderCapabilities::from_bits_retain(self.capabilities)
    }

    pub fn is_deprecated(&self) -> bool {
        self.capability_flags()
            .contains(EncoderCapabilities::DEPRECATED)
    }

    pub fn is_internal(&self) -> bool {
        self.capability_flags()
            .contains(EncoderCapabilities::INTERNAL)
    }

    /// Returns whether this encoder is likely hardware-accelerated.
    ///
    /// libobs does not expose one universal hardware bit. Passing textures is a strong
    /// signal for zero-copy hardware encoders; the fallback ID check covers common
    /// hardware plugins that do not advertise that capability. This is a preference
    /// heuristic only and is never used to exclude software fallbacks.
    pub fn is_likely_hardware_accelerated(&self) -> bool {
        if self
            .capability_flags()
            .contains(EncoderCapabilities::PASS_TEXTURE)
        {
            return true;
        }
        let id = self.id.to_ascii_lowercase();
        ["nvenc", "qsv", "amf", "vaapi", "videotoolbox"]
            .iter()
            .any(|marker| id.contains(marker))
    }

    pub fn properties(&self) -> Result<Vec<PropertyMetadata>, ObsError> {
        type_properties(&self.runtime, &self.id, libobs::obs_get_encoder_properties)
    }

    pub fn settings_schema(&self) -> Result<SettingsSchema, ObsError> {
        Ok(SettingsSchema::new(self.properties()?))
    }

    pub fn settings_schema_for(&self, settings: &ObsData) -> Result<SettingsSchema, ObsError> {
        self.runtime.ensure_same_runtime(settings.runtime())?;
        Ok(SettingsSchema::new(type_properties_for_settings(
            &self.runtime,
            &self.id,
            libobs::obs_get_encoder_properties,
            settings,
        )?))
    }

    pub fn settings_snapshot_for(&self, settings: &ObsData) -> Result<SettingsSnapshot, ObsError> {
        let schema = self.settings_schema_for(settings)?;
        let defaults = self.default_settings()?;
        schema.snapshot(settings, defaults.as_ref())
    }

    pub fn default_settings(&self) -> Result<Option<ImmutableObsData>, ObsError> {
        default_settings(&self.runtime, &self.id, libobs::obs_encoder_defaults)
    }

    /// Returns a mutable settings object initialized from this encoder type's defaults.
    pub fn default_settings_mut(&self) -> Result<ObsData, ObsError> {
        mutable_default_settings(&self.runtime, &self.id, libobs::obs_encoder_defaults)
    }
}

#[derive(Clone, Debug)]
pub struct ServiceTypeInfo {
    id: String,
    display_name: Option<String>,
    runtime: ObsRuntime,
}

impl ServiceTypeInfo {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    pub fn properties(&self) -> Result<Vec<PropertyMetadata>, ObsError> {
        type_properties(&self.runtime, &self.id, libobs::obs_get_service_properties)
    }

    pub fn settings_schema(&self) -> Result<SettingsSchema, ObsError> {
        Ok(SettingsSchema::new(self.properties()?))
    }

    pub fn settings_schema_for(&self, settings: &ObsData) -> Result<SettingsSchema, ObsError> {
        self.runtime.ensure_same_runtime(settings.runtime())?;
        Ok(SettingsSchema::new(type_properties_for_settings(
            &self.runtime,
            &self.id,
            libobs::obs_get_service_properties,
            settings,
        )?))
    }

    pub fn settings_snapshot_for(&self, settings: &ObsData) -> Result<SettingsSnapshot, ObsError> {
        let schema = self.settings_schema_for(settings)?;
        let defaults = self.default_settings()?;
        schema.snapshot(settings, defaults.as_ref())
    }

    pub fn default_settings(&self) -> Result<Option<ImmutableObsData>, ObsError> {
        default_settings(&self.runtime, &self.id, libobs::obs_service_defaults)
    }

    /// Returns a mutable settings object initialized from this service type's defaults.
    pub fn default_settings_mut(&self) -> Result<ObsData, ObsError> {
        mutable_default_settings(&self.runtime, &self.id, libobs::obs_service_defaults)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleInfo {
    pub file_name: Option<String>,
    pub name: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub binary_path: Option<String>,
    pub data_path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ObsCapabilities {
    source_types: Vec<SourceTypeInfo>,
    input_types: Vec<SourceTypeInfo>,
    filter_types: Vec<SourceTypeInfo>,
    transition_types: Vec<SourceTypeInfo>,
    outputs: Vec<OutputTypeInfo>,
    encoders: Vec<EncoderTypeInfo>,
    services: Vec<ServiceTypeInfo>,
    protocols: Vec<String>,
    modules: Vec<ModuleInfo>,
}

impl ObsCapabilities {
    pub fn source_types(&self) -> &[SourceTypeInfo] {
        &self.source_types
    }
    pub fn input_types(&self) -> &[SourceTypeInfo] {
        &self.input_types
    }
    pub fn filter_types(&self) -> &[SourceTypeInfo] {
        &self.filter_types
    }
    pub fn transition_types(&self) -> &[SourceTypeInfo] {
        &self.transition_types
    }
    pub fn outputs(&self) -> &[OutputTypeInfo] {
        &self.outputs
    }
    pub fn encoders(&self) -> &[EncoderTypeInfo] {
        &self.encoders
    }
    pub fn services(&self) -> &[ServiceTypeInfo] {
        &self.services
    }
    pub fn protocols(&self) -> &[String] {
        &self.protocols
    }
    pub fn modules(&self) -> &[ModuleInfo] {
        &self.modules
    }

    /// Starts a deterministic selection over discovered video encoders.
    pub fn select_video_encoder(&self) -> EncoderSelector<'_> {
        EncoderSelector::new(&self.encoders, EncoderKind::Video)
    }

    /// Starts a deterministic selection over discovered audio encoders.
    pub fn select_audio_encoder(&self) -> EncoderSelector<'_> {
        EncoderSelector::new(&self.encoders, EncoderKind::Audio)
    }

    /// Starts a deterministic selection over discovered outputs.
    pub fn select_output(&self) -> OutputSelector<'_> {
        OutputSelector::new(&self.outputs)
    }

    /// Resolves the best currently available output/encoder combination for a capability request.
    /// The operation is pure over this owned capability snapshot and never creates native objects.
    pub fn best_output_plan(
        &self,
        request: &OutputCompatibilityRequest,
    ) -> Result<OutputCompatibilityPlan, OutputCompatibilityReport> {
        let video_encoder = request.video_codec.as_deref().and_then(|codec| {
            let selector = self.select_video_encoder().codec(codec);
            if request.prefer_hardware_video {
                selector.prefer_hardware().best_available().cloned()
            } else {
                selector.best_available().cloned()
            }
        });
        let audio_encoder = request.audio_codec.as_deref().and_then(|codec| {
            self.select_audio_encoder()
                .codec(codec)
                .best_available()
                .cloned()
        });

        let mut issues = Vec::new();
        if let Some(codec) = request.video_codec.as_deref() {
            if video_encoder.is_none() {
                issues.push(CompatibilityIssue::NoVideoEncoder {
                    codec: codec.into(),
                });
            }
        }
        if let Some(codec) = request.audio_codec.as_deref() {
            if audio_encoder.is_none() {
                issues.push(CompatibilityIssue::NoAudioEncoder {
                    codec: codec.into(),
                });
            }
        }

        let mut matching_outputs = Vec::new();
        for output in &self.outputs {
            let mut reasons = Vec::new();
            if let Some(required_id) = request.output_id.as_deref() {
                if output.id() != required_id {
                    reasons.push(OutputRejectionReason::OutputId {
                        required: required_id.into(),
                    });
                }
            }
            let flags = output.capability_flags();
            if !flags.contains(request.required_output_capabilities) {
                reasons.push(OutputRejectionReason::MissingCapabilities {
                    required: request.required_output_capabilities,
                    actual: flags,
                });
            }
            if let Some(protocol) = request.protocol.as_deref() {
                if !output.supports_protocol(protocol) {
                    reasons.push(OutputRejectionReason::Protocol {
                        required: protocol.into(),
                    });
                }
            }
            if let Some(codec) = request.video_codec.as_deref() {
                if !output.supports_video_codec(codec) {
                    reasons.push(OutputRejectionReason::VideoCodec {
                        required: codec.into(),
                    });
                }
            }
            if let Some(codec) = request.audio_codec.as_deref() {
                if !output.supports_audio_codec(codec) {
                    reasons.push(OutputRejectionReason::AudioCodec {
                        required: codec.into(),
                    });
                }
            }
            if reasons.is_empty() {
                matching_outputs.push(output.clone());
            } else {
                issues.push(CompatibilityIssue::OutputRejected {
                    output_id: output.id.clone(),
                    reasons,
                });
            }
        }

        matching_outputs.sort_by(|a, b| a.id.cmp(&b.id));
        let output = matching_outputs.into_iter().next();
        if output.is_none() {
            issues.push(CompatibilityIssue::NoCompatibleOutput);
        }

        let video_required = request.video_codec.is_some();
        let audio_required = request.audio_codec.is_some();
        match (
            output,
            video_required && video_encoder.is_none(),
            audio_required && audio_encoder.is_none(),
        ) {
            (Some(output), false, false) => Ok(OutputCompatibilityPlan {
                output,
                video_encoder,
                audio_encoder,
                protocol: request.protocol.clone(),
            }),
            _ => Err(OutputCompatibilityReport { issues }),
        }
    }
}

/// Requirements used when resolving an output graph from runtime-discovered capabilities.
#[derive(Clone, Debug, Default)]
pub struct OutputCompatibilityRequest {
    output_id: Option<String>,
    protocol: Option<String>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    required_output_capabilities: OutputCapabilities,
    prefer_hardware_video: bool,
}

impl OutputCompatibilityRequest {
    pub fn new() -> Self {
        Self::default()
    }

    /// Restricts matching to one concrete output type while still validating codecs/flags.
    pub fn output_id(mut self, output_id: impl Into<String>) -> Self {
        self.output_id = Some(output_id.into());
        self
    }

    pub fn protocol(mut self, protocol: impl Into<String>) -> Self {
        self.protocol = Some(protocol.into());
        self
    }

    pub fn video_codec(mut self, codec: impl Into<String>) -> Self {
        self.video_codec = Some(codec.into());
        self
    }

    pub fn audio_codec(mut self, codec: impl Into<String>) -> Self {
        self.audio_codec = Some(codec.into());
        self
    }

    pub fn require_output_capabilities(mut self, capabilities: OutputCapabilities) -> Self {
        self.required_output_capabilities |= capabilities;
        self
    }

    pub fn prefer_hardware_video(mut self, prefer: bool) -> Self {
        self.prefer_hardware_video = prefer;
        self
    }
}

/// A concrete compatible choice of output and encoders from one capability snapshot.
#[derive(Clone, Debug)]
pub struct OutputCompatibilityPlan {
    output: OutputTypeInfo,
    video_encoder: Option<EncoderTypeInfo>,
    audio_encoder: Option<EncoderTypeInfo>,
    protocol: Option<String>,
}

impl OutputCompatibilityPlan {
    pub fn output(&self) -> &OutputTypeInfo {
        &self.output
    }
    pub fn video_encoder(&self) -> Option<&EncoderTypeInfo> {
        self.video_encoder.as_ref()
    }
    pub fn audio_encoder(&self) -> Option<&EncoderTypeInfo> {
        self.audio_encoder.as_ref()
    }
    pub fn protocol(&self) -> Option<&str> {
        self.protocol.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputRejectionReason {
    OutputId {
        required: String,
    },
    MissingCapabilities {
        required: OutputCapabilities,
        actual: OutputCapabilities,
    },
    Protocol {
        required: String,
    },
    VideoCodec {
        required: String,
    },
    AudioCodec {
        required: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompatibilityIssue {
    NoVideoEncoder {
        codec: String,
    },
    NoAudioEncoder {
        codec: String,
    },
    OutputRejected {
        output_id: String,
        reasons: Vec<OutputRejectionReason>,
    },
    NoCompatibleOutput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputCompatibilityReport {
    issues: Vec<CompatibilityIssue>,
}

impl OutputCompatibilityReport {
    pub fn issues(&self) -> &[CompatibilityIssue] {
        &self.issues
    }

    pub fn summary(&self) -> String {
        let encoder_issues = self
            .issues
            .iter()
            .filter(|issue| {
                matches!(
                    issue,
                    CompatibilityIssue::NoVideoEncoder { .. }
                        | CompatibilityIssue::NoAudioEncoder { .. }
                )
            })
            .count();
        let rejected_outputs = self
            .issues
            .iter()
            .filter(|issue| matches!(issue, CompatibilityIssue::OutputRejected { .. }))
            .count();
        format!(
            "no compatible OBS output graph: {encoder_issues} encoder requirement(s) unavailable, {rejected_outputs} output type(s) rejected"
        )
    }
}

/// Filters and ranks discovered encoder descriptors without creating native objects.
#[derive(Clone, Debug)]
pub struct EncoderSelector<'a> {
    encoders: &'a [EncoderTypeInfo],
    kind: EncoderKind,
    codec: Option<String>,
    required_capabilities: EncoderCapabilities,
    include_deprecated: bool,
    include_internal: bool,
    prefer_hardware: bool,
}

impl<'a> EncoderSelector<'a> {
    fn new(encoders: &'a [EncoderTypeInfo], kind: EncoderKind) -> Self {
        Self {
            encoders,
            kind,
            codec: None,
            required_capabilities: EncoderCapabilities::empty(),
            include_deprecated: false,
            include_internal: false,
            prefer_hardware: false,
        }
    }

    pub fn codec(mut self, codec: impl Into<String>) -> Self {
        self.codec = Some(codec.into());
        self
    }

    pub fn require_capabilities(mut self, capabilities: EncoderCapabilities) -> Self {
        self.required_capabilities |= capabilities;
        self
    }

    pub fn include_deprecated(mut self, include: bool) -> Self {
        self.include_deprecated = include;
        self
    }

    pub fn include_internal(mut self, include: bool) -> Self {
        self.include_internal = include;
        self
    }

    /// Prefer likely hardware-accelerated encoders while retaining software fallback.
    pub fn prefer_hardware(mut self) -> Self {
        self.prefer_hardware = true;
        self
    }

    pub fn matches(&self) -> Vec<&'a EncoderTypeInfo> {
        let mut candidates = self
            .encoders
            .iter()
            .filter(|encoder| encoder.kind == self.kind)
            .filter(|encoder| self.include_deprecated || !encoder.is_deprecated())
            .filter(|encoder| self.include_internal || !encoder.is_internal())
            .filter(|encoder| {
                self.codec.as_ref().is_none_or(|codec| {
                    encoder
                        .codec
                        .as_deref()
                        .is_some_and(|actual| actual.eq_ignore_ascii_case(codec))
                })
            })
            .filter(|encoder| {
                encoder
                    .capability_flags()
                    .contains(self.required_capabilities)
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|a, b| {
            let a_hardware = self.prefer_hardware && a.is_likely_hardware_accelerated();
            let b_hardware = self.prefer_hardware && b.is_likely_hardware_accelerated();
            b_hardware.cmp(&a_hardware).then_with(|| a.id.cmp(&b.id))
        });
        candidates
    }

    pub fn best_available(&self) -> Option<&'a EncoderTypeInfo> {
        self.matches().into_iter().next()
    }
}

/// Filters discovered outputs by their libobs-declared protocols, codecs, and flags.
#[derive(Clone, Debug)]
pub struct OutputSelector<'a> {
    outputs: &'a [OutputTypeInfo],
    protocol: Option<String>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    required_capabilities: OutputCapabilities,
}

impl<'a> OutputSelector<'a> {
    fn new(outputs: &'a [OutputTypeInfo]) -> Self {
        Self {
            outputs,
            protocol: None,
            video_codec: None,
            audio_codec: None,
            required_capabilities: OutputCapabilities::empty(),
        }
    }

    pub fn protocol(mut self, protocol: impl Into<String>) -> Self {
        self.protocol = Some(protocol.into());
        self
    }

    pub fn video_codec(mut self, codec: impl Into<String>) -> Self {
        self.video_codec = Some(codec.into());
        self
    }

    pub fn audio_codec(mut self, codec: impl Into<String>) -> Self {
        self.audio_codec = Some(codec.into());
        self
    }

    pub fn require_capabilities(mut self, capabilities: OutputCapabilities) -> Self {
        self.required_capabilities |= capabilities;
        self
    }

    pub fn matches(&self) -> Vec<&'a OutputTypeInfo> {
        let mut candidates = self
            .outputs
            .iter()
            .filter(|output| {
                self.protocol
                    .as_deref()
                    .is_none_or(|protocol| output.supports_protocol(protocol))
            })
            .filter(|output| {
                self.video_codec
                    .as_deref()
                    .is_none_or(|codec| output.supports_video_codec(codec))
            })
            .filter(|output| {
                self.audio_codec
                    .as_deref()
                    .is_none_or(|codec| output.supports_audio_codec(codec))
            })
            .filter(|output| {
                output
                    .capability_flags()
                    .contains(self.required_capabilities)
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| a.id.cmp(&b.id));
        candidates
    }

    pub fn best_available(&self) -> Option<&'a OutputTypeInfo> {
        self.matches().into_iter().next()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PropertyMetadata {
    pub name: String,
    pub description: Option<String>,
    pub long_description: Option<String>,
    pub enabled: bool,
    pub visible: bool,
    pub kind: PropertyKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PropertyKind {
    Invalid,
    Bool,
    Integer {
        min: i32,
        max: i32,
        step: i32,
        suffix: Option<String>,
        control: NumberControl,
    },
    Float {
        min: f64,
        max: f64,
        step: f64,
        suffix: Option<String>,
        control: NumberControl,
    },
    Text {
        text_type: TextType,
        monospace: bool,
        info_type: TextInfoType,
        word_wrap: bool,
    },
    Path {
        path_type: PathType,
        filter: Option<String>,
        default_path: Option<String>,
    },
    List {
        list_type: ListType,
        format: ListFormat,
        items: Vec<ListItem>,
    },
    Color,
    Button {
        button_type: ButtonType,
        url: Option<String>,
    },
    Font,
    EditableList {
        list_type: EditableListType,
        filter: Option<String>,
        default_path: Option<String>,
    },
    FrameRate {
        options: Vec<FrameRateOption>,
        ranges: Vec<FrameRateRange>,
    },
    Group {
        group_type: GroupType,
        properties: Vec<PropertyMetadata>,
    },
    ColorAlpha,
    /// A property type introduced by a newer libobs than this crate knows about.
    Unknown(i64),
}

macro_rules! raw_enum {
    ($name:ident { $($variant:ident = $constant:path),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant,)+
            Unknown(i64),
        }
        impl $name {
            fn from_raw(raw: i64) -> Self {
                $(if raw == $constant as i64 { return Self::$variant; })+
                Self::Unknown(raw)
            }
        }
    };
}

raw_enum!(NumberControl {
    Scroller = libobs::obs_number_type_OBS_NUMBER_SCROLLER,
    Slider = libobs::obs_number_type_OBS_NUMBER_SLIDER,
});
raw_enum!(TextType {
    Default = libobs::obs_text_type_OBS_TEXT_DEFAULT,
    Password = libobs::obs_text_type_OBS_TEXT_PASSWORD,
    Multiline = libobs::obs_text_type_OBS_TEXT_MULTILINE,
    Info = libobs::obs_text_type_OBS_TEXT_INFO,
});
raw_enum!(TextInfoType {
    Normal = libobs::obs_text_info_type_OBS_TEXT_INFO_NORMAL,
    Warning = libobs::obs_text_info_type_OBS_TEXT_INFO_WARNING,
    Error = libobs::obs_text_info_type_OBS_TEXT_INFO_ERROR,
});
raw_enum!(PathType {
    File = libobs::obs_path_type_OBS_PATH_FILE,
    FileSave = libobs::obs_path_type_OBS_PATH_FILE_SAVE,
    Directory = libobs::obs_path_type_OBS_PATH_DIRECTORY,
});
raw_enum!(ListType {
    Invalid = libobs::obs_combo_type_OBS_COMBO_TYPE_INVALID,
    Editable = libobs::obs_combo_type_OBS_COMBO_TYPE_EDITABLE,
    List = libobs::obs_combo_type_OBS_COMBO_TYPE_LIST,
    Radio = libobs::obs_combo_type_OBS_COMBO_TYPE_RADIO,
});
raw_enum!(ListFormat {
    Invalid = libobs::obs_combo_format_OBS_COMBO_FORMAT_INVALID,
    Int = libobs::obs_combo_format_OBS_COMBO_FORMAT_INT,
    Float = libobs::obs_combo_format_OBS_COMBO_FORMAT_FLOAT,
    String = libobs::obs_combo_format_OBS_COMBO_FORMAT_STRING,
    Bool = libobs::obs_combo_format_OBS_COMBO_FORMAT_BOOL,
});
raw_enum!(ButtonType {
    Default = libobs::obs_button_type_OBS_BUTTON_DEFAULT,
    Url = libobs::obs_button_type_OBS_BUTTON_URL,
});
raw_enum!(EditableListType {
    Strings = libobs::obs_editable_list_type_OBS_EDITABLE_LIST_TYPE_STRINGS,
    Files = libobs::obs_editable_list_type_OBS_EDITABLE_LIST_TYPE_FILES,
    FilesAndUrls = libobs::obs_editable_list_type_OBS_EDITABLE_LIST_TYPE_FILES_AND_URLS,
});
raw_enum!(GroupType {
    Invalid = libobs::obs_group_type_OBS_COMBO_INVALID,
    Normal = libobs::obs_group_type_OBS_GROUP_NORMAL,
    Checkable = libobs::obs_group_type_OBS_GROUP_CHECKABLE,
});

#[derive(Clone, Debug, PartialEq)]
pub struct ListItem {
    pub name: String,
    pub value: ListValue,
    pub disabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ListValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    UnknownFormat(i64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameRateOption {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRate {
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRateRange {
    pub min: FrameRate,
    pub max: FrameRate,
}

impl ObsSceneRef {
    /// Creates a source from a discovered descriptor and adds it directly to this scene.
    pub fn add_discovered_source(
        &mut self,
        source_type: &SourceTypeInfo,
        name: impl Into<ObsString>,
        settings: Option<ObsData>,
    ) -> Result<ObsSceneItemRef<ObsSourceRef>, ObsError> {
        self.runtime().ensure_same_runtime(&source_type.runtime)?;
        if source_type.kind == SourceKind::Filter {
            return Err(capability_kind_mismatch(
                &source_type.id,
                "source/input/transition",
                "filter",
            ));
        }
        ensure_settings_runtime(self.runtime(), settings.as_ref())?;
        let source = ObsSourceRef::new_from_info(
            ObjectInfo::new(source_type.id.as_str(), name, settings, None),
            self.runtime().clone(),
        )?;
        self.add(source)
    }
}

impl ObsContext {
    pub fn source_types(&self) -> Result<Vec<SourceTypeInfo>, ObsError> {
        discover_sources(self.runtime())
    }

    pub fn input_types(&self) -> Result<Vec<SourceTypeInfo>, ObsError> {
        discover_source_category(
            self.runtime(),
            SourceKind::Input,
            libobs::obs_enum_input_types,
        )
    }

    pub fn filter_types(&self) -> Result<Vec<SourceTypeInfo>, ObsError> {
        discover_source_category(
            self.runtime(),
            SourceKind::Filter,
            libobs::obs_enum_filter_types,
        )
    }

    pub fn transition_types(&self) -> Result<Vec<SourceTypeInfo>, ObsError> {
        discover_source_category(
            self.runtime(),
            SourceKind::Transition,
            libobs::obs_enum_transition_types,
        )
    }

    pub fn output_types(&self) -> Result<Vec<OutputTypeInfo>, ObsError> {
        discover_outputs(self.runtime())
    }

    pub fn encoder_types(&self) -> Result<Vec<EncoderTypeInfo>, ObsError> {
        discover_encoders(self.runtime())
    }

    pub fn service_types(&self) -> Result<Vec<ServiceTypeInfo>, ObsError> {
        discover_services(self.runtime())
    }

    pub fn protocols(&self) -> Result<Vec<String>, ObsError> {
        discover_protocols(self.runtime())
    }

    pub fn loaded_modules(&self) -> Result<Vec<ModuleInfo>, ObsError> {
        discover_modules(self.runtime())
    }

    pub fn source_type(&self, id: &str) -> Result<Option<SourceTypeInfo>, ObsError> {
        Ok(self.source_types()?.into_iter().find(|info| info.id == id))
    }

    pub fn output_type(&self, id: &str) -> Result<Option<OutputTypeInfo>, ObsError> {
        Ok(self.output_types()?.into_iter().find(|info| info.id == id))
    }

    pub fn encoder_type(&self, id: &str) -> Result<Option<EncoderTypeInfo>, ObsError> {
        Ok(self.encoder_types()?.into_iter().find(|info| info.id == id))
    }

    pub fn service_type(&self, id: &str) -> Result<Option<ServiceTypeInfo>, ObsError> {
        Ok(self.service_types()?.into_iter().find(|info| info.id == id))
    }

    /// Creates a typed source from a discovered source descriptor. Filters use
    /// [`ObsContext::create_filter`] so callers cannot accidentally lose filter semantics.
    pub fn create_source(
        &self,
        source_type: &SourceTypeInfo,
        name: impl Into<ObsString>,
        settings: Option<ObsData>,
    ) -> Result<ObsSourceRef, ObsError> {
        self.runtime().ensure_same_runtime(&source_type.runtime)?;
        if source_type.kind == SourceKind::Filter {
            return Err(capability_kind_mismatch(
                &source_type.id,
                "source/input/transition",
                "filter",
            ));
        }
        ensure_settings_runtime(self.runtime(), settings.as_ref())?;
        ObsSourceRef::new_from_info(
            ObjectInfo::new(source_type.id.as_str(), name, settings, None),
            self.runtime().clone(),
        )
    }

    /// Creates and registers a typed filter from a discovered filter descriptor.
    pub fn create_filter(
        &mut self,
        filter_type: &SourceTypeInfo,
        name: impl Into<ObsString>,
        settings: Option<ObsData>,
    ) -> Result<ObsFilterRef, ObsError> {
        self.runtime().ensure_same_runtime(&filter_type.runtime)?;
        if filter_type.kind != SourceKind::Filter {
            return Err(capability_kind_mismatch(
                &filter_type.id,
                "filter",
                &format!("{:?}", filter_type.kind),
            ));
        }
        ensure_settings_runtime(self.runtime(), settings.as_ref())?;
        self.obs_filter(ObjectInfo::new(
            filter_type.id.as_str(),
            name,
            settings,
            None,
        ))
    }

    /// Creates and registers an output from a discovered output descriptor.
    pub fn create_output(
        &mut self,
        output_type: &OutputTypeInfo,
        name: impl Into<ObsString>,
        settings: Option<ObsData>,
    ) -> Result<ObsOutputRef, ObsError> {
        self.runtime().ensure_same_runtime(&output_type.runtime)?;
        ensure_settings_runtime(self.runtime(), settings.as_ref())?;
        self.output(ObjectInfo::new(
            output_type.id.as_str(),
            name,
            settings,
            None,
        ))
    }

    /// Creates a video encoder from a discovered encoder descriptor.
    pub fn create_video_encoder(
        &self,
        encoder_type: &EncoderTypeInfo,
        name: impl Into<ObsString>,
        settings: Option<ObsData>,
    ) -> Result<std::sync::Arc<ObsVideoEncoder>, ObsError> {
        self.runtime().ensure_same_runtime(&encoder_type.runtime)?;
        if encoder_type.kind != EncoderKind::Video {
            return Err(capability_kind_mismatch(
                &encoder_type.id,
                "video encoder",
                &format!("{:?}", encoder_type.kind),
            ));
        }
        ensure_settings_runtime(self.runtime(), settings.as_ref())?;
        ObsVideoEncoder::new_from_info(
            ObjectInfo::new(encoder_type.id.as_str(), name, settings, None),
            self.runtime().clone(),
        )
    }

    /// Creates an audio encoder from a discovered encoder descriptor.
    pub fn create_audio_encoder(
        &self,
        encoder_type: &EncoderTypeInfo,
        name: impl Into<ObsString>,
        settings: Option<ObsData>,
        mixer_index: usize,
    ) -> Result<std::sync::Arc<ObsAudioEncoder>, ObsError> {
        self.runtime().ensure_same_runtime(&encoder_type.runtime)?;
        if encoder_type.kind != EncoderKind::Audio {
            return Err(capability_kind_mismatch(
                &encoder_type.id,
                "audio encoder",
                &format!("{:?}", encoder_type.kind),
            ));
        }
        ensure_settings_runtime(self.runtime(), settings.as_ref())?;
        ObsAudioEncoder::new_from_info(
            ObjectInfo::new(encoder_type.id.as_str(), name, settings, None),
            mixer_index,
            self.runtime().clone(),
        )
    }

    /// Creates a managed streaming service from a discovered service descriptor.
    pub fn create_service(
        &self,
        service_type: &ServiceTypeInfo,
        name: impl Into<ObsString>,
        settings: Option<ObsData>,
    ) -> Result<std::sync::Arc<ObsServiceRef>, ObsError> {
        self.runtime().ensure_same_runtime(&service_type.runtime)?;
        ensure_settings_runtime(self.runtime(), settings.as_ref())?;
        ObsServiceRef::new_from_info(
            ObjectInfo::new(service_type.id.as_str(), name, settings, None),
            self.runtime().clone(),
        )
    }

    pub fn capabilities(&self) -> Result<ObsCapabilities, ObsError> {
        Ok(ObsCapabilities {
            source_types: self.source_types()?,
            input_types: self.input_types()?,
            filter_types: self.filter_types()?,
            transition_types: self.transition_types()?,
            outputs: self.output_types()?,
            encoders: self.encoder_types()?,
            services: self.service_types()?,
            protocols: self.protocols()?,
            modules: self.loaded_modules()?,
        })
    }
}

type EnumTypeFn = unsafe extern "C" fn(usize, *mut *const c_char) -> bool;
type PropertiesFn = unsafe extern "C" fn(*const c_char) -> *mut libobs::obs_properties_t;
type DefaultsFn = unsafe extern "C" fn(*const c_char) -> *mut libobs::obs_data_t;

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
/// # Safety
/// This helper may only be called from an `ObsRuntime` actor command. All native pointers
/// supplied to it must remain valid for the duration of the call.
unsafe fn enum_ids_on_actor(enum_fn: EnumTypeFn) -> Vec<String> {
    let mut result = Vec::new();
    let mut index = 0;
    loop {
        let mut id = ptr::null();
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        if !unsafe { enum_fn(index, &mut id) } {
            break;
        }
        index += 1;
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        if let Some(id) = unsafe { cstr_owned(id) } {
            result.push(id);
        }
    }
    result.sort();
    result.dedup();
    result
}

fn discover_sources(runtime: &ObsRuntime) -> Result<Vec<SourceTypeInfo>, ObsError> {
    let runtime_for_result = runtime.clone();
    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
    let rows = run_with_obs!(runtime, move || unsafe {
        let inputs: HashSet<_> = enum_ids_on_actor(libobs::obs_enum_input_types)
            .into_iter()
            .collect();
        let filters: HashSet<_> = enum_ids_on_actor(libobs::obs_enum_filter_types)
            .into_iter()
            .collect();
        let transitions: HashSet<_> = enum_ids_on_actor(libobs::obs_enum_transition_types)
            .into_iter()
            .collect();
        enum_ids_on_actor(libobs::obs_enum_source_types)
            .into_iter()
            .map(|id| {
                let c_id = ObsString::new(&id);
                let display_name = cstr_owned(libobs::obs_source_get_display_name(c_id.as_ptr().0));
                let output_flags = libobs::obs_get_source_output_flags(c_id.as_ptr().0);
                let kind = if inputs.contains(&id) {
                    SourceKind::Input
                } else if filters.contains(&id) {
                    SourceKind::Filter
                } else if transitions.contains(&id) {
                    SourceKind::Transition
                } else {
                    SourceKind::Other
                };
                (id, display_name, kind, output_flags)
            })
            .collect::<Vec<_>>()
    })?;
    Ok(rows
        .into_iter()
        .map(|(id, display_name, kind, output_flags)| SourceTypeInfo {
            id,
            display_name,
            kind,
            output_flags,
            runtime: runtime_for_result.clone(),
        })
        .collect())
}

fn discover_source_category(
    runtime: &ObsRuntime,
    kind: SourceKind,
    enum_fn: EnumTypeFn,
) -> Result<Vec<SourceTypeInfo>, ObsError> {
    let runtime_for_result = runtime.clone();
    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
    let rows = run_with_obs!(runtime, move || unsafe {
        enum_ids_on_actor(enum_fn)
            .into_iter()
            .map(|id| {
                let c_id = ObsString::new(&id);
                (
                    id,
                    cstr_owned(libobs::obs_source_get_display_name(c_id.as_ptr().0)),
                    libobs::obs_get_source_output_flags(c_id.as_ptr().0),
                )
            })
            .collect::<Vec<_>>()
    })?;
    Ok(rows
        .into_iter()
        .map(|(id, display_name, output_flags)| SourceTypeInfo {
            id,
            display_name,
            kind,
            output_flags,
            runtime: runtime_for_result.clone(),
        })
        .collect())
}

fn discover_outputs(runtime: &ObsRuntime) -> Result<Vec<OutputTypeInfo>, ObsError> {
    let runtime_for_result = runtime.clone();
    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
    let rows = run_with_obs!(runtime, move || unsafe {
        let ids = enum_ids_on_actor(libobs::obs_enum_output_types);
        let mut protocols_by_output = HashMap::<String, Vec<String>>::new();
        let mut protocol_index = 0;
        loop {
            let mut protocol = ptr::null_mut();
            if !libobs::obs_enum_output_protocols(protocol_index, &mut protocol) {
                break;
            }
            protocol_index += 1;
            let Some(protocol_name) = cstr_owned(protocol.cast_const()) else {
                continue;
            };
            let mut matching_ids = HashSet::<String>::new();
            libobs::obs_enum_output_types_with_protocol(
                protocol.cast_const(),
                (&mut matching_ids as *mut HashSet<String>).cast::<c_void>(),
                Some(collect_output_type_id),
            );
            for id in matching_ids {
                protocols_by_output
                    .entry(id)
                    .or_default()
                    .push(protocol_name.clone());
            }
        }

        ids.into_iter()
            .map(|id| {
                let c_id = ObsString::new(&id);
                let mut protocols = protocols_by_output.remove(&id).unwrap_or_default();
                protocols.sort();
                protocols.dedup();
                (
                    id,
                    cstr_owned(libobs::obs_output_get_display_name(c_id.as_ptr().0)),
                    split_capability_string(libobs::obs_get_output_supported_video_codecs(
                        c_id.as_ptr().0,
                    )),
                    split_capability_string(libobs::obs_get_output_supported_audio_codecs(
                        c_id.as_ptr().0,
                    )),
                    protocols,
                    libobs::obs_get_output_flags(c_id.as_ptr().0),
                )
            })
            .collect::<Vec<_>>()
    })?;
    Ok(rows
        .into_iter()
        .map(
            |(id, display_name, video_codecs, audio_codecs, protocols, flags)| OutputTypeInfo {
                id,
                display_name,
                video_codecs,
                audio_codecs,
                protocols,
                flags,
                runtime: runtime_for_result.clone(),
            },
        )
        .collect())
}

/// # Safety
/// `data` must point to a live `HashSet<String>` for this synchronous libobs enumeration
/// and `id` must be null or a valid callback-borrowed C string for the duration of the call.
unsafe extern "C" fn collect_output_type_id(data: *mut c_void, id: *const c_char) -> bool {
    let Some(ids) = data.cast::<HashSet<String>>().as_mut() else {
        return false;
    };
    if let Some(id) = cstr_owned(id) {
        ids.insert(id);
    }
    true
}

fn discover_encoders(runtime: &ObsRuntime) -> Result<Vec<EncoderTypeInfo>, ObsError> {
    let runtime_for_result = runtime.clone();
    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
    let rows = run_with_obs!(runtime, move || unsafe {
        enum_ids_on_actor(libobs::obs_enum_encoder_types)
            .into_iter()
            .map(|id| {
                let c_id = ObsString::new(&id);
                let raw_kind = libobs::obs_get_encoder_type(c_id.as_ptr().0) as i64;
                let kind = if raw_kind == libobs::obs_encoder_type_OBS_ENCODER_AUDIO as i64 {
                    EncoderKind::Audio
                } else if raw_kind == libobs::obs_encoder_type_OBS_ENCODER_VIDEO as i64 {
                    EncoderKind::Video
                } else {
                    EncoderKind::Unknown(raw_kind)
                };
                (
                    id,
                    cstr_owned(libobs::obs_encoder_get_display_name(c_id.as_ptr().0)),
                    kind,
                    cstr_owned(libobs::obs_get_encoder_codec(c_id.as_ptr().0)),
                    libobs::obs_get_encoder_caps(c_id.as_ptr().0),
                )
            })
            .collect::<Vec<_>>()
    })?;
    Ok(rows
        .into_iter()
        .map(
            |(id, display_name, kind, codec, capabilities)| EncoderTypeInfo {
                id,
                display_name,
                kind,
                codec,
                capabilities,
                runtime: runtime_for_result.clone(),
            },
        )
        .collect())
}

fn discover_services(runtime: &ObsRuntime) -> Result<Vec<ServiceTypeInfo>, ObsError> {
    let runtime_for_result = runtime.clone();
    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
    let rows = run_with_obs!(runtime, move || unsafe {
        enum_ids_on_actor(libobs::obs_enum_service_types)
            .into_iter()
            .map(|id| {
                let c_id = ObsString::new(&id);
                (
                    id,
                    cstr_owned(libobs::obs_service_get_display_name(c_id.as_ptr().0)),
                )
            })
            .collect::<Vec<_>>()
    })?;
    Ok(rows
        .into_iter()
        .map(|(id, display_name)| ServiceTypeInfo {
            id,
            display_name,
            runtime: runtime_for_result.clone(),
        })
        .collect())
}

fn discover_protocols(runtime: &ObsRuntime) -> Result<Vec<String>, ObsError> {
    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
    run_with_obs!(runtime, move || unsafe {
        let mut result = Vec::new();
        let mut index = 0;
        loop {
            let mut protocol: *mut c_char = ptr::null_mut();
            if !libobs::obs_enum_output_protocols(index, &mut protocol) {
                break;
            }
            index += 1;
            if let Some(protocol) = cstr_owned(protocol.cast_const()) {
                result.push(protocol);
            }
        }
        result.sort();
        result.dedup();
        result
    })
}

fn discover_modules(runtime: &ObsRuntime) -> Result<Vec<ModuleInfo>, ObsError> {
    #[allow(unknown_lints)]
    #[allow(ensure_obs_call_in_runtime)]
    /// # Safety
    /// Called synchronously by `obs_enum_modules` on the OBS actor. `param` points to the
    /// live `Vec<ModuleInfo>` owned by `discover_modules` for the complete enumeration.
    unsafe extern "C" fn collect(param: *mut std::ffi::c_void, module: *mut libobs::obs_module_t) {
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        let rows = unsafe { &mut *(param as *mut Vec<ModuleInfo>) };
        rows.push(ModuleInfo {
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            file_name: unsafe { cstr_owned(libobs::obs_get_module_file_name(module)) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            name: unsafe { cstr_owned(libobs::obs_get_module_name(module)) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            author: unsafe { cstr_owned(libobs::obs_get_module_author(module)) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            description: unsafe { cstr_owned(libobs::obs_get_module_description(module)) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            binary_path: unsafe { cstr_owned(libobs::obs_get_module_binary_path(module)) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            data_path: unsafe { cstr_owned(libobs::obs_get_module_data_path(module)) },
        });
    }

    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
    run_with_obs!(runtime, move || unsafe {
        let mut rows: Vec<ModuleInfo> = Vec::new();
        // SAFETY: obs_enum_modules is synchronous; the Vec address remains valid for the
        // complete callback sequence and only owned strings are retained afterward.
        libobs::obs_enum_modules(Some(collect), (&mut rows as *mut Vec<ModuleInfo>).cast());
        rows.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        rows
    })
}

fn type_properties(
    runtime: &ObsRuntime,
    id: &str,
    getter: PropertiesFn,
) -> Result<Vec<PropertyMetadata>, ObsError> {
    let id = ObsString::new(id);
    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
    run_with_obs!(runtime, (id), move || unsafe {
        // SAFETY: The property tree is created, fully copied, and destroyed on the actor.
        let properties = getter(id.as_ptr().0);
        snapshot_owned_properties(properties)
    })
}

fn type_properties_for_settings(
    runtime: &ObsRuntime,
    id: &str,
    getter: PropertiesFn,
    settings: &ObsData,
) -> Result<Vec<PropertyMetadata>, ObsError> {
    runtime.ensure_same_runtime(settings.runtime())?;
    let id = ObsString::new(id);
    let settings_ptr = settings.as_ptr();
    run_with_obs!(runtime, (id, settings_ptr), move || unsafe {
        // Safety: the property tree and settings handle are live on the OBS actor. Applying
        // settings invokes plugin property-modified callbacks synchronously before snapshotting.
        let properties = getter(id.as_ptr().0);
        if properties.is_null() {
            return Vec::new();
        }
        libobs::obs_properties_apply_settings(properties, settings_ptr.get_ptr());
        snapshot_owned_properties(properties)
    })
}

fn source_properties(runtime: &ObsRuntime, id: &str) -> Result<Vec<PropertyMetadata>, ObsError> {
    type_properties(runtime, id, libobs::obs_get_source_properties)
}

fn source_properties_for_settings(
    runtime: &ObsRuntime,
    id: &str,
    settings: &ObsData,
) -> Result<Vec<PropertyMetadata>, ObsError> {
    type_properties_for_settings(runtime, id, libobs::obs_get_source_properties, settings)
}

fn default_settings(
    runtime: &ObsRuntime,
    id: &str,
    getter: DefaultsFn,
) -> Result<Option<ImmutableObsData>, ObsError> {
    let id = ObsString::new(id);
    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
    let ptr = run_with_obs!(runtime, (id), move || unsafe {
        // SAFETY: Type-default lookup runs on the OBS actor and returns an owned obs_data_t.
        Sendable(getter(id.as_ptr().0))
    })?;
    if ptr.0.is_null() {
        Ok(None)
    } else {
        Ok(Some(ImmutableObsData::from_raw_pointer(
            ptr,
            runtime.clone(),
        )))
    }
}

fn mutable_default_settings(
    runtime: &ObsRuntime,
    id: &str,
    getter: DefaultsFn,
) -> Result<ObsData, ObsError> {
    let id = ObsString::new(id);
    // Safety: the defaults callback is invoked on the OBS actor and returns an owned
    // obs_data_t reference when non-null.
    let ptr = run_with_obs!(runtime, (id), move || unsafe {
        Sendable(getter(id.as_ptr().0))
    })?;
    if ptr.0.is_null() {
        ObsData::new(runtime.clone())
    } else {
        Ok(ObsData::from_raw_pointer(ptr, runtime.clone()))
    }
}

fn ensure_settings_runtime(
    runtime: &ObsRuntime,
    settings: Option<&ObsData>,
) -> Result<(), ObsError> {
    if let Some(settings) = settings {
        runtime.ensure_same_runtime(settings.runtime())?;
    }
    Ok(())
}

fn capability_kind_mismatch(id: &str, expected: &str, actual: &str) -> ObsError {
    ObsError::CapabilityKindMismatch {
        id: id.to_owned(),
        expected: expected.to_owned(),
        actual: actual.to_owned(),
    }
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
/// # Safety
/// This helper may only be called from an `ObsRuntime` actor command. All native pointers
/// supplied to it must remain valid for the duration of the call.
unsafe fn snapshot_owned_properties(
    properties: *mut libobs::obs_properties_t,
) -> Vec<PropertyMetadata> {
    if properties.is_null() {
        return Vec::new();
    }
    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
    let result = unsafe { snapshot_properties(properties) };
    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
    unsafe { libobs::obs_properties_destroy(properties) };
    result
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
/// # Safety
/// This helper may only be called from an `ObsRuntime` actor command. All native pointers
/// supplied to it must remain valid for the duration of the call.
unsafe fn snapshot_properties(properties: *mut libobs::obs_properties_t) -> Vec<PropertyMetadata> {
    let mut result = Vec::new();
    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
    let mut property = unsafe { libobs::obs_properties_first(properties) };
    while !property.is_null() {
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        let name = unsafe { cstr_owned(libobs::obs_property_name(property)) }.unwrap_or_default();
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        let description = unsafe { cstr_owned(libobs::obs_property_description(property)) };
        let long_description =
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            unsafe { cstr_owned(libobs::obs_property_long_description(property)) };
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        let enabled = unsafe { libobs::obs_property_enabled(property) };
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        let visible = unsafe { libobs::obs_property_visible(property) };
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        let raw_type = unsafe { libobs::obs_property_get_type(property) } as i64;
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        let kind = unsafe { snapshot_property_kind(property, raw_type) };
        result.push(PropertyMetadata {
            name,
            description,
            long_description,
            enabled,
            visible,
            kind,
        });
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        if !unsafe { libobs::obs_property_next(&mut property) } {
            break;
        }
    }
    result
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
/// # Safety
/// This helper may only be called from an `ObsRuntime` actor command. All native pointers
/// supplied to it must remain valid for the duration of the call.
unsafe fn snapshot_property_kind(
    property: *mut libobs::obs_property_t,
    raw_type: i64,
) -> PropertyKind {
    if raw_type == libobs::obs_property_type_OBS_PROPERTY_INVALID as i64 {
        PropertyKind::Invalid
    } else if raw_type == libobs::obs_property_type_OBS_PROPERTY_BOOL as i64 {
        PropertyKind::Bool
    } else if raw_type == libobs::obs_property_type_OBS_PROPERTY_INT as i64 {
        PropertyKind::Integer {
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            min: unsafe { libobs::obs_property_int_min(property) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            max: unsafe { libobs::obs_property_int_max(property) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            step: unsafe { libobs::obs_property_int_step(property) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            suffix: unsafe { cstr_owned(libobs::obs_property_int_suffix(property)) },
            control: NumberControl::from_raw(
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                unsafe { libobs::obs_property_int_type(property) } as i64,
            ),
        }
    } else if raw_type == libobs::obs_property_type_OBS_PROPERTY_FLOAT as i64 {
        PropertyKind::Float {
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            min: unsafe { libobs::obs_property_float_min(property) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            max: unsafe { libobs::obs_property_float_max(property) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            step: unsafe { libobs::obs_property_float_step(property) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            suffix: unsafe { cstr_owned(libobs::obs_property_float_suffix(property)) },
            control: NumberControl::from_raw(
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                unsafe { libobs::obs_property_float_type(property) } as i64,
            ),
        }
    } else if raw_type == libobs::obs_property_type_OBS_PROPERTY_TEXT as i64 {
        PropertyKind::Text {
            text_type: TextType::from_raw(
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                unsafe { libobs::obs_property_text_type(property) } as i64,
            ),
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            monospace: unsafe { libobs::obs_property_text_monospace(property) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            info_type: TextInfoType::from_raw(unsafe {
                libobs::obs_property_text_info_type(property)
            } as i64),
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            word_wrap: unsafe { libobs::obs_property_text_info_word_wrap(property) },
        }
    } else if raw_type == libobs::obs_property_type_OBS_PROPERTY_PATH as i64 {
        PropertyKind::Path {
            path_type: PathType::from_raw(
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                unsafe { libobs::obs_property_path_type(property) } as i64,
            ),
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            filter: unsafe { cstr_owned(libobs::obs_property_path_filter(property)) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            default_path: unsafe { cstr_owned(libobs::obs_property_path_default_path(property)) },
        }
    } else if raw_type == libobs::obs_property_type_OBS_PROPERTY_LIST as i64 {
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        let format_raw = unsafe { libobs::obs_property_list_format(property) } as i64;
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        let count = unsafe { libobs::obs_property_list_item_count(property) };
        let mut items = Vec::with_capacity(count);
        for index in 0..count {
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            let name = unsafe { cstr_owned(libobs::obs_property_list_item_name(property, index)) }
                .unwrap_or_default();
            let value = if format_raw == libobs::obs_combo_format_OBS_COMBO_FORMAT_STRING as i64 {
                ListValue::String(
                    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                    unsafe { cstr_owned(libobs::obs_property_list_item_string(property, index)) }
                        .unwrap_or_default(),
                )
            } else if format_raw == libobs::obs_combo_format_OBS_COMBO_FORMAT_INT as i64 {
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                ListValue::Int(unsafe { libobs::obs_property_list_item_int(property, index) })
            } else if format_raw == libobs::obs_combo_format_OBS_COMBO_FORMAT_FLOAT as i64 {
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                ListValue::Float(unsafe { libobs::obs_property_list_item_float(property, index) })
            } else if format_raw == libobs::obs_combo_format_OBS_COMBO_FORMAT_BOOL as i64 {
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                ListValue::Bool(unsafe { libobs::obs_property_list_item_bool(property, index) })
            } else {
                ListValue::UnknownFormat(format_raw)
            };
            items.push(ListItem {
                name,
                value,
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                disabled: unsafe { libobs::obs_property_list_item_disabled(property, index) },
            });
        }
        PropertyKind::List {
            list_type: ListType::from_raw(
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                unsafe { libobs::obs_property_list_type(property) } as i64,
            ),
            format: ListFormat::from_raw(format_raw),
            items,
        }
    } else if raw_type == libobs::obs_property_type_OBS_PROPERTY_COLOR as i64 {
        PropertyKind::Color
    } else if raw_type == libobs::obs_property_type_OBS_PROPERTY_BUTTON as i64 {
        PropertyKind::Button {
            button_type: ButtonType::from_raw(
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                unsafe { libobs::obs_property_button_type(property) } as i64,
            ),
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            url: unsafe { cstr_owned(libobs::obs_property_button_url(property)) },
        }
    } else if raw_type == libobs::obs_property_type_OBS_PROPERTY_FONT as i64 {
        PropertyKind::Font
    } else if raw_type == libobs::obs_property_type_OBS_PROPERTY_EDITABLE_LIST as i64 {
        PropertyKind::EditableList {
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            list_type: EditableListType::from_raw(unsafe {
                libobs::obs_property_editable_list_type(property)
            } as i64),
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            filter: unsafe { cstr_owned(libobs::obs_property_editable_list_filter(property)) },
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            default_path: unsafe {
                cstr_owned(libobs::obs_property_editable_list_default_path(property))
            },
        }
    } else if raw_type == libobs::obs_property_type_OBS_PROPERTY_FRAME_RATE as i64 {
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        let option_count = unsafe { libobs::obs_property_frame_rate_options_count(property) };
        let mut options = Vec::with_capacity(option_count);
        for index in 0..option_count {
            options.push(FrameRateOption {
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                name: unsafe {
                    cstr_owned(libobs::obs_property_frame_rate_option_name(property, index))
                },
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                description: unsafe {
                    cstr_owned(libobs::obs_property_frame_rate_option_description(
                        property, index,
                    ))
                },
            });
        }
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        let range_count = unsafe { libobs::obs_property_frame_rate_fps_ranges_count(property) };
        let mut ranges = Vec::with_capacity(range_count);
        for index in 0..range_count {
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            let min = unsafe { libobs::obs_property_frame_rate_fps_range_min(property, index) };
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            let max = unsafe { libobs::obs_property_frame_rate_fps_range_max(property, index) };
            ranges.push(FrameRateRange {
                min: FrameRate {
                    numerator: min.numerator,
                    denominator: min.denominator,
                },
                max: FrameRate {
                    numerator: max.numerator,
                    denominator: max.denominator,
                },
            });
        }
        PropertyKind::FrameRate { options, ranges }
    } else if raw_type == libobs::obs_property_type_OBS_PROPERTY_GROUP as i64 {
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        let content = unsafe { libobs::obs_property_group_content(property) };
        PropertyKind::Group {
            group_type: GroupType::from_raw(
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                unsafe { libobs::obs_property_group_type(property) } as i64,
            ),
            properties: if content.is_null() {
                Vec::new()
            } else {
                // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
                unsafe { snapshot_properties(content) }
            },
        }
    } else if raw_type == libobs::obs_property_type_OBS_PROPERTY_COLOR_ALPHA as i64 {
        PropertyKind::ColorAlpha
    } else {
        PropertyKind::Unknown(raw_type)
    }
}

/// # Safety
/// A non-null `ptr` must address a NUL-terminated C string that is readable for this call.
unsafe fn cstr_owned(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        Some(
            // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
            unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}

/// # Safety
/// A non-null `ptr` must address a NUL-terminated C string that is readable for this call.
fn capability_list_supports(values: &[String], requested: &str) -> bool {
    values.is_empty()
        || values
            .iter()
            .any(|value| value.eq_ignore_ascii_case(requested))
}

fn split_capability_string(ptr: *const c_char) -> Vec<String> {
    // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
    let Some(value) = (unsafe { cstr_owned(ptr) }) else {
        return Vec::new();
    };
    value
        .split(|c: char| c == ',' || c == ';' || c.is_ascii_whitespace())
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn future_enum_values_are_preserved() {
        assert_eq!(
            NumberControl::from_raw(99_999),
            NumberControl::Unknown(99_999)
        );
        assert_eq!(ListFormat::from_raw(-7), ListFormat::Unknown(-7));
    }

    #[test]
    fn capability_string_parser_is_owned_and_tolerant() {
        let text = std::ffi::CString::new("h264, hevc;av1").unwrap();
        // SAFETY: The surrounding actor/helper contract guarantees native pointer validity for this call.
        let parsed = split_capability_string(text.as_ptr());
        assert_eq!(parsed, ["h264", "hevc", "av1"]);
    }
}
