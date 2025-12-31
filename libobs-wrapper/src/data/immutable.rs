use std::{ffi::CStr, sync::Arc};

use libobs::obs_data_t;

use crate::{
    data::{ObsDataGetters, ObsDataPointers},
    run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::Sendable,
    utils::ObsError,
};

use super::{ObsData, _ObsDataDropGuard};

#[derive(Clone, Debug)]
/// Immutable wrapper around obs_data_t to be prevent modification and to be used in creation of other objects.
/// This should not be updated directly using the pointer, but instead through the corresponding update methods on the holder of this data.
pub struct ImmutableObsData {
    ptr: Sendable<*mut obs_data_t>,
    runtime: ObsRuntime,
    _drop_guard: Arc<_ObsDataDropGuard>,
}

impl ImmutableObsData {
    pub fn new(runtime: &ObsRuntime) -> Result<Self, ObsError> {
        let ptr = run_with_obs!(runtime, move || unsafe {
            Sendable(libobs::obs_data_create())
        })?;

        Ok(ImmutableObsData {
            ptr: ptr.clone(),
            runtime: runtime.clone(),
            _drop_guard: Arc::new(_ObsDataDropGuard {
                obs_data: ptr,
                runtime: runtime.clone(),
            }),
        })
    }

    pub fn from_raw(data: Sendable<*mut obs_data_t>, runtime: ObsRuntime) -> Self {
        ImmutableObsData {
            ptr: data.clone(),
            runtime: runtime.clone(),
            _drop_guard: Arc::new(_ObsDataDropGuard {
                obs_data: data.clone(),
                runtime,
            }),
        }
    }

    pub fn to_mutable(&self) -> Result<ObsData, ObsError> {
        let ptr = self.ptr.clone();
        let json = run_with_obs!(self.runtime, (ptr), move || unsafe {
            Sendable(libobs::obs_data_get_json(ptr))
        })?;

        let json = unsafe { CStr::from_ptr(json.0) }
            .to_str()
            .map_err(|_| ObsError::JsonParseError)?
            .to_string();

        ObsData::from_json(json.as_ref(), self.runtime.clone())
    }
}

impl ObsDataPointers for ImmutableObsData {
    fn runtime(&self) -> &ObsRuntime {
        &self.runtime
    }

    fn as_ptr(&self) -> Sendable<*mut obs_data_t> {
        self.ptr.clone()
    }
}

impl ObsDataGetters for ImmutableObsData {}

impl From<ObsData> for ImmutableObsData {
    fn from(mut data: ObsData) -> Self {
        // Set to null pointer to prevent double free
        let ptr = data.obs_data.0;

        data.obs_data.0 = std::ptr::null_mut();
        ImmutableObsData {
            ptr: Sendable(ptr),
            runtime: data.runtime.clone(),
            _drop_guard: data._drop_guard,
        }
    }
}
