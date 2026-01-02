use crate::display::window_manager::MiscDisplayTrait;
use crate::display::ObsDisplayRef;
use crate::run_with_obs;
use crate::utils::ObsError;

impl MiscDisplayTrait for ObsDisplayRef {
    fn is_enabled(&self) -> Result<bool, ObsError> {
        let display_ptr = self.as_ptr();
        run_with_obs!(self.runtime, (display_ptr), move || {
            unsafe {
                // Safety: The pointer is valid because we are using a smart pointer
                libobs::obs_display_enabled(display_ptr.get_ptr())
            }
        })
    }

    fn set_enabled(&self, enabled: bool) -> Result<(), ObsError> {
        let display_ptr = self.as_ptr();

        run_with_obs!(self.runtime, (display_ptr), move || {
            unsafe {
                // Safety: The pointer is valid because we are using a smart pointer
                libobs::obs_display_set_enabled(display_ptr.get_ptr(), enabled)
            }
        })
    }

    fn set_background_color(&self, r: u8, g: u8, b: u8) -> Result<(), ObsError> {
        let color: u32 = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        let display_ptr = self.as_ptr();

        run_with_obs!(self.runtime, (display_ptr), move || {
            unsafe {
                // Safety: The pointer is valid because we are using a smart pointer and the color is literally just a number
                libobs::obs_display_set_background_color(display_ptr.get_ptr(), color)
            }
        })
    }
}
