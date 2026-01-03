use crate::{
    data::object::ObsObjectTrait,
    macros::impl_eq_of_ptr,
    sources::{ObsFilterRef, ObsSourceSignals, _ObsRemoveFilterOnDrop},
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
pub trait ObsSourceTrait: ObsObjectTrait<*mut libobs::obs_source_t> {
    fn signals(&self) -> &Arc<ObsSourceSignals>;

    fn get_active_filters(&self) -> Result<Vec<ObsFilterGuardPair>, ObsError>;
    fn apply_filter(&self, filter: &ObsFilterRef) -> Result<(), ObsError>;
}

impl_eq_of_ptr!(dyn ObsSourceTrait);
