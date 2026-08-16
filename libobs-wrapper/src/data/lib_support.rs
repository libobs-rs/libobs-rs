//! Use the `libobs-source` crate to create sources like `window_capture` for obs

use crate::{
    data::{object::ObsObjectTrait, ObsData},
    runtime::ObsRuntime,
    unsafe_send::NativePointer,
    utils::{ObjectInfo, ObsError, ObsString},
};

use super::updater::ObsDataUpdater;

/// Should be implemented for any enum that can be represented as a String.
///
/// This is mostly used for `ObsSourceBuilders`.
pub trait StringEnum {
    fn to_str(&self) -> &str;
}

/// Trait for building OBS objects.
/// This can range from building audio encoders to building scenes, every ObsObject
/// has the same underlying properties:
/// - name
/// - id
/// - hotkey_data
/// - settings
pub trait ObsObjectBuilder {
    fn new<T: Into<ObsString> + Send + Sync>(
        name: T,
        runtime: ObsRuntime,
    ) -> Result<Self, ObsError>
    where
        Self: Sized;

    fn runtime(&self) -> &ObsRuntime;

    /// Returns the name of the source.
    fn get_name(&self) -> ObsString;

    fn object_build(self) -> Result<ObjectInfo, ObsError>
    where
        Self: Sized;

    fn get_settings(&self) -> &ObsData;
    fn get_settings_updater(&mut self) -> &mut ObsDataUpdater;

    fn get_hotkeys(&self) -> &ObsData;
    fn get_hotkeys_updater(&mut self) -> &mut ObsDataUpdater;

    /// Returns the ID of the source.
    fn get_id() -> ObsString;
}

/// A trait that is used to represent any struct than can update an OBS object.
/// This can be for example a ´WindowSourceUpdater´, which updates the settings of the `WindowSourceRef`, when
/// the `update` method is called.
pub trait ObsObjectUpdater<'a, K: NativePointer> {
    type ToUpdate: ObsObjectTrait<K>;
    fn create_update(
        runtime: ObsRuntime,
        updatable: &'a mut Self::ToUpdate,
    ) -> Result<Self, ObsError>
    where
        Self: Sized;

    fn get_settings(&self) -> &ObsData;
    fn get_settings_updater(&mut self) -> &mut ObsDataUpdater;

    fn update(self) -> Result<(), ObsError>;

    fn runtime(&self) -> &ObsRuntime;

    /// Returns the ID of the object
    fn get_id() -> ObsString;
}
