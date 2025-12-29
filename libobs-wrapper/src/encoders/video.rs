use libobs::{obs_encoder, video_output};
use std::{
    ptr,
    sync::{Arc, RwLock},
};

use crate::{
    data::{
        object::{inner_fn_update_settings, ObsObjectTrait, ObsObjectTraitSealed},
        ImmutableObsData, ObsData, ObsDataPointers,
    },
    encoders::ObsEncoderTrait,
    impl_obs_drop, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::Sendable,
    utils::{ObsError, ObsString, VideoEncoderInfo},
};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ObsVideoEncoder {
    pub(crate) encoder: Sendable<*mut obs_encoder>,
    pub(crate) id: ObsString,
    pub(crate) name: ObsString,
    pub(crate) settings: Arc<RwLock<ImmutableObsData>>,
    pub(crate) hotkey_data: Arc<RwLock<ImmutableObsData>>,
    pub(crate) runtime: ObsRuntime,
}

impl ObsVideoEncoder {
    /// Info: the handler attribute is no longer needed and kept for compatibility. The `handler` parameter will be removed in a future release.
    pub fn new_from_info(
        info: VideoEncoderInfo,
        runtime: ObsRuntime,
    ) -> Result<Arc<Self>, ObsError> {
        let settings_ptr = match &info.settings {
            Some(x) => x.as_ptr(),
            None => Sendable(ptr::null_mut()),
        };

        let hotkey_data_ptr = match &info.hotkey_data {
            Some(x) => x.as_ptr(),
            None => Sendable(ptr::null_mut()),
        };

        let id_ptr = info.id.as_ptr();
        let name_ptr = info.name.as_ptr();
        let encoder_ptr = run_with_obs!(
            runtime,
            (id_ptr, name_ptr, hotkey_data_ptr, settings_ptr),
            move || unsafe {
                let ptr = libobs::obs_video_encoder_create(
                    id_ptr,
                    name_ptr,
                    settings_ptr,
                    hotkey_data_ptr,
                );
                Sendable(ptr)
            }
        )?;

        if encoder_ptr.0.is_null() {
            return Err(ObsError::NullPointer);
        }

        let hotkey_data = match info.hotkey_data {
            Some(h) => h,
            None => ImmutableObsData::new(&runtime)?,
        };

        let settings = {
            let settings_ptr = run_with_obs!(runtime, (encoder_ptr), move || unsafe {
                Sendable(libobs::obs_encoder_get_settings(encoder_ptr))
            })?;
            ImmutableObsData::from_raw(settings_ptr, runtime.clone())
        };

        Ok(Arc::new(Self {
            encoder: encoder_ptr,
            id: info.id,
            name: info.name,
            settings: Arc::new(RwLock::new(settings)),
            hotkey_data: Arc::new(RwLock::new(hotkey_data)),
            runtime,
        }))
    }

    /// This is only needed once for global video context
    pub fn set_video_context(
        &mut self,
        handler: Sendable<*mut video_output>,
    ) -> Result<(), ObsError> {
        let self_ptr = self.as_ptr();
        run_with_obs!(self.runtime, (handler, self_ptr), move || unsafe {
            libobs::obs_encoder_set_video(self_ptr, handler);
        })
    }
}

impl_obs_drop!(ObsVideoEncoder, (encoder), move || unsafe {
    libobs::obs_encoder_release(encoder);
});

impl ObsObjectTraitSealed for ObsVideoEncoder {
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

impl ObsObjectTrait for ObsVideoEncoder {
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
}

impl ObsEncoderTrait for ObsVideoEncoder {
    fn as_ptr(&self) -> Sendable<*mut obs_encoder> {
        self.encoder.clone()
    }
}
