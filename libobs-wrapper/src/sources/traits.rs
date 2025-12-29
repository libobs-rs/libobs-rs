use std::{fmt::Debug, hash::Hash, sync::Arc};

use crate::{
    data::object::ObsObjectTrait,
    sources::ObsSourceSignals,
    unsafe_send::{Sendable, SendableComp},
    utils::ObsError,
};

#[doc(hidden)]
pub trait ObsSourceTraitSealed: Debug + Send + Sync {
    fn add_scene_item_ptr(
        &self,
        scene_ptr: SendableComp<*mut libobs::obs_scene_t>,
        item_ptr: Sendable<*mut libobs::obs_scene_item>,
    ) -> Result<(), ObsError>;

    fn remove_scene_item_ptr(
        &self,
        scene_ptr: SendableComp<*mut libobs::obs_scene_t>,
    ) -> Result<(), ObsError>;

    fn get_scene_item_ptr(
        &self,
        scene_ptr: &SendableComp<*mut libobs::obs_scene_t>,
    ) -> Result<Option<Sendable<*mut libobs::obs_scene_item>>, ObsError>;
}

impl PartialEq for dyn ObsSourceTrait {
    fn eq(&self, other: &Self) -> bool {
        self.as_ptr().0 == other.as_ptr().0
    }
}

impl Eq for dyn ObsSourceTrait {}

impl Hash for dyn ObsSourceTrait {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_ptr().0.hash(state);
    }
}

#[allow(private_bounds)]
pub trait ObsSourceTrait: ObsObjectTrait + ObsSourceTraitSealed {
    fn as_ptr(&self) -> Sendable<*mut libobs::obs_source_t>;
    fn signals(&self) -> &Arc<ObsSourceSignals>;
}
