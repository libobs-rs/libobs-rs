mod builder;
pub use builder::*;

mod traits;
pub use traits::*;

mod macros;

use libobs::{obs_scene_item, obs_scene_t, obs_source_t};

use crate::{
    data::{
        object::{inner_fn_update_settings, ObsObjectTrait, ObsObjectTraitSealed},
        ImmutableObsData, ObsDataPointers,
    },
    impl_obs_drop, impl_signal_manager,
    macros::impl_eq_of_ptr,
    run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::{Sendable, SendableComp},
    utils::{ObsError, ObsString},
};

use std::{
    collections::HashMap,
    hash::Hash,
    ptr,
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ObsSourceRef {
    /// Disconnect signals first
    pub(crate) signal_manager: Arc<ObsSourceSignals>,

    pub(crate) source: Sendable<*mut obs_source_t>,
    pub(crate) id: ObsString,
    pub(crate) name: ObsString,
    pub(crate) settings: Arc<RwLock<ImmutableObsData>>,
    pub(crate) hotkey_data: Arc<RwLock<ImmutableObsData>>,

    /// This is a map to all attached scene items of this source.
    /// If the corresponding scene gets dropped, the scene will remove itself from the map and drop the scene item as well.
    pub(crate) scene_items:
        Arc<RwLock<HashMap<SendableComp<*mut obs_scene_t>, Sendable<*mut obs_scene_item>>>>,
    _guard: Arc<_ObsSourceGuard>,
    pub(crate) runtime: ObsRuntime,
}

impl_eq_of_ptr!(ObsSourceRef, source);
impl ObsSourceRef {
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
        let settings_ptr = settings
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(Sendable(ptr::null_mut()));

        let id_ptr = id.as_ptr();
        let name_ptr = name.as_ptr();

        let source_ptr = run_with_obs!(
            runtime,
            (hotkey_data_ptr, settings_ptr, id_ptr, name_ptr),
            move || unsafe {
                Sendable(libobs::obs_source_create(
                    id_ptr,
                    name_ptr,
                    settings_ptr,
                    hotkey_data_ptr,
                ))
            }
        )?;

        if source_ptr.0.is_null() {
            return Err(ObsError::NullPointer);
        }

        // Getting default settings if none were provided
        let settings = {
            let default_settings_ptr = run_with_obs!(runtime, (source_ptr), move || unsafe {
                Sendable(libobs::obs_source_get_settings(source_ptr))
            })?;

            ImmutableObsData::from_raw(default_settings_ptr, runtime.clone())
        };

        let signals = ObsSourceSignals::new(&source_ptr, runtime.clone())?;
        Ok(Self {
            source: source_ptr.clone(),
            id,
            name,
            settings: Arc::new(RwLock::new(settings)),
            hotkey_data: Arc::new(RwLock::new(hotkey_data)),
            _guard: Arc::new(_ObsSourceGuard {
                source: source_ptr,
                runtime: runtime.clone(),
            }),
            scene_items: Arc::new(RwLock::new(HashMap::new())),
            runtime,
            signal_manager: Arc::new(signals),
        })
    }
}

impl ObsObjectTraitSealed for ObsSourceRef {
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

impl ObsObjectTrait for ObsSourceRef {
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
}

impl ObsSourceTraitSealed for ObsSourceRef {
    fn add_scene_item_ptr(
        &self,
        scene_ptr: SendableComp<*mut libobs::obs_scene_t>,
        item_ptr: Sendable<*mut libobs::obs_scene_item>,
    ) -> Result<(), ObsError> {
        self.scene_items
            .write()
            .map_err(|_| {
                ObsError::LockError("Failed to acquire write lock on scene items map".into())
            })
            .map(|mut guard| {
                guard.insert(scene_ptr, item_ptr);
            })
    }

    fn remove_scene_item_ptr(
        &self,
        scene_ptr: SendableComp<*mut libobs::obs_scene_t>,
    ) -> Result<(), ObsError> {
        self.scene_items
            .write()
            .map_err(|_| {
                ObsError::LockError("Failed to acquire write lock on scene items map".into())
            })
            .map(|mut guard| {
                guard.remove(&scene_ptr);
            })
    }

    fn get_scene_item_ptr(
        &self,
        scene_ptr: &SendableComp<*mut libobs::obs_scene_t>,
    ) -> Result<Option<Sendable<*mut libobs::obs_scene_item>>, ObsError> {
        let guard = self.scene_items.read().map_err(|_| {
            ObsError::LockError("Failed to acquire read lock on scene items map".into())
        })?;

        Ok(guard.get(scene_ptr).cloned())
    }
}

impl ObsSourceTrait for ObsSourceRef {
    fn as_ptr(&self) -> Sendable<*mut libobs::obs_source_t> {
        self.source.clone()
    }

    fn signals(&self) -> &Arc<ObsSourceSignals> {
        &self.signal_manager
    }
}

impl_signal_manager!(|ptr| unsafe { libobs::obs_source_get_signal_handler(ptr) }, ObsSourceSignals for ObsSourceRef<*mut libobs::obs_source_t>, [
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

impl_obs_drop!(_ObsSourceGuard, (source), move || unsafe {
    libobs::obs_source_release(source);
});

pub type ObsFilterRef = ObsSourceRef;
