use std::{collections::HashMap, sync::Arc};

use crate::{
    data::{output::ObsOutputRef, properties::_ObsPropertiesDropGuard},
    run_with_obs,
    runtime::ObsRuntime,
    sources::ObsSourceTrait,
    unsafe_send::{Sendable, SmartPointerSendable},
    utils::{ObsError, ObsString},
};

use super::{property_ptr_to_struct, ObsProperty, ObsPropertyObject, ObsPropertyObjectPrivate};

impl<K: ObsSourceTrait> ObsPropertyObject for K {
    fn get_properties(&self) -> Result<HashMap<String, ObsProperty>, ObsError> {
        let properties_raw = self.get_properties_raw()?;
        property_ptr_to_struct(properties_raw, self.runtime().clone())
    }
}

impl<K: ObsSourceTrait> ObsPropertyObjectPrivate for K {
    fn get_properties_raw(
        &self,
    ) -> Result<SmartPointerSendable<*mut libobs::obs_properties_t>, ObsError> {
        let source_ptr = self.as_ptr();
        let runtime = self.runtime().clone();

        let raw_ptr = run_with_obs!(runtime, (source_ptr), move || {
            let source_ptr = source_ptr;
            let property_ptr = unsafe {
                // Safety: Safe because of smart pointer
                libobs::obs_source_properties(source_ptr.get_ptr())
            };

            if property_ptr.is_null() {
                Err(ObsError::NullPointer(None))
            } else {
                Ok(Sendable(property_ptr))
            }
        })??;

        let drop_guard = Arc::new(_ObsPropertiesDropGuard {
            properties: raw_ptr.clone(),
            runtime: self.runtime().clone(),
        });

        Ok(SmartPointerSendable::new(raw_ptr.0, drop_guard))
    }

    fn get_properties_by_id_raw<T: Into<ObsString> + Sync + Send>(
        id: T,
        runtime: ObsRuntime,
    ) -> Result<SmartPointerSendable<*mut libobs::obs_properties_t>, ObsError> {
        let id: ObsString = id.into();
        let raw_ptr = run_with_obs!(runtime, (id), move || {
            let id_ptr = id.as_ptr();
            let property_ptr = unsafe {
                // Safety: Safe because of smart pointer
                libobs::obs_get_source_properties(id_ptr.0)
            };

            if property_ptr.is_null() {
                Err(ObsError::NullPointer(None))
            } else {
                Ok(Sendable(property_ptr))
            }
        })??;

        let drop_guard = _ObsPropertiesDropGuard {
            properties: raw_ptr.clone(),
            runtime: runtime.clone(),
        };

        let ptr = SmartPointerSendable::new(raw_ptr.0, Arc::new(drop_guard));
        Ok(ptr)
    }
}

impl ObsPropertyObject for ObsOutputRef {
    fn get_properties(&self) -> Result<HashMap<String, ObsProperty>, ObsError> {
        let properties_raw = self.get_properties_raw()?;
        property_ptr_to_struct(properties_raw, self.runtime.clone())
    }
}

impl ObsPropertyObjectPrivate for ObsOutputRef {
    fn get_properties_raw(
        &self,
    ) -> Result<SmartPointerSendable<*mut libobs::obs_properties_t>, ObsError> {
        let output_ptr = self.output.clone();
        let ptr = run_with_obs!(self.runtime, (output_ptr), move || {
            let property_ptr = unsafe {
                // Safety: Safe because of smart pointer
                libobs::obs_output_properties(output_ptr.get_ptr())
            };

            if property_ptr.is_null() {
                Err(ObsError::NullPointer(None))
            } else {
                Ok(Sendable(property_ptr))
            }
        })??;

        let drop_guard = Arc::new(_ObsPropertiesDropGuard {
            properties: ptr.clone(),
            runtime: self.runtime.clone(),
        });

        Ok(SmartPointerSendable::new(ptr.0, drop_guard))
    }

    fn get_properties_by_id_raw<T: Into<ObsString> + Sync + Send>(
        id: T,
        runtime: ObsRuntime,
    ) -> Result<SmartPointerSendable<*mut libobs::obs_properties_t>, ObsError> {
        let id: ObsString = id.into();
        let ptr = run_with_obs!(runtime, (id), move || {
            let id_ptr = id.as_ptr();
            let property_ptr = unsafe {
                // Safety: Safe because of smart pointer
                libobs::obs_get_output_properties(id_ptr.0)
            };

            if property_ptr.is_null() {
                Err(ObsError::NullPointer(None))
            } else {
                Ok(Sendable(property_ptr))
            }
        })??;

        let drop_guard = _ObsPropertiesDropGuard {
            properties: ptr.clone(),
            runtime: runtime.clone(),
        };

        let ptr = SmartPointerSendable::new(ptr.0, Arc::new(drop_guard));
        Ok(ptr)
    }
}
