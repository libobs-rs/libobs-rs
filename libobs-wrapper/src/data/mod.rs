//! This module contains every struct related to storing data in OBS.
//! There are two important structs:
//! 1. `ObsData`
//! - This struct holds a mutable reference to a ObsData, so you can set a string, ints and similar
//! - You can convert this ObsData object to a immutable reference
//! - Cloning this ObsData struct is very memory intensive, as the ObsData will completely clone every member of this data.
//! 2. `ImmutableObsData`
//! - This structs holds, as the name might suggest, an immutable reference to ObsData.
//! - The data inside this struct can not be changed and is intended for read-only.
//! - You can turn this ImmutableObsData into a writable `ObsData` struct again, but this will internally clone the data and not affect the `ImmutableObsData` itself.
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
/// `ObsData` is intentionally not `Clone`: duplicating it requires a native round-trip
/// and can fail. Use [`ObsData::try_clone`] when an independent mutable copy is needed.
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
        let ptr =
            SmartPointerSendable::new(obs_data.0, drop_guard.clone(), runtime.native_registry());
        Ok(ObsData {
            ptr,
            runtime: runtime.clone(),
        })
    }

    /// Wraps an owned native data pointer returned by libobs.
    ///
    /// The caller must only pass pointers for which this wrapper owns one reference.
    /// Keeping this constructor crate-private makes that ownership contract an internal
    /// FFI invariant rather than something downstream callers need to reason about.
    pub(crate) fn from_raw_pointer(
        data: Sendable<*mut libobs::obs_data_t>,
        runtime: ObsRuntime,
    ) -> Self {
        let drop_guard = Arc::new(_ObsDataDropGuard {
            data_ptr: data.clone(),
            runtime: runtime.clone(),
        });
        Self {
            ptr: SmartPointerSendable::new(data.0, drop_guard, runtime.native_registry()),
            runtime,
        }
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

        let ptr =
            SmartPointerSendable::new(raw_ptr.0, drop_guard.clone(), runtime.native_registry());

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

    /// Creates an independent mutable copy of this OBS data object.
    ///
    /// This is fallible by design; the previous `Clone` implementation performed the
    /// same JSON round-trip but panicked when either native operation failed.
    pub fn try_clone(&self) -> Result<Self, ObsError> {
        let json = self.get_json()?;
        Self::from_json(&json, self.runtime.clone())
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
