use libobs::obs_output;
use std::collections::HashMap;
use std::ptr;
use std::sync::{Arc, RwLock};

use crate::data::object::{inner_fn_update_settings, ObsObjectTrait, ObsObjectTraitPrivate};
use crate::data::ImmutableObsData;
use crate::data::ObsDataPointers;
use crate::runtime::ObsRuntime;
use crate::unsafe_send::{Sendable, SmartPointerSendable};
use crate::utils::{ObsDropGuard, OutputInfo};
use crate::{impl_obs_drop, impl_signal_manager, run_with_obs};

use crate::{
    encoders::{audio::ObsAudioEncoder, video::ObsVideoEncoder},
    utils::{ObsError, ObsString},
};

use super::ObsData;

pub(crate) mod macros;
mod traits;
pub use traits::*;

mod replay_buffer;
pub use replay_buffer::*;

#[derive(Debug)]
struct _ObsOutputDropGuard {
    output: Sendable<*mut obs_output>,
    runtime: ObsRuntime,
}

impl ObsDropGuard for _ObsOutputDropGuard {}

impl_obs_drop!(_ObsOutputDropGuard, (output), move || unsafe {
    // Safety: We are in the runtime and drop guards are always constructed from valid output pointers. Also this guard should be in an Arc, so no double free should happen.
    libobs::obs_output_release(output.0);
});

#[derive(Debug, Clone)]
/// A reference to an OBS output.
///
/// This struct represents an output in OBS, which is responsible for
/// outputting encoded audio and video data to a destination such as:
/// - A file (recording)
/// - A streaming service (RTMP, etc.)
/// - A replay buffer
///
/// The output is associated with video and audio encoders that convert
/// raw media to the required format before sending/storing.
pub struct ObsOutputRef {
    /// Disconnect signals first
    pub(crate) signal_manager: Arc<ObsOutputSignals>,

    /// Settings for the output
    pub(crate) settings: Arc<RwLock<ImmutableObsData>>,

    /// Hotkey configuration data for the output
    pub(crate) hotkey_data: Arc<RwLock<ImmutableObsData>>,

    /// Video encoders attached to this output
    pub(crate) curr_video_encoder: Arc<RwLock<Option<Arc<ObsVideoEncoder>>>>,

    /// Audio encoders attached to this output
    pub(crate) audio_encoders: Arc<RwLock<HashMap<usize, Arc<ObsAudioEncoder>>>>,

    /// The type identifier of this output
    pub(crate) id: ObsString,

    /// The unique name of this output
    pub(crate) name: ObsString,

    pub(crate) runtime: ObsRuntime,

    /// Pointer to the underlying OBS output
    pub(crate) output: SmartPointerSendable<*mut obs_output>,
}

impl ObsOutputTraitSealed for ObsOutputRef {
    fn new(output: OutputInfo, runtime: ObsRuntime) -> Result<Self, ObsError> {
        let OutputInfo {
            id,
            name,
            settings,
            hotkey_data,
        } = output;

        let settings_ptr = settings.as_ref().map(|x| x.as_ptr());
        let hotkey_data_ptr = hotkey_data.as_ref().map(|x| x.as_ptr());

        let output = run_with_obs!(
            runtime,
            (id, name, settings_ptr, hotkey_data_ptr),
            move || {
                let settings_raw_ptr = match settings_ptr {
                    Some(s) => s.get_ptr(),
                    None => ptr::null_mut(),
                };

                let hotkey_data_raw_ptr = match hotkey_data_ptr {
                    Some(h) => h.get_ptr(),
                    None => ptr::null_mut(),
                };

                let id_ptr = id.as_ptr().0;
                let name_ptr = name.as_ptr().0;

                let output = unsafe {
                    // Safety: All pointers are valid because we are keeping them in this scope and because we are using smart pointers for ObsData
                    libobs::obs_output_create(
                        id_ptr,
                        name_ptr,
                        settings_raw_ptr,
                        hotkey_data_raw_ptr,
                    )
                };

                if output.is_null() {
                    return Err(ObsError::NullPointer(None));
                }

                Ok(Sendable(output))
            }
        )??;

        let output = SmartPointerSendable::new(
            output.0,
            Arc::new(_ObsOutputDropGuard {
                output: output.clone(),
                runtime: runtime.clone(),
            }),
        );

        // We are getting the settings from OBS because OBS will have updated it with default values.
        let new_settings_ptr = run_with_obs!(runtime, (output), move || {
            let new_settings_ptr = unsafe {
                // Safety: At this point, the output can't be released because we are using a SmartPointer.
                libobs::obs_output_get_settings(output.get_ptr())
            };

            if new_settings_ptr.is_null() {
                return Err(ObsError::NullPointer(None));
            }

            Ok(Sendable(new_settings_ptr))
        })??;

        let settings = ImmutableObsData::from_raw_pointer(new_settings_ptr, runtime.clone());

        // We are creating the hotkey data here because even it is null, OBS would create it nonetheless.
        // https://github.com/obsproject/obs-studio/blob/d97e5ad820abcccf826faf897df4c7f511857cd4/libobs/obs.c#L2629
        let hotkey_data = match hotkey_data {
            Some(h) => h,
            None => ImmutableObsData::new(&runtime)?,
        };

        let signal_manager = ObsOutputSignals::new(&output, runtime.clone())?;
        Ok(Self {
            settings: Arc::new(RwLock::new(settings)),
            hotkey_data: Arc::new(RwLock::new(hotkey_data)),

            curr_video_encoder: Arc::new(RwLock::new(None)),
            audio_encoders: Arc::new(RwLock::new(HashMap::new())),

            output: output.clone(),
            id,
            name,

            runtime,
            signal_manager: Arc::new(signal_manager),
        })
    }
}

impl ObsObjectTraitPrivate for ObsOutputRef {
    fn __internal_replace_settings(&self, settings: ImmutableObsData) -> Result<(), ObsError> {
        self.settings
            .write()
            .map_err(|_| ObsError::LockError("Failed to acquire write lock on settings".into()))
            .map(|mut settings_lock| {
                *settings_lock = settings;
            })
    }

    fn __internal_replace_hotkey_data(
        &self,
        hotkey_data: ImmutableObsData,
    ) -> Result<(), ObsError> {
        self.hotkey_data
            .write()
            .map_err(|_| ObsError::LockError("Failed to acquire write lock on hotkey data".into()))
            .map(|mut hotkey_lock| {
                *hotkey_lock = hotkey_data;
            })
    }
}

impl ObsObjectTrait<*mut libobs::obs_output> for ObsOutputRef {
    fn name(&self) -> ObsString {
        self.name.clone()
    }

    fn id(&self) -> ObsString {
        self.id.clone()
    }

    fn runtime(&self) -> &ObsRuntime {
        &self.runtime
    }

    fn settings(&self) -> Result<ImmutableObsData, ObsError> {
        let r = self
            .settings
            .read()
            .map_err(|_| ObsError::LockError("Failed to acquire read lock on settings".into()))?;

        Ok(r.clone())
    }

    fn hotkey_data(&self) -> Result<ImmutableObsData, ObsError> {
        let r = self.hotkey_data.read().map_err(|_| {
            ObsError::LockError("Failed to acquire read lock on hotkey data".into())
        })?;

        Ok(r.clone())
    }

    fn update_settings(&self, settings: ObsData) -> Result<(), ObsError> {
        if self.is_active()? {
            return Err(ObsError::OutputAlreadyActive);
        }

        inner_fn_update_settings!(self, libobs::obs_output_update, settings)
    }

    fn as_ptr(&self) -> SmartPointerSendable<*mut obs_output> {
        self.output.clone()
    }
}

impl ObsOutputTrait for ObsOutputRef {
    fn signals(&self) -> &Arc<ObsOutputSignals> {
        &self.signal_manager
    }

    fn video_encoder(&self) -> &Arc<RwLock<Option<Arc<ObsVideoEncoder>>>> {
        &self.curr_video_encoder
    }

    fn audio_encoders(&self) -> &Arc<RwLock<HashMap<usize, Arc<ObsAudioEncoder>>>> {
        &self.audio_encoders
    }
}

impl_signal_manager!(|ptr: SmartPointerSendable<*mut libobs::obs_output>| unsafe {
    // Safety: We are using a smart pointer, so it is fine
    libobs::obs_output_get_signal_handler(ptr.get_ptr())
}, ObsOutputSignals for ObsOutputRef<*mut libobs::obs_output>, [
    "start": {},
    "stop": {code: crate::enums::ObsOutputStopSignal},
    "pause": {},
    "unpause": {},
    "starting": {},
    "stopping": {},
    "activate": {},
    "deactivate": {},
    "reconnect": {},
    "reconnect_success": {}
]);
