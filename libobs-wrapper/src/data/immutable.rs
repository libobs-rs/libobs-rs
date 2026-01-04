use std::{ffi::CStr, sync::Arc};

use libobs::obs_data_t;

use crate::{
    data::{ObsDataGetters, ObsDataPointers},
    run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::{Sendable, SmartPointerSendable},
    utils::ObsError,
};

use super::{ObsData, _ObsDataDropGuard};

#[derive(Clone, Debug)]
/// Immutable wrapper around obs_data_t to be prevent modification and to be used in creation of other objects.
/// This should not be updated directly using the pointer, but instead through the corresponding update methods on the holder of this data.
pub struct ImmutableObsData {
    runtime: ObsRuntime,
    ptr: SmartPointerSendable<*mut obs_data_t>,
}

impl ImmutableObsData {
    pub fn new(runtime: &ObsRuntime) -> Result<Self, ObsError> {
        let ptr = run_with_obs!(runtime, move || unsafe {
            // Safety: We are in the runtime, so creating new obs_data_t is safe.
            Sendable(libobs::obs_data_create())
        })?;

        let drop_guard = Arc::new(_ObsDataDropGuard {
            data_ptr: ptr.clone(),
            runtime: runtime.clone(),
        });

        let ptr = SmartPointerSendable::new(ptr.0, drop_guard);
        Ok(ImmutableObsData {
            ptr,
            runtime: runtime.clone(),
        })
    }

    pub fn from_raw_pointer(data: Sendable<*mut obs_data_t>, runtime: ObsRuntime) -> Self {
        ImmutableObsData {
            ptr: SmartPointerSendable::new(
                data.0,
                Arc::new(_ObsDataDropGuard {
                    data_ptr: data.clone(),
                    runtime: runtime.clone(),
                }),
            ),
            runtime,
        }
    }

    pub fn to_mutable(&self) -> Result<ObsData, ObsError> {
        let ptr = self.ptr.clone();
        let json = run_with_obs!(self.runtime, (ptr), move || {
            let json_ptr = unsafe {
                // Safety: We are making sure by using a SmartPointer, that this pointer is valid during the call.
                libobs::obs_data_get_json(ptr.get_ptr())
            };

            if json_ptr.is_null() {
                return Err(ObsError::NullPointer(Some(
                    "Couldn't get json representation of OBS data".into(),
                )));
            }

            let json = unsafe {
                // Safety: We made sure the json ptr is valid because it is not null.
                CStr::from_ptr(json_ptr)
            }
            .to_str()
            .map_err(|_| ObsError::JsonParseError)?
            .to_string();

            Ok(json)
        })??;

        ObsData::from_json(json.as_ref(), self.runtime.clone())
    }
}

impl ObsDataPointers for ImmutableObsData {
    fn runtime(&self) -> &ObsRuntime {
        &self.runtime
    }

    fn as_ptr(&self) -> SmartPointerSendable<*mut obs_data_t> {
        self.ptr.clone()
    }
}

impl ObsDataGetters for ImmutableObsData {}

impl From<ObsData> for ImmutableObsData {
    fn from(data: ObsData) -> Self {
        ImmutableObsData {
            ptr: data.as_ptr(),
            runtime: data.runtime.clone(),
        }
    }
}
