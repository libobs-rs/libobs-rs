use libobs::obs_encoder;

use crate::{data::object::ObsObjectTrait, run_with_obs, unsafe_send::Sendable, utils::ObsError};

pub trait ObsEncoderTrait: ObsObjectTrait {
    fn as_ptr(&self) -> Sendable<*mut obs_encoder>;

    fn is_active(&self) -> Result<bool, ObsError> {
        let encoder_ptr = self.as_ptr();

        run_with_obs!(self.runtime(), (encoder_ptr), move || unsafe {
            libobs::obs_encoder_active(encoder_ptr)
        })
    }
}
