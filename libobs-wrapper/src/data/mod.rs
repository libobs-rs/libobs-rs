use std::{ffi::CString, sync::Arc};

use crate::{
    impl_obs_drop, run_with_obs, runtime::ObsRuntime, unsafe_send::Sendable, utils::ObsError,
};
pub use immutable::ImmutableObsData;
use libobs::obs_data;

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
pub(crate) struct _ObsDataDropGuard {
    obs_data: Sendable<*mut obs_data>,
    pub(crate) runtime: ObsRuntime,
}

/// Contains `obs_data` and its related strings. Note that
/// this struct prevents string pointers from being freed
/// by keeping them owned.
/// Cloning `ObsData` is blocking and will create a new `ObsData` instance. Recommended is to use `ObsData::full_clone()` instead.
/// ## Panics
/// If the underlying JSON representation can not be parsed.
//NOTE: Update: The strings are actually copied by obs itself, we don't need to store them
#[derive(Debug)]
pub struct ObsData {
    obs_data: Sendable<*mut obs_data>,
    pub(crate) runtime: ObsRuntime,
    pub(crate) _drop_guard: Arc<_ObsDataDropGuard>,
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
            Sendable(libobs::obs_data_create())
        })?;

        Ok(ObsData {
            obs_data: obs_data.clone(),
            runtime: runtime.clone(),
            _drop_guard: Arc::new(_ObsDataDropGuard { obs_data, runtime }),
        })
    }

    /// Returns a pointer to the raw `obs_data`
    /// represented by `ObsData`.
    pub fn as_ptr(&self) -> Sendable<*mut obs_data> {
        self.obs_data.clone()
    }

    pub fn bulk_update(&mut self) -> ObsDataUpdater {
        ObsDataUpdater {
            changes: Vec::new(),
            obs_data: self.obs_data.clone(),
            _drop_guard: self._drop_guard.clone(),
        }
    }

    pub fn from_json(json: &str, runtime: ObsRuntime) -> Result<Self, ObsError> {
        let cstr = CString::new(json).map_err(|_| ObsError::JsonParseError)?;

        let cstr_ptr = Sendable(cstr.as_ptr());
        let result = run_with_obs!(runtime, (cstr_ptr), move || unsafe {
            Sendable(libobs::obs_data_create_from_json(cstr_ptr))
        })?;

        if result.0.is_null() {
            return Err(ObsError::JsonParseError);
        }

        Ok(ObsData {
            obs_data: result.clone(),
            runtime: runtime.clone(),
            _drop_guard: Arc::new(_ObsDataDropGuard {
                obs_data: result,
                runtime,
            }),
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

    fn as_ptr(&self) -> Sendable<*mut obs_data> {
        self.obs_data.clone()
    }
}

impl ObsDataGetters for ObsData {}
impl ObsDataSetters for ObsData {}

impl_obs_drop!(_ObsDataDropGuard, (obs_data), move || unsafe {
    libobs::obs_data_release(obs_data)
});

impl Clone for ObsData {
    fn clone(&self) -> Self {
        let json = self.get_json().unwrap();
        Self::from_json(json.as_str(), self.runtime.clone()).unwrap()
    }
}
