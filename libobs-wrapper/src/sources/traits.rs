use std::{fmt::Debug, hash::Hash, sync::Arc};

use crate::{
    data::object::ObsObjectTrait, macros::impl_eq_of_ptr, sources::ObsSourceSignals, unsafe_send::{Sendable, SendableComp}, utils::ObsError
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

impl_eq_of_ptr!(dyn ObsSourceTrait);


#[allow(private_bounds)]
pub trait ObsSourceTrait: ObsObjectTrait<*mut libobs::obs_source_t> + ObsSourceTraitSealed {
    fn signals(&self) -> &Arc<ObsSourceSignals>;
}
