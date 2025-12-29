use libobs::{audio_output, obs_encoder};
use std::{
    borrow::Borrow,
    ptr,
    sync::{Arc, RwLock},
};

use crate::{
    data::{
        object::{inner_fn_update_settings, ObsObjectTrait, ObsObjectTraitSealed},
        ImmutableObsData, ObsDataPointers,
    },
    encoders::ObsEncoderTrait,
    impl_obs_drop, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::Sendable,
    utils::{AudioEncoderInfo, ObsError, ObsString},
};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ObsAudioEncoder {
    pub(crate) encoder: Sendable<*mut libobs::obs_encoder_t>,
    pub(crate) id: ObsString,
    pub(crate) name: ObsString,
    pub(crate) settings: Arc<RwLock<ImmutableObsData>>,
    pub(crate) hotkey_data: Arc<RwLock<ImmutableObsData>>,
    pub(crate) runtime: ObsRuntime,
}

impl ObsAudioEncoder {
    /// Info: the handler attribute is no longer needed and kept for compatibility. The `handler` parameter will be removed in a future release.
    pub fn new_from_info(
        info: AudioEncoderInfo,
        mixer_idx: usize,
        runtime: ObsRuntime,
    ) -> Result<Arc<Self>, ObsError> {
        let settings_ptr = match info.settings.borrow() {
            Some(x) => x.as_ptr(),
            None => Sendable(ptr::null_mut()),
        };

        let hotkey_data_ptr = match info.hotkey_data.borrow() {
            Some(x) => x.as_ptr(),
            None => Sendable(ptr::null_mut()),
        };

        let id_ptr = info.id.as_ptr();
        let name_ptr = info.name.as_ptr();

        let encoder = run_with_obs!(
            runtime,
            (hotkey_data_ptr, settings_ptr, id_ptr, name_ptr),
            move || unsafe {
                let ptr = libobs::obs_audio_encoder_create(
                    id_ptr,
                    name_ptr,
                    settings_ptr,
                    mixer_idx,
                    hotkey_data_ptr,
                );
                Sendable(ptr)
            }
        )?;

        if encoder.0.is_null() {
            return Err(ObsError::NullPointer);
        }

        let settings = {
            let settings_ptr = run_with_obs!(runtime, (encoder), move || unsafe {
                Sendable(libobs::obs_encoder_get_settings(encoder))
            })?;

            ImmutableObsData::from_raw(settings_ptr, runtime.clone())
        };

        let hotkey_data = match info.hotkey_data.borrow() {
            Some(h) => h.clone(),
            None => ImmutableObsData::new(&runtime)?,
        };

        Ok(Arc::new(Self {
            encoder,
            id: info.id,
            name: info.name,
            settings: Arc::new(RwLock::new(settings)),
            hotkey_data: Arc::new(RwLock::new(hotkey_data)),
            runtime,
        }))
    }

    /// This is only needed once for global audio context
    pub fn set_audio_context(
        &mut self,
        handler: Sendable<*mut audio_output>,
    ) -> Result<(), ObsError> {
        let encoder_ptr = self.encoder.clone();

        run_with_obs!(self.runtime, (handler, encoder_ptr), move || unsafe {
            libobs::obs_encoder_set_audio(encoder_ptr, handler)
        })
    }
}

impl_obs_drop!(ObsAudioEncoder, (encoder), move || unsafe {
    libobs::obs_encoder_release(encoder);
});

impl ObsObjectTraitSealed for ObsAudioEncoder {
    fn __internal_replace_settings(&self, settings: ImmutableObsData) -> Result<(), ObsError> {
        self.settings
            .write()
            .map_err(|_| {
                ObsError::LockError(
                    "Failed to acquire lock for replacing settings in the audio encoder".into(),
                )
            })
            .map(|mut guard| {
                *guard = settings;
            })
    }

    fn __internal_replace_hotkey_data(
        &self,
        hotkey_data: ImmutableObsData,
    ) -> Result<(), ObsError> {
        self.hotkey_data
            .write()
            .map_err(|_| {
                ObsError::LockError(
                    "Failed to acquire lock for replacing hotkey data in the audio encoder".into(),
                )
            })
            .map(|mut guard| {
                *guard = hotkey_data;
            })
    }
}

impl ObsObjectTrait for ObsAudioEncoder {
    fn runtime(&self) -> &ObsRuntime {
        &self.runtime
    }

    fn settings(&self) -> Result<ImmutableObsData, ObsError> {
        self.settings
            .read()
            .map_err(|_| {
                ObsError::LockError("Failed to acquire read lock on audio encoder settings".into())
            })
            .map(|s| s.clone())
    }

    fn hotkey_data(&self) -> Result<ImmutableObsData, ObsError> {
        self.hotkey_data
            .read()
            .map_err(|_| {
                ObsError::LockError(
                    "Failed to acquire read lock on audio encoder hotkey data".into(),
                )
            })
            .map(|h| h.clone())
    }

    fn id(&self) -> ObsString {
        self.id.clone()
    }

    fn name(&self) -> ObsString {
        self.name.clone()
    }

    fn update_settings(&self, settings: crate::data::ObsData) -> Result<(), ObsError> {
        if self.is_active()? {
            return Err(ObsError::EncoderActive);
        }

        inner_fn_update_settings!(self, libobs::obs_encoder_update, settings)
    }
}

impl ObsEncoderTrait for ObsAudioEncoder {
    fn as_ptr(&self) -> Sendable<*mut obs_encoder> {
        self.encoder.clone()
    }
}
