use libobs_wrapper::{
    data::ObsData,
    settings::{PropertyValue, SettingsSchema},
    utils::ObsError,
};

/// Whether generic simple-output settings may fall back to plugin defaults.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConfigurationMode {
    /// Keep plugin defaults for absent/rejected generic settings and describe that in the report.
    #[default]
    Permissive,
    /// Fail when a requested generic setting cannot be applied exactly.
    Strict,
}

/// What happened to one requested setting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingDisposition {
    Applied,
    Unsupported,
    Rejected { reason: String },
}

/// Effective result of one generic setting request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingConfiguration {
    pub component: String,
    pub name: String,
    pub requested: String,
    pub disposition: SettingDisposition,
}

/// Structured report produced while building a simple output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutputConfigurationReport {
    pub output_plugin_id: String,
    pub video_encoder_plugin_id: String,
    pub audio_encoder_plugin_ids: Vec<String>,
    pub settings: Vec<SettingConfiguration>,
    pub fallback_reasons: Vec<String>,
    pub attempts: Vec<EncoderAttempt>,
}

/// One concrete video-encoder attempt made by a managed simple workflow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncoderAttempt {
    pub plugin_id: String,
    pub stage: EncoderAttemptStage,
    pub outcome: EncoderAttemptOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncoderAttemptStage {
    Creation,
    Start,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncoderAttemptOutcome {
    Succeeded,
    Failed {
        error: String,
        fallback_eligible: bool,
    },
}

impl OutputConfigurationReport {
    pub(crate) fn setting(
        &mut self,
        component: impl Into<String>,
        name: &str,
        requested: &PropertyValue,
        disposition: SettingDisposition,
    ) {
        self.settings.push(SettingConfiguration {
            component: component.into(),
            name: name.to_owned(),
            requested: format!("{requested:?}"),
            disposition,
        });
    }
}

/// Applies a common cross-encoder setting while recording the effective result.
pub(crate) fn set_reported(
    schema: &SettingsSchema,
    settings: &mut ObsData,
    component: &str,
    name: &str,
    value: PropertyValue,
    mode: ConfigurationMode,
    report: &mut OutputConfigurationReport,
) -> Result<(), ObsError> {
    if schema.property(name).is_none() {
        report.setting(component, name, &value, SettingDisposition::Unsupported);
        return match mode {
            ConfigurationMode::Permissive => Ok(()),
            ConfigurationMode::Strict => Err(ObsError::PropertyNotFound {
                name: name.to_owned(),
            }),
        };
    }

    match schema.set(settings, name, value.clone()) {
        Ok(()) => {
            report.setting(component, name, &value, SettingDisposition::Applied);
            Ok(())
        }
        Err(
            error @ (ObsError::PropertyValueNotAllowed { .. }
            | ObsError::PropertyValueTypeMismatch { .. }),
        ) => {
            report.setting(
                component,
                name,
                &value,
                SettingDisposition::Rejected {
                    reason: error.to_string(),
                },
            );
            match mode {
                ConfigurationMode::Permissive => {
                    log::debug!(
                        "Keeping OBS default because generic setting '{name}' is not accepted by the selected plugin"
                    );
                    Ok(())
                }
                ConfigurationMode::Strict => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

/// Compatibility helper for existing callers that intentionally use permissive plugin defaults.
pub(super) fn set_if_supported(
    schema: &SettingsSchema,
    settings: &mut ObsData,
    name: &str,
    value: PropertyValue,
) -> Result<(), ObsError> {
    let mut ignored_report = OutputConfigurationReport::default();
    set_reported(
        schema,
        settings,
        "unspecified",
        name,
        value,
        ConfigurationMode::Permissive,
        &mut ignored_report,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_records_selected_plugins_and_attempts_without_log_parsing() {
        let mut report = OutputConfigurationReport {
            output_plugin_id: "ffmpeg_muxer".into(),
            video_encoder_plugin_id: "obs_x264".into(),
            audio_encoder_plugin_ids: vec!["ffmpeg_aac".into()],
            ..Default::default()
        };
        report
            .fallback_reasons
            .push("hardware encoder unavailable".into());
        report.attempts.push(EncoderAttempt {
            plugin_id: "obs_nvenc_h264_tex".into(),
            stage: EncoderAttemptStage::Creation,
            outcome: EncoderAttemptOutcome::Failed {
                error: "encoder unavailable".into(),
                fallback_eligible: true,
            },
        });

        assert_eq!(report.output_plugin_id, "ffmpeg_muxer");
        assert_eq!(report.fallback_reasons.len(), 1);
        assert_eq!(report.attempts.len(), 1);
    }
}
