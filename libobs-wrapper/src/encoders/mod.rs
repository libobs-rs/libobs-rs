use crate::{
    context::ObsContext,
    enums::{ObsEncoderType, OsEnumType},
    run_with_obs,
    runtime::ObsRuntime,
    utils::{ObsError, ENCODER_HIDE_FLAGS},
};
use std::{ffi::CStr, os::raw::c_char};

pub mod audio;
mod enums;
mod traits;
pub use traits::*;
mod property_helper;
pub use property_helper::*;
pub mod video;
pub use enums::*;

pub trait ObsContextEncoders {
    fn best_video_encoder(&self) -> Result<ObsVideoEncoderBuilder, ObsError>;

    fn best_audio_encoder(&self) -> Result<ObsAudioEncoderBuilder, ObsError>;

    fn available_audio_encoders(&self) -> Result<Vec<ObsAudioEncoderBuilder>, ObsError>;

    fn available_video_encoders(&self) -> Result<Vec<ObsVideoEncoderBuilder>, ObsError>;
}

fn get_encoders_raw(
    encoder_type: ObsEncoderType,
    runtime: &ObsRuntime,
) -> Result<Vec<String>, ObsError> {
    let type_primitive = encoder_type as OsEnumType;

    run_with_obs!(runtime, move || {
        let mut n = 0;
        let mut encoders = Vec::new();

        let mut ptr: *const c_char = unsafe { std::mem::zeroed() };
        while unsafe { libobs::obs_enum_encoder_types(n, &mut ptr) } {
            n += 1;
            let cstring = unsafe { CStr::from_ptr(ptr) };
            if let Ok(enc) = cstring.to_str() {
                unsafe {
                    let is_hidden = libobs::obs_get_encoder_caps(ptr) & ENCODER_HIDE_FLAGS != 0;
                    if is_hidden || libobs::obs_get_encoder_type(ptr) != type_primitive {
                        continue;
                    }
                }

                log::debug!("Found encoder: {}", enc);
                encoders.push(enc.into());
            }
        }

        encoders.sort_unstable();
        encoders
    })
}

impl ObsContextEncoders for ObsContext {
    fn best_video_encoder(&self) -> Result<ObsVideoEncoderBuilder, ObsError> {
        let encoders = self.available_video_encoders()?;
        encoders
            .into_iter()
            .next()
            .ok_or(ObsError::NoAvailableEncoders)
    }

    fn best_audio_encoder(&self) -> Result<ObsAudioEncoderBuilder, ObsError> {
        let encoders = self.available_audio_encoders()?;
        encoders
            .into_iter()
            .next()
            .ok_or(ObsError::NoAvailableEncoders)
    }

    fn available_audio_encoders(&self) -> Result<Vec<ObsAudioEncoderBuilder>, ObsError> {
        Ok(get_encoders_raw(ObsEncoderType::Audio, &self.runtime)?
            .into_iter()
            .map(|x| ObsAudioEncoderBuilder::new(self.clone(), &x))
            .collect::<Vec<_>>())
    }

    fn available_video_encoders(&self) -> Result<Vec<ObsVideoEncoderBuilder>, ObsError> {
        Ok(get_encoders_raw(ObsEncoderType::Video, &self.runtime)?
            .into_iter()
            .map(|x| ObsVideoEncoderBuilder::new(self.clone(), &x))
            .collect::<Vec<_>>())
    }
}
