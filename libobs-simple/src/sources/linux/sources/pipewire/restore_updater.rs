use libobs_wrapper::{
    data::{object::ObsObjectTrait, ObsData, ObsDataUpdater, ObsObjectUpdater},
    runtime::ObsRuntime,
    utils::{ObsError, ObsString},
};

use crate::sources::linux::pipewire::ObsPipeWireSourceRef;

pub struct ObsPipeWireGeneralUpdater<'a> {
    settings: ObsData,
    settings_updater: ObsDataUpdater,
    updatable: &'a mut ObsPipeWireSourceRef,
}

impl<'a> ObsObjectUpdater<'a, *mut libobs::obs_source> for ObsPipeWireGeneralUpdater<'a> {
    type ToUpdate = ObsPipeWireSourceRef;

    fn get_id() -> libobs_wrapper::utils::ObsString {
        ObsString::new("pipewire-restore-token-updater")
    }

    fn create_update(
        runtime: ObsRuntime,
        updatable: &'a mut Self::ToUpdate,
    ) -> Result<Self, ObsError> {
        let mut settings = ObsData::new(runtime)?;

        Ok(Self {
            settings_updater: settings.bulk_update(),
            settings,
            updatable,
        })
    }

    fn get_settings(&self) -> &ObsData {
        &self.settings
    }

    fn get_settings_updater(&mut self) -> &mut ObsDataUpdater {
        &mut self.settings_updater
    }

    fn update(self) -> Result<(), ObsError> {
        let ObsPipeWireGeneralUpdater {
            settings_updater,
            updatable,
            settings,
        } = self;

        settings_updater.apply()?;

        updatable.update_settings(settings)
    }

    fn runtime(&self) -> &ObsRuntime {
        self.updatable.runtime()
    }
}

impl<'a> ObsPipeWireGeneralUpdater<'a> {
    pub fn set_show_cursor(mut self, show: bool) -> Self {
        self.settings_updater.set_bool_ref("ShowCursor", show);
        self
    }

    /// Enable cursor capture for screen recording
    pub fn with_cursor(self) -> Self {
        self.set_show_cursor(true)
    }
}
