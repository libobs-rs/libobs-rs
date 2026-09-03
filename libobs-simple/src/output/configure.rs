use libobs_wrapper::{
    data::ObsData,
    settings::{PropertyValue, SettingsSchema},
    utils::ObsError,
};

/// Applies a common cross-encoder setting when the selected runtime plugin exposes it.
///
/// OBS encoder plugins do not share one universal preset/rate-control vocabulary. The simple layer
/// therefore treats absent or backend-specific enum values as “keep the plugin default”, while real
/// runtime/data errors still propagate to the caller.
pub(super) fn set_if_supported(
    schema: &SettingsSchema,
    settings: &mut ObsData,
    name: &str,
    value: PropertyValue,
) -> Result<(), ObsError> {
    if schema.property(name).is_none() {
        return Ok(());
    }
    match schema.set(settings, name, value) {
        Ok(()) => Ok(()),
        Err(
            ObsError::PropertyValueNotAllowed { .. } | ObsError::PropertyValueTypeMismatch { .. },
        ) => {
            log::debug!(
                "Keeping OBS default because generic setting '{name}' is not accepted by the selected plugin"
            );
            Ok(())
        }
        Err(error) => Err(error),
    }
}
