use libobs::obs_output;
use std::collections::HashMap;
use std::ptr;
use std::sync::{Arc, RwLock};

use crate::data::immutable::ImmutableObsData;
use crate::data::object::{ObsObjectTrait, ObsObjectTraitSealed};
use crate::data::ObsDataPointers;
use crate::runtime::ObsRuntime;
use crate::unsafe_send::Sendable;
use crate::utils::OutputInfo;
use crate::{impl_obs_drop, impl_signal_manager};

use crate::{
    encoders::{audio::ObsAudioEncoder, video::ObsVideoEncoder},
    utils::{ObsError, ObsString},
};

use super::ObsData;

mod traits;
pub use traits::*;

mod replay_buffer;
pub use replay_buffer::*;

#[derive(Debug)]
struct _ObsOutputDropGuard {
    output: Sendable<*mut obs_output>,
    runtime: ObsRuntime,
}

impl_obs_drop!(_ObsOutputDropGuard, (output), move || unsafe {
    libobs::obs_output_release(output);
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
    pub(crate) settings: Arc<RwLock<Option<ImmutableObsData>>>,

    /// Hotkey configuration data for the output
    pub(crate) hotkey_data: Arc<RwLock<Option<ImmutableObsData>>>,

    /// Video encoders attached to this output
    pub(crate) curr_video_encoder: Arc<RwLock<Option<Arc<ObsVideoEncoder>>>>,

    /// Audio encoders attached to this output
    pub(crate) audio_encoders: Arc<RwLock<HashMap<usize, Arc<ObsAudioEncoder>>>>,

    /// Pointer to the underlying OBS output
    pub(crate) output: Sendable<*mut obs_output>,

    /// The type identifier of this output
    pub(crate) id: ObsString,

    /// The unique name of this output
    pub(crate) name: ObsString,

    pub(crate) runtime: ObsRuntime,

    /// RAII guard that ensures proper cleanup when the output is dropped
    _drop_guard: Arc<_ObsOutputDropGuard>,
}

impl ObsOutputTraitSealed for ObsOutputRef {
    fn new(output: OutputInfo, runtime: ObsRuntime) -> Result<Self, ObsError> {
        let (output, id, name, settings, hotkey_data) = runtime.run_with_obs_result(|| {
            let OutputInfo {
                id,
                name,
                settings,
                hotkey_data,
            } = output;

            let settings_ptr = match settings.as_ref() {
                Some(x) => x.as_ptr(),
                None => Sendable(ptr::null_mut()),
            };

            let hotkey_data_ptr = match hotkey_data.as_ref() {
                Some(x) => x.as_ptr(),
                None => Sendable(ptr::null_mut()),
            };

            let output = unsafe {
                libobs::obs_output_create(
                    id.as_ptr().0,
                    name.as_ptr().0,
                    settings_ptr.0,
                    hotkey_data_ptr.0,
                )
            };

            (Sendable(output), id, name, settings, hotkey_data)
        })?;

        if output.0.is_null() {
            return Err(ObsError::NullPointer);
        }

        let signal_manager = ObsOutputSignals::new(&output, runtime.clone())?;
        Ok(Self {
            settings: Arc::new(RwLock::new(settings)),
            hotkey_data: Arc::new(RwLock::new(hotkey_data)),

            curr_video_encoder: Arc::new(RwLock::new(None)),
            audio_encoders: Arc::new(RwLock::new(HashMap::new())),

            output: output.clone(),
            id,
            name,

            _drop_guard: Arc::new(_ObsOutputDropGuard {
                output,
                runtime: runtime.clone(),
            }),

            runtime,
            signal_manager: Arc::new(signal_manager),
        })
    }
}

impl ObsObjectTraitSealed for ObsOutputRef {
    fn replace_settings(&self, settings: ImmutableObsData) -> Result<(), ObsError> {
        self.settings
            .write()
            .map_err(|_| ObsError::LockError("Failed to acquire write lock on settings".into()))?
            .replace(settings);
        Ok(())
    }

    fn replace_hotkey_data(&self, hotkey_data: ImmutableObsData) -> Result<(), ObsError> {
        self.hotkey_data
            .write()
            .map_err(|_| ObsError::LockError("Failed to acquire write lock on hotkey data".into()))?
            .replace(hotkey_data);

        Ok(())
    }
}

impl ObsObjectTrait for ObsOutputRef {
    fn name(&self) -> ObsString {
        self.name.clone()
    }

    fn id(&self) -> ObsString {
        self.id.clone()
    }

    fn runtime(&self) -> &ObsRuntime {
        &self.runtime
    }

    fn settings(&self) -> &ImmutableObsData {
        &self.settings
    }

    fn hotkey_data(&self) -> &ImmutableObsData {
        &self.hotkey_data
    }
}

impl ObsOutputTrait for ObsOutputRef {
    fn signal_manager(&self) -> &Arc<ObsOutputSignals> {
        &self.signal_manager
    }

    fn video_encoder(&self) -> &Arc<RwLock<Option<Arc<ObsVideoEncoder>>>> {
        &self.curr_video_encoder
    }

    fn audio_encoders(&self) -> &Arc<RwLock<HashMap<usize, Arc<ObsAudioEncoder>>>> {
        &self.audio_encoders
    }

    fn as_ptr(&self) -> Sendable<*mut obs_output> {
        self.output.clone()
    }
}

impl_signal_manager!(|ptr| unsafe { libobs::obs_output_get_signal_handler(ptr) }, ObsOutputSignals for ObsOutputRef<*mut libobs::obs_output>, [
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
