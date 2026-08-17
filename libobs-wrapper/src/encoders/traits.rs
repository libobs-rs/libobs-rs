use std::ffi::CStr;

use crate::{data::object::ObsObjectTrait, run_with_obs, utils::ObsError};

pub trait ObsEncoderTrait: ObsObjectTrait<Native = *mut libobs::obs_encoder> {
    /// Returns the codec identifier reported by the live encoder instance.
    fn codec(&self) -> Result<Option<String>, ObsError> {
        let encoder_ptr = self.__native_handle();
        run_with_obs!(self.runtime(), (encoder_ptr), move || {
            // Safety: the managed encoder handle remains alive for the actor call.
            let codec = unsafe { libobs::obs_encoder_get_codec(encoder_ptr.get_ptr()) };
            if codec.is_null() {
                None
            } else {
                // Safety: libobs returns a borrowed NUL-terminated string for the encoder lifetime.
                Some(
                    unsafe { CStr::from_ptr(codec) }
                        .to_string_lossy()
                        .into_owned(),
                )
            }
        })
    }

    fn is_active(&self) -> Result<bool, ObsError> {
        let encoder_ptr = self.__native_handle();

        run_with_obs!(self.runtime(), (encoder_ptr), move || {
            // Safety: The pointer is valid because we are using a smart pointer
            unsafe { libobs::obs_encoder_active(encoder_ptr.get_ptr()) }
        })
    }
}
