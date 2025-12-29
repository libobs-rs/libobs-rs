use std::sync::Arc;

use libobs::obs_data;

use crate::{
    run_with_obs,
    unsafe_send::Sendable,
    utils::{ObsError, ObsString},
};

use super::_ObsDataDropGuard;

#[derive(Debug)]
pub enum ObsDataChange {
    String(ObsString, ObsString),
    Int(ObsString, i64),
    Bool(ObsString, bool),
    Double(ObsString, f64),
}

#[derive(Debug)]
/// Important: Make sure to call `update()` after setting the values.
/// This will apply the changes to the `ObsData` object.
#[must_use = "The `update()` method must be called to apply changes."]
pub struct ObsDataUpdater {
    pub(crate) changes: Vec<ObsDataChange>,
    pub(crate) obs_data: Sendable<*mut obs_data>,
    pub(crate) _drop_guard: Arc<_ObsDataDropGuard>,
}

impl ObsDataUpdater {
    pub fn set_string_ref(&mut self, key: impl Into<ObsString>, value: impl Into<ObsString>) {
        let key = key.into();
        let value = value.into();

        log::trace!("Setting string: {:?} = {:?}", key, value);
        self.changes.push(ObsDataChange::String(key, value));
    }

    pub fn set_string(mut self, key: impl Into<ObsString>, value: impl Into<ObsString>) -> Self {
        self.set_string_ref(key, value);
        self
    }

    pub fn set_int_ref(&mut self, key: impl Into<ObsString>, value: i64) {
        let key = key.into();
        self.changes.push(ObsDataChange::Int(key, value));
    }

    pub fn set_int(mut self, key: impl Into<ObsString>, value: i64) -> Self {
        self.set_int_ref(key, value);
        self
    }

    pub fn set_bool_ref(&mut self, key: impl Into<ObsString>, value: bool) {
        let key = key.into();
        self.changes.push(ObsDataChange::Bool(key, value));
    }

    pub fn set_bool(mut self, key: impl Into<ObsString>, value: bool) -> Self {
        self.set_bool_ref(key, value);
        self
    }

    pub fn apply(self) -> Result<(), ObsError> {
        let ObsDataUpdater {
            changes,
            obs_data,
            _drop_guard,
        } = self;

        let obs_data = obs_data.clone();
        run_with_obs!(_drop_guard.runtime, (obs_data), move || unsafe {
            for change in changes {
                match change {
                    ObsDataChange::String(key, value) => {
                        libobs::obs_data_set_string(obs_data, key.as_ptr().0, value.as_ptr().0)
                    }
                    ObsDataChange::Int(key, value) => {
                        libobs::obs_data_set_int(obs_data, key.as_ptr().0, value)
                    }
                    ObsDataChange::Bool(key, value) => {
                        libobs::obs_data_set_bool(obs_data, key.as_ptr().0, value)
                    }
                    ObsDataChange::Double(key, value) => {
                        libobs::obs_data_set_double(obs_data, key.as_ptr().0, value)
                    }
                };
            }
        })
    }

    #[deprecated = "Use `apply()` instead."]
    pub fn update(self) -> Result<(), ObsError> {
        self.apply()
    }
}
