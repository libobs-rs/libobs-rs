use std::collections::HashMap;

use crate::{
    data::output::ObsOutputRef,
    runtime::ObsRuntime,
    sources::ObsSourceTrait,
    unsafe_send::Sendable,
    utils::{ObsError, ObsString},
};

use super::{get_properties_inner, ObsProperty, ObsPropertyObject, ObsPropertyObjectPrivate};

impl<K: ObsSourceTrait> ObsPropertyObject for K {
    fn get_properties(&self) -> Result<HashMap<String, ObsProperty>, ObsError> {
        let properties_raw = self.get_properties_raw()?;
        get_properties_inner(properties_raw, self.runtime().clone())
    }
}

impl<K: ObsSourceTrait> ObsPropertyObjectPrivate for K {
    fn get_properties_raw(&self) -> Result<Sendable<*mut libobs::obs_properties_t>, ObsError> {
        let source_ptr = self.as_ptr();
        self.runtime()
            .run_with_obs_result(move || unsafe {
                let source_ptr = source_ptr;

                Sendable(libobs::obs_source_properties(source_ptr.0))
            })
            .map_err(|e| ObsError::InvocationError(e.to_string()))
    }

    fn get_properties_by_id_raw<T: Into<ObsString> + Sync + Send>(
        id: T,
        runtime: ObsRuntime,
    ) -> Result<Sendable<*mut libobs::obs_properties_t>, ObsError> {
        let id: ObsString = id.into();
        let id_ptr = id.as_ptr();
        runtime
            .run_with_obs_result(move || unsafe {
                let id_ptr = id_ptr;
                Sendable(libobs::obs_get_source_properties(id_ptr.0))
            })
            .map_err(|e| ObsError::InvocationError(e.to_string()))
    }
}

impl ObsPropertyObject for ObsOutputRef {
    fn get_properties(&self) -> Result<HashMap<String, ObsProperty>, ObsError> {
        let properties_raw = self.get_properties_raw()?;
        get_properties_inner(properties_raw, self.runtime.clone())
    }
}

impl ObsPropertyObjectPrivate for ObsOutputRef {
    fn get_properties_raw(&self) -> Result<Sendable<*mut libobs::obs_properties_t>, ObsError> {
        let output_ptr = self.output.clone();
        self.runtime
            .run_with_obs_result(move || unsafe {
                let output_ptr = output_ptr;

                Sendable(libobs::obs_output_properties(output_ptr.0))
            })
            .map_err(|e| ObsError::InvocationError(e.to_string()))
    }

    fn get_properties_by_id_raw<T: Into<ObsString> + Sync + Send>(
        id: T,
        runtime: ObsRuntime,
    ) -> Result<Sendable<*mut libobs::obs_properties_t>, ObsError> {
        let id: ObsString = id.into();
        let id_ptr = id.as_ptr();
        runtime
            .run_with_obs_result(move || unsafe {
                let id_ptr = id_ptr;

                Sendable(libobs::obs_get_output_properties(id_ptr.0))
            })
            .map_err(|e| ObsError::InvocationError(e.to_string()))
    }
}
