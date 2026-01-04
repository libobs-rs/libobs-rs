use libobs::{obs_encoder, video_output};
use std::{
    ptr,
    sync::{Arc, RwLock},
};

use crate::{
    data::{
        object::{inner_fn_update_settings, ObsObjectTrait, ObsObjectTraitPrivate},
        ImmutableObsData, ObsData, ObsDataPointers,
    },
    encoders::{ObsEncoderTrait, _ObsEncoderDropGuard},
    run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::{Sendable, SmartPointerSendable},
    utils::{ObsError, ObsString, VideoEncoderInfo},
};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ObsVideoEncoder {
    pub(crate) id: ObsString,
    pub(crate) name: ObsString,
    pub(crate) settings: Arc<RwLock<ImmutableObsData>>,
    pub(crate) hotkey_data: Arc<RwLock<ImmutableObsData>>,
    pub(crate) runtime: ObsRuntime,
    pub(crate) encoder: SmartPointerSendable<*mut obs_encoder>,
}

impl ObsVideoEncoder {
    /// Info: the handler attribute is no longer needed and kept for compatibility. The `handler` parameter will be removed in a future release.
    pub fn new_from_info(
        info: VideoEncoderInfo,
        runtime: ObsRuntime,
    ) -> Result<Arc<Self>, ObsError> {
        let VideoEncoderInfo {
            id,
            name,
            settings,
            hotkey_data,
        } = info;

        let settings_ptr = settings.as_ref().map(|s| s.as_ptr());
        let hotkey_data_ptr = hotkey_data.as_ref().map(|h| h.as_ptr());

        let encoder_ptr = run_with_obs!(
            runtime,
            (id, name, hotkey_data_ptr, settings_ptr),
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
                    libobs::obs_video_encoder_create(
                        id.as_ptr().0,
                        name.as_ptr().0,
                        settings_ptr_raw,
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

        let encoder_ptr = SmartPointerSendable::new(
            encoder_ptr.0,
            Arc::new(_ObsEncoderDropGuard {
                encoder: encoder_ptr,
                runtime: runtime.clone(),
            }),
        );

        let hotkey_data = match hotkey_data {
            Some(h) => h,
            None => ImmutableObsData::new(&runtime)?,
        };

        let settings = {
            let settings_ptr = run_with_obs!(runtime, (encoder_ptr), move || {
                let ptr = unsafe {
                    // Safety: encoder_ptr is valid because of the SmartPointer
                    libobs::obs_encoder_get_settings(encoder_ptr.get_ptr())
                };

                Sendable(ptr)
            })?;
            ImmutableObsData::from_raw_pointer(settings_ptr, runtime.clone())
        };

        Ok(Arc::new(Self {
            encoder: encoder_ptr,
            id,
            name,
            settings: Arc::new(RwLock::new(settings)),
            hotkey_data: Arc::new(RwLock::new(hotkey_data)),
            runtime,
        }))
    }

    /// This is only needed once for global video context
    /// # Safety
    /// The handler pointer must be a valid pointer to a video_output that lives as long as this function call.
    pub unsafe fn set_video_context(
        &mut self,
        handler: Sendable<*mut video_output>,
    ) -> Result<(), ObsError> {
        let self_ptr = self.as_ptr();
        run_with_obs!(self.runtime, (handler, self_ptr), move || {
            unsafe {
                // Safety: Caller must make sure that the handler pointer is valid and the self pointer is a SmartPointer.
                libobs::obs_encoder_set_video(self_ptr.get_ptr(), handler.0);
            }
        })
    }
}

impl ObsObjectTraitPrivate for ObsVideoEncoder {
    fn __internal_replace_settings(&self, settings: ImmutableObsData) -> Result<(), ObsError> {
        self.settings
            .write()
            .map_err(|_| {
                ObsError::LockError(
                    "Failed to acquire lock for replacing settings in the video encoder".into(),
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
                    "Failed to acquire lock for replacing hotkey data in the video encoder".into(),
                )
            })
            .map(|mut guard| {
                *guard = hotkey_data;
            })
    }
}

impl ObsObjectTrait<*mut libobs::obs_encoder> for ObsVideoEncoder {
    fn runtime(&self) -> &ObsRuntime {
        &self.runtime
    }

    fn settings(&self) -> Result<ImmutableObsData, ObsError> {
        self.settings
            .read()
            .map_err(|_| {
                ObsError::LockError(
                    "Failed to acquire lock for reading settings in the video encoder".into(),
                )
            })
            .map(|s| s.clone())
    }

    fn hotkey_data(&self) -> Result<ImmutableObsData, ObsError> {
        self.hotkey_data
            .read()
            .map_err(|_| {
                ObsError::LockError(
                    "Failed to acquire lock for reading hotkey data in the video encoder".into(),
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

    /// Updates the settings of this output. Fails if active.
    fn update_settings(&self, settings: ObsData) -> Result<(), ObsError> {
        if self.is_active()? {
            return Err(ObsError::EncoderActive);
        }

        inner_fn_update_settings!(self, libobs::obs_encoder_update, settings)
    }

    fn as_ptr(&self) -> SmartPointerSendable<*mut obs_encoder> {
        self.encoder.clone()
    }
}

impl ObsEncoderTrait for ObsVideoEncoder {}
