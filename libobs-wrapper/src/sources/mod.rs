mod builder;
pub use builder::*;

mod traits;
pub use traits::*;

mod macros;

mod filter;
pub use filter::*;

use libobs::obs_source_t;

use crate::{
    data::{
        object::{inner_fn_update_settings, ObsObjectTrait, ObsObjectTraitPrivate},
        ImmutableObsData, ObsDataPointers,
    },
    impl_obs_drop, impl_signal_manager, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::{Sendable, SmartPointerSendable},
    utils::{ObsDropGuard, ObsError, ObsString, SourceInfo},
};

use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ObsSourceRef {
    /// Disconnect signals first
    signal_manager: Arc<ObsSourceSignals>,

    id: ObsString,
    name: ObsString,
    settings: Arc<RwLock<ImmutableObsData>>,
    hotkey_data: Arc<RwLock<ImmutableObsData>>,

    attached_filters: Arc<RwLock<Vec<ObsFilterGuardPair>>>,

    runtime: ObsRuntime,
    source: SmartPointerSendable<*mut obs_source_t>,
}

impl ObsSourceRef {
    pub fn new_from_info(info: SourceInfo, runtime: ObsRuntime) -> Result<Self, ObsError> {
        let SourceInfo {
            id,
            name,
            settings,
            hotkey_data,
        } = info;

        Self::new(id, name, settings, hotkey_data, runtime)
    }

    pub fn new<T: Into<ObsString> + Sync + Send, K: Into<ObsString> + Sync + Send>(
        id: T,
        name: K,
        settings: Option<ImmutableObsData>,
        hotkey_data: Option<ImmutableObsData>,
        runtime: ObsRuntime,
    ) -> Result<Self, ObsError> {
        let id = id.into();
        let name = name.into();

        // We are creating empty immutable settings here because OBS would do it nonetheless if we passed a null pointer.
        let hotkey_data = match hotkey_data {
            Some(x) => x,
            None => ImmutableObsData::new(&runtime)?,
        };

        let hotkey_data_ptr = hotkey_data.as_ptr();
        let settings_ptr = settings.map(|x| x.as_ptr());

        let source_ptr = run_with_obs!(
            runtime,
            (hotkey_data_ptr, settings_ptr, id, name),
            move || {
                let id_ptr = id.as_ptr().0;
                let name_ptr = name.as_ptr().0;

                let settings_raw_ptr = match settings_ptr {
                    Some(s) => s.get_ptr(),
                    None => std::ptr::null_mut(),
                };

                let source_ptr = unsafe {
                    // Safety: Id, Name must be valid pointers because they are not dropped. Also the settings_ptr and hotkey_data_ptr may be null, its fine.
                    libobs::obs_source_create(
                        id_ptr,
                        name_ptr,
                        settings_raw_ptr,
                        hotkey_data_ptr.get_ptr(),
                    )
                };

                if source_ptr.is_null() {
                    Err(ObsError::NullPointer(None))
                } else {
                    Ok(Sendable(source_ptr))
                }
            }
        )??;

        let source_ptr = SmartPointerSendable::new(
            source_ptr.0,
            Arc::new(_ObsSourceGuard {
                source: source_ptr.clone(),
                runtime: runtime.clone(),
            }),
        );

        // Getting default settings if none were provided
        let settings = {
            let default_settings_ptr = run_with_obs!(runtime, (source_ptr), move || {
                unsafe {
                    // Safety: This safe to call because we are using a smart pointer and the source pointer must not be dropped.
                    Sendable(libobs::obs_source_get_settings(source_ptr.get_ptr()))
                }
            })?;

            ImmutableObsData::from_raw_pointer(default_settings_ptr, runtime.clone())
        };

        let signals = ObsSourceSignals::new(&source_ptr, runtime.clone())?;
        Ok(Self {
            source: source_ptr.clone(),
            id,
            name,
            settings: Arc::new(RwLock::new(settings)),
            hotkey_data: Arc::new(RwLock::new(hotkey_data)),
            attached_filters: Arc::new(RwLock::new(Vec::new())),
            runtime,
            signal_manager: Arc::new(signals),
        })
    }
}

impl ObsObjectTraitPrivate for ObsSourceRef {
    fn __internal_replace_settings(&self, settings: ImmutableObsData) -> Result<(), ObsError> {
        let mut guard = self
            .settings
            .write()
            .map_err(|_| ObsError::LockError("Failed to acquire write lock on settings".into()))?;

        *guard = settings;
        Ok(())
    }

    fn __internal_replace_hotkey_data(
        &self,
        hotkey_data: ImmutableObsData,
    ) -> Result<(), ObsError> {
        let mut guard = self.hotkey_data.write().map_err(|_| {
            ObsError::LockError("Failed to acquire write lock on hotkey data".into())
        })?;

        *guard = hotkey_data;
        Ok(())
    }
}

impl ObsObjectTrait<*mut libobs::obs_source_t> for ObsSourceRef {
    fn runtime(&self) -> &ObsRuntime {
        &self.runtime
    }

    fn settings(&self) -> Result<ImmutableObsData, ObsError> {
        let res = self
            .settings
            .read()
            .map_err(|_| ObsError::LockError("Failed to acquire read lock on settings".into()))?
            .clone();

        Ok(res)
    }

    fn hotkey_data(&self) -> Result<ImmutableObsData, ObsError> {
        let res = self
            .hotkey_data
            .read()
            .map_err(|_| ObsError::LockError("Failed to acquire read lock on hotkey data".into()))?
            .clone();

        Ok(res)
    }

    fn id(&self) -> ObsString {
        self.id.clone()
    }

    fn name(&self) -> ObsString {
        self.name.clone()
    }

    fn update_settings(&self, settings: crate::data::ObsData) -> Result<(), ObsError> {
        inner_fn_update_settings!(self, libobs::obs_source_update, settings)
    }

    fn as_ptr(&self) -> SmartPointerSendable<*mut libobs::obs_source_t> {
        self.source.clone()
    }
}

impl ObsSourceTrait for ObsSourceRef {
    fn signals(&self) -> &Arc<ObsSourceSignals> {
        &self.signal_manager
    }

    fn get_active_filters(&self) -> Result<Vec<ObsFilterGuardPair>, ObsError> {
        let guard = self.attached_filters.read().map_err(|_| {
            ObsError::LockError("Failed to acquire read lock on attached filters".into())
        })?;

        Ok(guard.clone())
    }

    fn apply_filter(&self, filter: &ObsFilterRef) -> Result<(), ObsError> {
        let mut guard = self.attached_filters.write().map_err(|_| {
            ObsError::LockError("Failed to acquire write lock on attached filters".into())
        })?;

        let source_ptr = self.as_ptr();
        let filter_ptr = filter.as_ptr();

        let has_filter = guard
            .iter()
            .any(|f| f.get_inner().as_ptr().get_ptr() == filter.as_ptr().get_ptr());

        if has_filter {
            return Err(ObsError::FilterAlreadyApplied);
        }

        run_with_obs!(self.runtime(), (source_ptr, filter_ptr), move || unsafe {
            // Safety: Both pointers are valid because of the smart pointers.
            libobs::obs_source_filter_add(source_ptr.get_ptr(), filter_ptr.get_ptr());
            Ok(())
        })??;

        let runtime = self.runtime().clone();
        let drop_guard = _ObsRemoveFilterOnDrop::new(self.as_ptr(), filter.as_ptr(), None, runtime);

        guard.push(ObsFilterGuardPair::new(
            filter.clone(),
            Arc::new(drop_guard),
        ));

        Ok(())
    }
}

impl_signal_manager!(|ptr: SmartPointerSendable<*mut libobs::obs_source_t>| unsafe {
    // Safety: We are using a smart pointer, so it is fine
    libobs::obs_source_get_signal_handler(ptr.get_ptr())
}, ObsSourceSignals for *mut libobs::obs_source_t, [
    "destroy": {},
    "remove": {},
    "update": {},
    "save": {},
    "load": {},
    "activate": {},
    "deactivate": {},
    "show": {},
    "hide": {},
    "mute": { struct MuteSignal {
        muted: bool
    } },
    "push_to_mute_changed": {struct PushToMuteChangedSignal {
        enabled: bool
    }},
    "push_to_mute_delay": {struct PushToMuteDelaySignal {
        delay: i64
    }},
    "push_to_talk_changed": {struct PushToTalkChangedSignal {
        enabled: bool
    }},
    "push_to_talk_delay": {struct PushToTalkDelaySignal {
        delay: i64
    }},
    "enable": {struct EnableSignal {
        enabled: bool
    }},
    "rename": {struct NewNameSignal {
        new_name: String,
        prev_name: String
    }},
    "update_properties": {},
    "update_flags": {struct UpdateFlagsSignal {
        flags: i64
    }},
    "audio_sync": {struct AudioSyncSignal {
        offset: i64,
    }},
    "audio_balance": {struct AudioBalanceSignal {
        balance: f64,
    }},
    "audio_mixers": {struct AudioMixersSignal {
        mixers: i64,
    }},
    "audio_activate": {},
    "audio_deactivate": {},
    "filter_add": {struct FilterAddSignal {
        POINTERS {
            filter: *mut libobs::obs_source_t,
        }
    }},
    "filter_remove": {struct FilterRemoveSignal {
        POINTERS {
            filter: *mut libobs::obs_source_t,
        }
    }},
    "reorder_filters": {},
    "transition_start": {},
    "transition_video_stop": {},
    "transition_stop": {},
    "media_started": {},
    "media_ended":{},
    "media_pause": {},
    "media_play": {},
    "media_restart": {},
    "media_stopped": {},
    "media_next": {},
    "media_previous": {},
]);

#[derive(Debug)]
struct _ObsSourceGuard {
    source: Sendable<*mut obs_source_t>,
    runtime: ObsRuntime,
}

impl ObsDropGuard for _ObsSourceGuard {}

impl_obs_drop!(_ObsSourceGuard, (source), move || unsafe {
    // Safety: We are in the runtime and the pointer is valid because of the drop guard
    libobs::obs_source_release(source.0);
});
