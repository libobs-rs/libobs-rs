use crate::{
    data::object::ObsObjectTrait,
    macros::impl_eq_of_obs_object,
    sources::{_ObsRemoveFilterOnDrop, ObsFilterRef, ObsSourceSignals},
    utils::ObsError,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ObsFilterGuardPair {
    filter: ObsFilterRef,
    pub(crate) _guard: Arc<_ObsRemoveFilterOnDrop>,
}

impl ObsFilterGuardPair {
    pub(crate) fn new(filter: ObsFilterRef, guard: Arc<_ObsRemoveFilterOnDrop>) -> Self {
        Self {
            filter,
            _guard: guard,
        }
    }

    pub fn get_inner(&self) -> &ObsFilterRef {
        &self.filter
    }

    pub fn get_inner_mut(&mut self) -> &mut ObsFilterRef {
        &mut self.filter
    }
}

#[allow(private_bounds)]
pub trait ObsSourceTrait: ObsObjectTrait<Native = *mut libobs::obs_source_t> {
    fn signals(&self) -> &Arc<ObsSourceSignals>;

    fn get_active_filters(&self) -> Result<Vec<ObsFilterGuardPair>, ObsError>;
    fn apply_filter(&self, filter: &ObsFilterRef) -> Result<(), ObsError>;

    /// Returns libobs's persistent UUID for this source.
    ///
    /// Unlike [`ObsObjectTrait::object_id`], this identifier is intended for persisted
    /// application state. `object_id()` is runtime-scoped and must never be written to disk.
    fn persistent_uuid(&self) -> Result<String, ObsError> {
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        crate::run_with_obs!(runtime, (source), move || unsafe {
            // Safety: the managed source lease remains alive for the actor call and libobs owns
            // the returned NUL-terminated UUID string.
            let uuid = libobs::obs_source_get_uuid(source.get_ptr());
            if uuid.is_null() {
                return Err(ObsError::NullPointer(Some(
                    "obs_source_get_uuid returned null".into(),
                )));
            }
            Ok(std::ffi::CStr::from_ptr(uuid)
                .to_string_lossy()
                .into_owned())
        })?
    }
}

impl_eq_of_obs_object!(dyn ObsSourceTrait);
