use std::fmt::Debug;

use crate::{
    data::{ImmutableObsData, ObsData, ObsObjectUpdater},
    macros::trait_with_optional_send_sync,
    runtime::ObsRuntime,
    unsafe_send::SmartPointerSendable,
    utils::{ObsError, ObsString},
};

mod macros;
pub(crate) use macros::*;

/// Helper trait to enable cloning boxed outputs.
pub trait ObsObjectClone<K: Clone> {
    fn clone_box(&self) -> Box<dyn ObsObjectTrait<K>>;
}

impl<T, K: Clone> ObsObjectClone<K> for T
where
    T: ObsObjectTrait<K> + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn ObsObjectTrait<K>> {
        Box::new(self.clone())
    }
}

impl<K: Clone> Clone for Box<dyn ObsObjectTrait<K>> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

trait_with_optional_send_sync! {
    #[doc(hidden)]
    pub trait ObsObjectTraitPrivate: Debug {
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
}

#[allow(private_bounds)]
/// Trait representing an OBS object.
///
/// Hashing for this trait is automatically done by comparing the underlying raw pointer addresses.
pub trait ObsObjectTrait<K: Clone>: ObsObjectClone<K> + ObsObjectTraitPrivate {
    fn runtime(&self) -> &ObsRuntime;
    fn settings(&self) -> Result<ImmutableObsData, ObsError>;
    fn hotkey_data(&self) -> Result<ImmutableObsData, ObsError>;

    fn id(&self) -> ObsString;
    fn name(&self) -> ObsString;

    /// Updates the settings of this output. Fails if active.
    fn update_settings(&self, settings: ObsData) -> Result<(), ObsError>;

    /// Updates the object with the current settings.
    /// For examples please take a look at the [Github repository](https://github.com/libobs-rs/libobs-rs/blob/main/examples).
    fn create_updater<'a, T: ObsObjectUpdater<'a, K, ToUpdate = Self> + Send + Sync>(
        &'a mut self,
    ) -> Result<T, ObsError>
    where
        Self: Sized + Send + Sync,
    {
        let runtime = self.runtime().clone();
        T::create_update(runtime, self)
    }

    /// Creates a new reference to the drop guard.
    /// This is useful if you are using the underlying raw pointer, make sure to store it along the drop guard
    fn as_ptr(&self) -> SmartPointerSendable<K>;
}
