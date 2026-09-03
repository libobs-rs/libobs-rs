use std::fmt::Debug;

use crate::{
    data::{ImmutableObsData, ObsData, ObsObjectUpdater},
    macros::trait_with_optional_send_sync,
    runtime::ObsRuntime,
    unsafe_send::{NativePointer, SmartPointerSendable},
    utils::{ObsError, ObsString},
};

mod macros;
pub(crate) use macros::*;

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
/// A OBs object has an id, a name, `settings` and `hotkey_data`.
pub trait ObsObjectTrait: ObsObjectTraitPrivate {
    #[doc(hidden)]
    type Native: NativePointer;

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

    /// Stable opaque identity for this native object. It is scoped to the owning runtime.
    fn object_id(&self) -> crate::unsafe_send::NativeObjectId {
        self.__native_handle().native_id()
    }

    #[doc(hidden)]
    fn __native_handle(&self) -> SmartPointerSendable<Self::Native>;
}
