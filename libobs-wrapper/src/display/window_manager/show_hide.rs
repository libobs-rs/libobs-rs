#[cfg(windows)]
use std::sync::atomic::Ordering;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOWNA};

use crate::display::window_manager::ShowHideTrait;
use crate::display::ObsDisplayRef;
use crate::run_with_obs;
use crate::utils::ObsError;

impl ShowHideTrait for ObsDisplayRef {
    /// Shows the window.
    ///
    /// # Panics
    /// if the internal lock is poisoned.
    fn show(&mut self) -> Result<(), ObsError> {
        log::trace!("show");

        #[cfg(windows)]
        if let Some(m) = &self.child_window_handler {
            let m = m
                .read()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
            unsafe {
                // Safety: The window handle is valid as long as the ObsDisplayRef exists
                let _ = ShowWindow(m.window_handle.get_hwnd(), SW_SHOWNA);
            }

            m.is_hidden.store(false, Ordering::Relaxed);
            return Ok(());
        }

        let ptr = self.as_ptr();
        run_with_obs!(self.runtime, (ptr), move || {
            unsafe {
                // Safety: The pointer is valid because we are using a smart pointer
                libobs::obs_display_set_enabled(ptr.get_ptr(), true);
            }
        })?;
        Ok(())
    }

    fn hide(&mut self) -> Result<(), ObsError> {
        log::trace!("hide");
        #[cfg(windows)]
        if let Some(m) = &self.child_window_handler {
            let m = m
                .read()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

            unsafe {
                // Safety: The window handle is valid as long as the ObsDisplayRef exists
                let _ = ShowWindow(m.window_handle.get_hwnd(), SW_HIDE);
            }

            m.is_hidden.store(true, Ordering::Relaxed);
            return Ok(());
        }

        let ptr = self.as_ptr();
        run_with_obs!(self.runtime, (ptr), move || {
            unsafe {
                // Safety: The pointer is valid because we are using a smart pointer
                libobs::obs_display_set_enabled(ptr.get_ptr(), false);
            }
        })?;
        Ok(())
    }

    fn is_visible(&self) -> Result<bool, ObsError> {
        #[cfg(windows)]
        if let Some(m) = &self.child_window_handler {
            let m = m
                .read()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

            return Ok(!m.is_hidden.load(Ordering::Relaxed));
        }

        let ptr = self.as_ptr();
        run_with_obs!(self.runtime, (ptr), move || {
            let enabled = unsafe {
                // Safety: The pointer is valid because we are using a smart pointer
                libobs::obs_display_enabled(ptr.get_ptr())
            };

            Ok(enabled)
        })?
    }
}
