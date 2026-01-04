use libobs::{audio_output, obs_encoder};
use std::{
    ptr,
    sync::{Arc, RwLock},
};

use crate::{
    data::{
        object::{inner_fn_update_settings, ObsObjectTrait, ObsObjectTraitPrivate},
        ImmutableObsData, ObsDataPointers,
    },
    encoders::{ObsEncoderTrait, _ObsEncoderDropGuard},
    run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::{Sendable, SmartPointerSendable},
    utils::{AudioEncoderInfo, ObsError, ObsString},
};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ObsAudioEncoder {
    pub(crate) id: ObsString,
    pub(crate) name: ObsString,
    pub(crate) settings: Arc<RwLock<ImmutableObsData>>,
    pub(crate) hotkey_data: Arc<RwLock<ImmutableObsData>>,
    pub(crate) runtime: ObsRuntime,
    pub(crate) encoder: SmartPointerSendable<*mut libobs::obs_encoder_t>,
}

impl ObsAudioEncoder {
    /// Info: the handler attribute is no longer needed and kept for compatibility. The `handler` parameter will be removed in a future release.
    pub fn new_from_info(
        info: AudioEncoderInfo,
        mixer_idx: usize,
        runtime: ObsRuntime,
    ) -> Result<Arc<Self>, ObsError> {
        let AudioEncoderInfo {
            id,
            name,
            settings,
            hotkey_data,
        } = info;

        let settings_ptr = settings.as_ref().map(|s| s.as_ptr());
        let hotkey_data_ptr = hotkey_data.as_ref().map(|h| h.as_ptr());

        let encoder = run_with_obs!(
            runtime,
            (id, name, settings_ptr, hotkey_data_ptr),
            move || {
                let settings_ptr_raw = match settings_ptr {
                    Some(s) => s.get_ptr(),
                    None => ptr::null_mut(),
                };

                let hotkey_data_ptr_raw = match hotkey_data_ptr {
                    Some(h) => h.get_ptr(),
                    None => ptr::null_mut(),
                };

                let ptr = unsafe {
                    // Safety: All pointers are in the current scope and therefore valid.
                    libobs::obs_audio_encoder_create(
                        id.as_ptr().0,
                        name.as_ptr().0,
                        settings_ptr_raw,
                        mixer_idx,
                        hotkey_data_ptr_raw,
                    )
                };

                if ptr.is_null() {
                    Err(ObsError::NullPointer(None))
                } else {
                    Ok(Sendable(ptr))
                }
            }
        )??;

        let encoder = SmartPointerSendable::new(
            encoder.0,
            Arc::new(_ObsEncoderDropGuard {
                encoder,
                runtime: runtime.clone(),
            }),
        );

        let settings = {
            let settings_ptr = run_with_obs!(runtime, (encoder), move || unsafe {
                // Safety: We are using a smart pointer to ensure that the encoder pointer is valid
                Sendable(libobs::obs_encoder_get_settings(encoder.get_ptr()))
            })?;

            ImmutableObsData::from_raw_pointer(settings_ptr, runtime.clone())
        };

        let hotkey_data = match hotkey_data {
            Some(h) => h,
            None => ImmutableObsData::new(&runtime)?,
        };

        Ok(Arc::new(Self {
            encoder,
            id,
            name,
            settings: Arc::new(RwLock::new(settings)),
            hotkey_data: Arc::new(RwLock::new(hotkey_data)),
            runtime,
        }))
    }

    /// This is only needed once for global audio context
    /// # Safety
    /// You must ensure that the `handler` pointer is valid and lives as long as this function call.
    pub unsafe fn set_audio_context(
        &mut self,
        handler: Sendable<*mut audio_output>,
    ) -> Result<(), ObsError> {
        let encoder_ptr = self.encoder.clone();

        run_with_obs!(self.runtime, (handler, encoder_ptr), move || {
            unsafe {
                // Safety: Caller made sure that handler is valid and encoder_ptr is valid because of a SmartPointer
                libobs::obs_encoder_set_audio(encoder_ptr.get_ptr(), handler.0)
            }
        })
    }
}

impl ObsObjectTraitPrivate for ObsAudioEncoder {
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

impl ObsObjectTrait<*mut libobs::obs_encoder> for ObsAudioEncoder {
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

    fn as_ptr(&self) -> SmartPointerSendable<*mut obs_encoder> {
        self.encoder.clone()
    }
}

impl ObsEncoderTrait for ObsAudioEncoder {}
