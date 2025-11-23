mod builder;
pub use builder::*;

use libobs::{obs_scene_item, obs_source_t};

use crate::{
    data::{immutable::ImmutableObsData, ObsData},
    impl_obs_drop, impl_signal_manager, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::Sendable,
    utils::{traits::ObsUpdatable, ObsError, ObsString},
};

use std::sync::Arc;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ObsSourceRef {
    /// Disconnect signals first
    pub(crate) signal_manager: Arc<ObsSourceSignals>,

    pub(crate) source: Sendable<*mut obs_source_t>,
    pub(crate) id: ObsString,
    pub(crate) name: ObsString,
    pub(crate) settings: Arc<ImmutableObsData>,
    pub(crate) hotkey_data: Arc<ImmutableObsData>,
    pub(crate) scene_item: Option<Sendable<*mut obs_scene_item>>,

    _guard: Arc<_ObsSourceGuard>,
    pub(crate) runtime: ObsRuntime,
}

impl ObsSourceRef {
    pub fn new<T: Into<ObsString> + Sync + Send, K: Into<ObsString> + Sync + Send>(
        id: T,
        name: K,
        mut settings: Option<ObsData>,
        mut hotkey_data: Option<ObsData>,
        runtime: ObsRuntime,
    ) -> Result<Self, ObsError> {
        let id = id.into();
        let name = name.into();

        let settings = match settings.take() {
            Some(x) => ImmutableObsData::from(x),
            None => ImmutableObsData::new(&runtime)?,
        };

        let hotkey_data = match hotkey_data.take() {
            Some(x) => ImmutableObsData::from(x),
            None => ImmutableObsData::new(&runtime)?,
        };

        let hotkey_data_ptr = hotkey_data.as_ptr();
        let settings_ptr = settings.as_ptr();
        let id_ptr = id.as_ptr();
        let name_ptr = name.as_ptr();

        let source = run_with_obs!(
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

        if source.0.is_null() {
            return Err(ObsError::NullPointer);
        }

        let signals = ObsSourceSignals::new(&source, runtime.clone())?;
        Ok(Self {
            source: source.clone(),
            id,
            name,
            settings: Arc::new(settings),
            hotkey_data: Arc::new(hotkey_data),
            _guard: Arc::new(_ObsSourceGuard {
                source,
                runtime: runtime.clone(),
            }),
            scene_item: None,
            runtime,
            signal_manager: Arc::new(signals),
        })
    }

    pub fn settings(&self) -> &ImmutableObsData {
        &self.settings
    }

    pub fn hotkey_data(&self) -> &ImmutableObsData {
        &self.hotkey_data
    }

    pub fn name(&self) -> String {
        self.name.to_string()
    }

    pub fn id(&self) -> String {
        self.id.to_string()
    }

    pub fn signal_manager(&self) -> Arc<ObsSourceSignals> {
        self.signal_manager.clone()
    }

    pub fn as_ptr(&self) -> *mut obs_source_t {
        self.source.0
    }
}

impl ObsUpdatable for ObsSourceRef {
    fn update_raw(&mut self, data: ObsData) -> Result<(), ObsError> {
        let data_ptr = data.as_ptr();
        let source_ptr = self.source.clone();
        log::trace!("Updating source: {:?}", self.source);
        run_with_obs!(self.runtime, (source_ptr, data_ptr), move || unsafe {
            libobs::obs_source_update(source_ptr, data_ptr);
        })
    }

    fn reset_and_update_raw(&mut self, data: ObsData) -> Result<(), ObsError> {
        let source_ptr = self.source.clone();
        run_with_obs!(self.runtime, (source_ptr), move || unsafe {
            libobs::obs_source_reset_settings(source_ptr, data.as_ptr().0);
        })
    }

    fn runtime(&self) -> ObsRuntime {
        self.runtime.clone()
    }

    fn get_settings(&self) -> Result<ImmutableObsData, ObsError> {
        log::trace!("Getting settings for source: {:?}", self.source);
        let source_ptr = self.source.clone();
        let res = run_with_obs!(self.runtime, (source_ptr), move || unsafe {
            Sendable(libobs::obs_source_get_settings(source_ptr))
        })?;

        log::trace!("Got settings: {:?}", res);
        Ok(ImmutableObsData::from_raw(res, self.runtime.clone()))
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
    /// This is just for sources that are of the `game-capture`, `window-capture` or `win-wasapi` type. Other sources will never emit this signal.
    //TODO Add support for the `linux-capture` type as it does not contain the `title` field (its 'name' instead)
    "hooked": {struct HookedSignal {
        title: String,
        class: String,
        executable: String;
        POINTERS {
            source: *mut libobs::obs_source_t,
        }
    }},
    /// This is just for sources that are of the `game-capture`, `window-capture` or `win-wasapi` type. Other sources will never emit this signal.
    //TODO Add support for the `linux-capture` type as it does not contain the `title` field (its 'name' instead)
    "unhooked": {struct UnhookedSignal {
        POINTERS {
            source: *mut libobs::obs_source_t,
        }
    }},
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
