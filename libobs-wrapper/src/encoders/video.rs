use libobs::{obs_encoder, video_output};
use std::{ptr, sync::Arc};

use crate::{
    data::ObsData,
    impl_obs_drop, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::Sendable,
    utils::{ObsError, ObsString, VideoEncoderInfo},
};

#[derive(Debug)]
#[allow(dead_code)]
pub struct ObsVideoEncoder {
    pub(crate) encoder: Sendable<*mut obs_encoder>,
    pub(crate) id: ObsString,
    pub(crate) name: ObsString,
    pub(crate) settings: Option<ObsData>,
    pub(crate) hotkey_data: Option<ObsData>,
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
        let encoder = run_with_obs!(
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

        if encoder.0.is_null() {
            return Err(ObsError::NullPointer(None));
        }

        Ok(Arc::new(Self {
            encoder,
            id: info.id,
            name: info.name,
            settings: info.settings,
            hotkey_data: info.hotkey_data,
            runtime,
        }))
    }

    pub fn as_ptr(&self) -> Sendable<*mut obs_encoder> {
        self.encoder.clone()
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

    pub fn is_active(&self) -> Result<bool, ObsError> {
        let encoder_ptr = self.as_ptr();

        run_with_obs!(self.runtime, (encoder_ptr), move || unsafe {
            libobs::obs_encoder_active(encoder_ptr)
        })
    }

    pub fn update_settings(&mut self, settings: &ObsData) -> Result<(), ObsError> {
        let encoder_ptr = self.as_ptr();
        if self.is_active()? {
            return Err(ObsError::EncoderActive);
        }

        let settings_ptr = settings.as_ptr();

        run_with_obs!(self.runtime, (encoder_ptr, settings_ptr), move || unsafe {
            libobs::obs_encoder_update(encoder_ptr, settings_ptr);
        })
    }
}

impl_obs_drop!(ObsVideoEncoder, (encoder), move || unsafe {
    libobs::obs_encoder_release(encoder);
});
