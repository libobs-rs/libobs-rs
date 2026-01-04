use crate::{
    context::ObsContext,
    enums::{ObsEncoderType, OsEnumType},
    impl_obs_drop, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::Sendable,
    utils::{ObsDropGuard, ObsError, ENCODER_HIDE_FLAGS},
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

        let mut ptr: *const c_char = unsafe {
            // Safety: We are not dereferencing this pointer yet, it is first set by the method below
            // and we are in the runtime
            std::mem::zeroed()
        };
        while unsafe {
            // Safety: We initialized the memory above and are modifying the pointer in the loop
            libobs::obs_enum_encoder_types(n, &mut ptr)
        } {
            n += 1;
            if ptr.is_null() {
                continue;
            }

            let cstring = unsafe {
                // Safety: We made sure that the pointer is not null, so it must be valid
                CStr::from_ptr(ptr)
            };
            if let Ok(enc) = cstring.to_str() {
                unsafe {
                    // Safety: We know know that the pointer is valid, therefore we can use it again
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

#[derive(Debug)]
pub(super) struct _ObsEncoderDropGuard {
    encoder: Sendable<*mut libobs::obs_encoder_t>,
    runtime: ObsRuntime,
}

impl ObsDropGuard for _ObsEncoderDropGuard {}

impl_obs_drop!(_ObsEncoderDropGuard, (encoder), move || unsafe {
    // Safety: The pointer is valid because we are in the runtime and the guard is alive.
    libobs::obs_encoder_release(encoder.0);
});
