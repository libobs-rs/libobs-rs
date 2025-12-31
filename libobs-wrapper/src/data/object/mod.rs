use std::fmt::Debug;

use crate::{
    data::{ImmutableObsData, ObsData, ObsObjectUpdater},
    runtime::ObsRuntime,
    utils::{ObsError, ObsString},
};

mod macros;
pub(crate) use macros::*;

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

#[doc(hidden)]
pub trait ObsObjectTraitSealed: Debug + Send + Sync {
    /// Replaces the settings data of the object. This should only be called if the actual OBS object has been updated.
    ///
    /// DO NOT USE THIS METHOD UNLESS YOU KNOW WHAT YOU ARE DOING.
    fn __internal_replace_settings(&self, settings: ImmutableObsData) -> Result<(), ObsError>;
    /// Replaces the hotkey data of the object. This should only be called if the actual OBS object has been updated.
    ///
    /// DO NOT USE THIS METHOD UNLESS YOU KNOW WHAT YOU ARE DOING.
    fn __internal_replace_hotkey_data(&self, hotkey_data: ImmutableObsData)
        -> Result<(), ObsError>;
}

#[allow(private_bounds)]
pub trait ObsObjectTrait: ObsObjectClone + ObsObjectTraitSealed {
    fn runtime(&self) -> &ObsRuntime;
    fn settings(&self) -> Result<ImmutableObsData, ObsError>;
    fn hotkey_data(&self) -> Result<ImmutableObsData, ObsError>;

    fn id(&self) -> ObsString;
    fn name(&self) -> ObsString;

    /// Updates the settings of this output. Fails if active.
    fn update_settings(&self, settings: ObsData) -> Result<(), ObsError>;

    /// Updates the object with the current settings.
    /// For examples please take a look at the [Github repository](https://github.com/libobs-rs/libobs-rs/blob/main/examples).
    fn create_updater<'a, T: ObsObjectUpdater<'a, ToUpdate = Self> + Send + Sync>(
        &'a mut self,
    ) -> Result<T, ObsError>
    where
        Self: Sized + Send + Sync,
    {
        let runtime = self.runtime().clone();
        T::create_update(runtime, self)
    }
}
