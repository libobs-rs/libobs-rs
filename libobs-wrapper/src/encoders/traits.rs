use libobs::obs_encoder;

use crate::{data::object::ObsObjectTrait, run_with_obs, utils::ObsError};

pub trait ObsEncoderTrait: ObsObjectTrait<*mut obs_encoder> {
    fn is_active(&self) -> Result<bool, ObsError> {
        let encoder_ptr = self.as_ptr();

        run_with_obs!(self.runtime(), (encoder_ptr), move || {
            // Safety: The pointer is valid because we are using a smart pointer
            unsafe { libobs::obs_encoder_active(encoder_ptr.get_ptr()) }
        })
    }
}
