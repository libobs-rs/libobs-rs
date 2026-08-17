//! High-level validated output-pipeline construction.
//!
//! The builder validates the complete encoder/service graph against the selected output
//! descriptor before it creates or mutates an OBS output. Low-level attachment remains
//! available through [`super::ObsOutputTrait`] for callers that intentionally need it.

use std::sync::Arc;

use crate::{
    capabilities::{OutputCapabilities, OutputTypeInfo},
    context::ObsContext,
    data::{object::ObsObjectTrait, ObsData, ObsDataPointers},
    encoders::{audio::ObsAudioEncoder, video::ObsVideoEncoder, ObsEncoderTrait},
    services::ObsServiceRef,
    utils::{ObsError, ObsString},
};

use super::{ObsOutputComposition, ObsOutputRef, ObsOutputTrait};

/// Builder for one validated OBS output graph.
#[derive(Debug)]
pub struct ObsOutputPipelineBuilder {
    context: ObsContext,
    output_type: OutputTypeInfo,
    name: ObsString,
    settings: Option<ObsData>,
    composition: ObsOutputComposition,
}

impl ObsOutputPipelineBuilder {
    pub(crate) fn new(
        context: ObsContext,
        output_type: OutputTypeInfo,
        name: ObsString,
        settings: Option<ObsData>,
    ) -> Self {
        Self {
            context,
            output_type,
            name,
            settings,
            composition: ObsOutputComposition::new(),
        }
    }

    pub fn video_encoder(mut self, encoder: Arc<ObsVideoEncoder>) -> Self {
        self.composition = self.composition.with_video_encoder(encoder);
        self
    }

    pub fn audio_encoder(mut self, mixer_idx: usize, encoder: Arc<ObsAudioEncoder>) -> Self {
        self.composition = self.composition.with_audio_encoder(mixer_idx, encoder);
        self
    }

    pub fn service(mut self, service: Arc<ObsServiceRef>) -> Self {
        self.composition = self.composition.with_service(service);
        self
    }

    /// Validates the complete graph without creating an output.
    pub fn validate(&self) -> Result<(), ObsError> {
        self.context
            .runtime()
            .ensure_same_runtime(self.output_type.runtime())?;

        if let Some(settings) = self.settings.as_ref() {
            self.context
                .runtime()
                .ensure_same_runtime(settings.runtime())?;
        }

        let flags = self.output_type.capability_flags();
        let encoded = flags.contains(OutputCapabilities::ENCODED);
        let needs_video = encoded && flags.contains(OutputCapabilities::VIDEO);
        let needs_audio = encoded && flags.contains(OutputCapabilities::AUDIO);
        let needs_service = flags.contains(OutputCapabilities::SERVICE);

        let video_encoder = self.composition.video_encoder();
        if needs_video && video_encoder.is_none() {
            return Err(missing_component(&self.output_type, "a video encoder"));
        }
        if video_encoder.is_some() && !needs_video {
            return Err(unexpected_component(&self.output_type, "a video encoder"));
        }

        let audio_encoders = self.composition.audio_encoders();
        if needs_audio && audio_encoders.is_empty() {
            return Err(missing_component(
                &self.output_type,
                "at least one audio encoder",
            ));
        }
        if !audio_encoders.is_empty() && !needs_audio {
            return Err(unexpected_component(&self.output_type, "audio encoders"));
        }
        for (mixer_idx, encoder) in audio_encoders {
            if *mixer_idx >= libobs::MAX_AUDIO_MIXES as usize {
                return Err(ObsError::AudioMixerIndexOutOfBounds {
                    index: *mixer_idx,
                    max: (libobs::MAX_AUDIO_MIXES - 1) as usize,
                });
            }
            self.context
                .runtime()
                .ensure_same_runtime(encoder.runtime())?;
        }

        let service = self.composition.service();
        if needs_service && service.is_none() {
            return Err(missing_component(&self.output_type, "a service"));
        }
        if service.is_some() && !needs_service {
            return Err(unexpected_component(&self.output_type, "a service"));
        }

        if let Some(encoder) = video_encoder {
            self.context
                .runtime()
                .ensure_same_runtime(encoder.runtime())?;
            if let Some(codec) = encoder.codec()? {
                if !self.output_type.video_codecs().is_empty()
                    && !self.output_type.supports_video_codec(&codec)
                {
                    return Err(ObsError::OutputPipelineUnsupportedCodec {
                        output_id: self.output_type.id().to_owned(),
                        media: "video".to_owned(),
                        codec,
                    });
                }
            }
        }

        for encoder in audio_encoders.values() {
            if let Some(codec) = encoder.codec()? {
                if !self.output_type.audio_codecs().is_empty()
                    && !self.output_type.supports_audio_codec(&codec)
                {
                    return Err(ObsError::OutputPipelineUnsupportedCodec {
                        output_id: self.output_type.id().to_owned(),
                        media: "audio".to_owned(),
                        codec,
                    });
                }
            }
        }

        if let Some(service) = service {
            self.context
                .runtime()
                .ensure_same_runtime(service.runtime())?;
            if let Some(protocol) = service.protocol()? {
                if !self.output_type.protocols().is_empty()
                    && !self.output_type.supports_protocol(&protocol)
                {
                    return Err(ObsError::OutputPipelineUnsupportedProtocol {
                        output_id: self.output_type.id().to_owned(),
                        protocol,
                    });
                }
            }
        }

        Ok(())
    }

    /// Validates the complete graph, creates the output, then applies the desired state.
    /// No output is created when validation fails.
    pub fn build(mut self) -> Result<ObsOutputPipeline, ObsError> {
        self.validate()?;
        let output =
            self.context
                .create_output(&self.output_type, self.name, self.settings.take())?;
        output.apply_composition(self.composition)?;
        Ok(ObsOutputPipeline { output })
    }
}

/// A fully validated and wired output ready for lifecycle operations.
#[derive(Clone, Debug)]
pub struct ObsOutputPipeline {
    output: ObsOutputRef,
}

impl ObsOutputPipeline {
    pub fn output(&self) -> &ObsOutputRef {
        &self.output
    }

    pub fn start(&self) -> Result<(), ObsError> {
        self.output.start()
    }

    pub fn stop(&self) -> Result<(), ObsError> {
        self.output.stop()
    }

    pub fn is_active(&self) -> Result<bool, ObsError> {
        self.output.is_active()
    }

    pub fn into_output(self) -> ObsOutputRef {
        self.output
    }
}

impl ObsContext {
    /// Starts a high-level pipeline builder for a discovered output type.
    pub fn output_pipeline(
        &self,
        output_type: &OutputTypeInfo,
        name: impl Into<ObsString>,
        settings: Option<ObsData>,
    ) -> ObsOutputPipelineBuilder {
        ObsOutputPipelineBuilder::new(self.clone(), output_type.clone(), name.into(), settings)
    }
}

fn missing_component(output_type: &OutputTypeInfo, component: &str) -> ObsError {
    ObsError::OutputPipelineMissingComponent {
        output_id: output_type.id().to_owned(),
        component: component.to_owned(),
    }
}

fn unexpected_component(output_type: &OutputTypeInfo, component: &str) -> ObsError {
    ObsError::OutputPipelineUnexpectedComponent {
        output_id: output_type.id().to_owned(),
        component: component.to_owned(),
    }
}
