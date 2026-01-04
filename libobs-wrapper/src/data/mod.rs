use std::{ffi::CString, sync::Arc};

use crate::{
    impl_obs_drop, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::{Sendable, SmartPointerSendable},
    utils::{ObsDropGuard, ObsError},
};
pub use immutable::ImmutableObsData;

pub mod audio;
mod immutable;
mod lib_support;
pub mod object;
pub mod output;
pub mod properties;
pub mod video;
pub use lib_support::*;
mod updater;
pub use updater::*;
mod traits;
pub use traits::*;

#[derive(Debug)]
pub(super) struct _ObsDataDropGuard {
    data_ptr: Sendable<*mut libobs::obs_data_t>,
    runtime: ObsRuntime,
}

impl_obs_drop!(_ObsDataDropGuard, (data_ptr), move || unsafe {
    // Safety: This is the drop guard, so the data_ptr must be valid here.
    libobs::obs_data_release(data_ptr.0)
});

impl ObsDropGuard for _ObsDataDropGuard {}

/// Contains `obs_data` and its related strings. Note that
/// this struct prevents string pointers from being freed
/// by keeping them owned.
/// Cloning `ObsData` is blocking and will create a new `ObsData` instance. Recommended is to use `ObsData::full_clone()` instead.
/// ## Panics
/// If the underlying JSON representation can not be parsed.
//NOTE: Update: The strings are actually copied by obs itself, we don't need to store them
#[derive(Debug)]
pub struct ObsData {
    pub(crate) runtime: ObsRuntime,
    ptr: SmartPointerSendable<*mut libobs::obs_data_t>,
}

impl ObsData {
    /// Creates a new empty `ObsData` wrapper for the
    /// libobs `obs_data` data structure.
    ///
    /// `ObsData` can then be populated using the set
    /// functions, which take ownership of the
    /// `ObsString` types to prevent them from being
    /// dropped prematurely. This makes it safer than
    /// using `obs_data` directly from libobs.
    pub fn new(runtime: ObsRuntime) -> Result<Self, ObsError> {
        let obs_data = run_with_obs!(runtime, move || unsafe {
            // Safety: We are in the runtime, so creating new obs_data_t is safe.
            Sendable(libobs::obs_data_create())
        })?;

        let drop_guard = Arc::new(_ObsDataDropGuard {
            data_ptr: obs_data.clone(),
            runtime: runtime.clone(),
        });
        let ptr = SmartPointerSendable::new(obs_data.0, drop_guard.clone());
        Ok(ObsData {
            ptr,
            runtime: runtime.clone(),
        })
    }

    pub fn bulk_update(&mut self) -> ObsDataUpdater {
        ObsDataUpdater::new(self.as_ptr(), self.runtime.clone())
    }

    pub fn from_json(json: &str, runtime: ObsRuntime) -> Result<Self, ObsError> {
        let cstr = CString::new(json).map_err(|_| ObsError::JsonParseError)?;

        let raw_ptr = run_with_obs!(runtime, (cstr), move || unsafe {
            // Safety: We made sure that the cstr pointer is valid during the call.
            Sendable(libobs::obs_data_create_from_json(cstr.as_ptr()))
        })?;

        if raw_ptr.0.is_null() {
            return Err(ObsError::JsonParseError);
        }

        let drop_guard = Arc::new(_ObsDataDropGuard {
            data_ptr: raw_ptr.clone(),
            runtime: runtime.clone(),
        });

        let ptr = SmartPointerSendable::new(raw_ptr.0, drop_guard.clone());

        Ok(ObsData {
            ptr,
            runtime: runtime.clone(),
        })
    }

    /// Converts this `ObsData` into an `ImmutableObsData`.
    /// Transfers the pointer without cloning.
    pub fn into_immutable(self) -> ImmutableObsData {
        ImmutableObsData::from(self)
    }
}

impl ObsDataPointers for ObsData {
    fn runtime(&self) -> &ObsRuntime {
        &self.runtime
    }

    fn as_ptr(&self) -> SmartPointerSendable<*mut libobs::obs_data_t> {
        self.ptr.clone()
    }
}

impl ObsDataGetters for ObsData {}
impl ObsDataSetters for ObsData {}

impl Clone for ObsData {
    fn clone(&self) -> Self {
        let json = self.get_json().unwrap();
        Self::from_json(json.as_str(), self.runtime.clone()).unwrap()
    }
}
