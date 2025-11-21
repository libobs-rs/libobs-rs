use std::sync::atomic::Ordering;

use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOWNA};

use crate::display::window_manager::ShowHideTrait;
use crate::display::ObsDisplayRef;
use crate::utils::ObsError;

impl ShowHideTrait for ObsDisplayRef {
    /// Shows the window.
    ///
    /// # Panics
    /// if the internal lock is poisoned.
    fn show(&mut self) -> Result<(), ObsError> {
        log::trace!("show");
        let m = self
            .manager
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        unsafe {
            let _ = ShowWindow(m.window_handle.get_hwnd(), SW_SHOWNA);
        }

        m.is_hidden.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn hide(&mut self) -> Result<(), ObsError> {
        log::trace!("hide");
        let m = self
            .manager
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        unsafe {
            let _ = ShowWindow(m.window_handle.get_hwnd(), SW_HIDE);
        }

        m.is_hidden.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn is_visible(&self) -> Result<bool, ObsError> {
        let m = self
            .manager
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        Ok(!m.is_hidden.load(Ordering::Relaxed))
    }
}
