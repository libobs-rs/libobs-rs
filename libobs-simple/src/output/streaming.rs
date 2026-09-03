//! Dead-simple capability-driven RTMP streaming.
//!
//! This module intentionally chooses the common OBS pieces for the caller: an RTMP output,
//! H.264 video, AAC audio, and the standard `rtmp_custom` service. Concrete encoder backends are
//! selected from the loaded OBS installation, preferring hardware while retaining software fallback.

use libobs_wrapper::{
    capabilities::{OutputCapabilities, OutputCompatibilityRequest},
    context::ObsContext,
    data::output::ObsOutputRef,
    settings::PropertyValue,
    utils::{ObsError, ObsString},
};

use super::configure::set_if_supported;

/// Builder for a conventional H.264/AAC RTMP stream.
#[derive(Debug)]
pub struct SimpleRtmpStreamingBuilder {
    context: ObsContext,
    name: ObsString,
    server: String,
    stream_key: String,
    video_bitrate: u32,
    audio_bitrate: u32,
    prefer_hardware_video: bool,
}

/// Adds the common RTMP streaming workflow to [`ObsContext`].
pub trait ObsContextStreamingExt {
    /// Creates a simple RTMP builder using H.264/AAC and runtime capability selection.
    fn simple_rtmp_stream(
        &self,
        name: impl Into<ObsString>,
        server: impl Into<String>,
        stream_key: impl Into<String>,
    ) -> SimpleRtmpStreamingBuilder;
}

impl ObsContextStreamingExt for ObsContext {
    fn simple_rtmp_stream(
        &self,
        name: impl Into<ObsString>,
        server: impl Into<String>,
        stream_key: impl Into<String>,
    ) -> SimpleRtmpStreamingBuilder {
        SimpleRtmpStreamingBuilder::new(self.clone(), name, server, stream_key)
    }
}

impl SimpleRtmpStreamingBuilder {
    pub fn new(
        context: ObsContext,
        name: impl Into<ObsString>,
        server: impl Into<String>,
        stream_key: impl Into<String>,
    ) -> Self {
        Self {
            context,
            name: name.into(),
            server: server.into(),
            stream_key: stream_key.into(),
            video_bitrate: 6_000,
            audio_bitrate: 160,
            prefer_hardware_video: true,
        }
    }

    pub fn video_bitrate(mut self, bitrate_kbps: u32) -> Self {
        self.video_bitrate = bitrate_kbps;
        self
    }

    pub fn audio_bitrate(mut self, bitrate_kbps: u32) -> Self {
        self.audio_bitrate = bitrate_kbps;
        self
    }

    /// Controls whether hardware H.264 encoders are ranked ahead of software encoders.
    /// Software remains available as a fallback either way.
    pub fn prefer_hardware_video(mut self, prefer: bool) -> Self {
        self.prefer_hardware_video = prefer;
        self
    }

    /// Discovers, configures, validates, and creates the complete RTMP output graph.
    /// Calling this does not start network I/O; use [`libobs_wrapper::data::output::ObsOutputTrait::start`]
    /// when the application is ready to stream.
    pub fn build(self) -> Result<ObsOutputRef, ObsError> {
        let capabilities = self.context.capabilities()?;
        let request = OutputCompatibilityRequest::new()
            .protocol("RTMP")
            .video_codec("h264")
            .audio_codec("aac")
            .prefer_hardware_video(self.prefer_hardware_video)
            .require_output_capabilities(
                OutputCapabilities::ENCODED
                    | OutputCapabilities::VIDEO
                    | OutputCapabilities::AUDIO
                    | OutputCapabilities::SERVICE,
            );
        let plan = capabilities.best_output_plan(&request).map_err(|report| {
            ObsError::NoCompatibleOutputGraph {
                summary: report.summary(),
            }
        })?;
        let video_type = plan
            .video_encoder()
            .ok_or_else(|| ObsError::NoCompatibleOutputGraph {
                summary: "no H.264 video encoder was selected".into(),
            })?;
        let audio_type = plan
            .audio_encoder()
            .ok_or_else(|| ObsError::NoCompatibleOutputGraph {
                summary: "no AAC audio encoder was selected".into(),
            })?;

        let mut video_settings = video_type.default_settings_mut()?;
        let video_schema = video_type.settings_schema_for(&video_settings)?;
        set_if_supported(
            &video_schema,
            &mut video_settings,
            "rate_control",
            PropertyValue::String("CBR".into()),
        )?;
        set_if_supported(
            &video_schema,
            &mut video_settings,
            "bitrate",
            PropertyValue::Integer(i64::from(self.video_bitrate)),
        )?;
        let video_encoder = self.context.create_video_encoder(
            video_type,
            format!("{}_video", self.name),
            Some(video_settings),
        )?;

        let mut audio_settings = audio_type.default_settings_mut()?;
        let audio_schema = audio_type.settings_schema_for(&audio_settings)?;
        set_if_supported(
            &audio_schema,
            &mut audio_settings,
            "bitrate",
            PropertyValue::Integer(i64::from(self.audio_bitrate)),
        )?;
        let audio_encoder = self.context.create_audio_encoder(
            audio_type,
            format!("{}_audio", self.name),
            Some(audio_settings),
            0,
        )?;

        let service_type = self
            .context
            .service_type("rtmp_custom")?
            .ok_or_else(|| ObsError::SourceNotAvailable("rtmp_custom service".into()))?;
        let mut service_settings = service_type.default_settings_mut()?;
        let service_schema = service_type.settings_schema_for(&service_settings)?;
        service_schema.set(
            &mut service_settings,
            "server",
            PropertyValue::String(self.server),
        )?;
        service_schema.set(
            &mut service_settings,
            "key",
            PropertyValue::String(self.stream_key),
        )?;
        let service = self.context.create_service(
            &service_type,
            format!("{}_service", self.name),
            Some(service_settings),
        )?;

        let output_settings = plan.output().default_settings_mut()?;
        Ok(self
            .context
            .output_pipeline(plan.output(), self.name, Some(output_settings))
            .video_encoder(video_encoder)
            .audio_encoder(0, audio_encoder)
            .service(service)
            .build()?
            .into_output())
    }
}
