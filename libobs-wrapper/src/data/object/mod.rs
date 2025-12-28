use std::fmt::Debug;

use crate::{
    data::immutable::ImmutableObsData,
    runtime::ObsRuntime,
    utils::{ObsError, ObsString},
};

/// Helper trait to enable cloning boxed outputs.
pub trait ObsObjectClone {
    fn clone_box(&self) -> Box<dyn ObsObjectTrait>;
}

impl<T> ObsObjectClone for T
where
    T: ObsObjectTrait + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn ObsObjectTrait> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn ObsObjectTrait> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub trait ObsObjectTraitSealed: Debug + Send + Sync {
    fn replace_settings(&self, settings: ImmutableObsData) -> Result<(), ObsError>;
    fn replace_hotkey_data(&self, hotkey_data: ImmutableObsData) -> Result<(), ObsError>;
}

#[allow(private_bounds)]
pub trait ObsObjectTrait: ObsObjectClone + ObsObjectTraitSealed {
    fn runtime(&self) -> &ObsRuntime;
    fn settings(&self) -> &ImmutableObsData;
    fn hotkey_data(&self) -> &ImmutableObsData;

    fn id(&self) -> ObsString;
    fn name(&self) -> ObsString;
}
