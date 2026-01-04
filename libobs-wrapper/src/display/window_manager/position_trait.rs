#[cfg(windows)]
use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{RedrawWindow, RDW_ERASE, RDW_INVALIDATE},
    UI::WindowsAndMessaging::{
        SetWindowPos, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOCOPYBITS, SWP_NOSIZE, SWP_NOZORDER,
        SWP_SHOWWINDOW,
    },
};

use crate::display::window_manager::WindowPositionTrait;
use crate::display::DISPLAY_POSITIONS;
use crate::utils::ObsError;
use crate::{display::ObsDisplayRef, run_with_obs};

impl WindowPositionTrait for ObsDisplayRef {
    fn set_render_at_bottom(&self, _render_at_bottom: bool) -> Result<(), ObsError> {
        log::trace!("Set render bottom");
        #[cfg(windows)]
        if let Some(m) = self.child_window_handler.as_ref() {
            let mut m = m
                .write()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

            m.render_at_bottom = _render_at_bottom;
        }
        Ok(())
    }

    fn get_render_at_bottom(&self) -> Result<bool, ObsError> {
        #[cfg(windows)]
        if let Some(m) = self.child_window_handler.as_ref() {
            let m = m
                .read()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
            return Ok(m.render_at_bottom);
        }

        Ok(false)
    }

    fn set_pos(&self, x: i32, y: i32) -> Result<(), ObsError> {
        log::trace!("Set pos {x} {y}");

        #[cfg(windows)]
        if let Some(m) = self.child_window_handler.as_ref() {
            let mut m = m
                .write()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

            assert!(
                m.obs_display.is_some(),
                "Invalid state. The display should have been created and set, but it wasn't."
            );

            let insert_after = if m.render_at_bottom {
                HWND_BOTTOM
            } else {
                HWND::default()
            };

            m.x = x;
            m.y = y;

            unsafe {
                // SAFETY: The window handle is valid because it was created and stored in the struct during initialization.
                // SetWindowPos is a safe Windows API call when provided with a valid window handle and valid parameters.
                let flags = SWP_NOCOPYBITS | SWP_NOSIZE | SWP_NOACTIVATE;
                // Just use dummy values as size is not changed
                SetWindowPos(
                    m.window_handle.get_hwnd(),
                    Some(insert_after),
                    x,
                    y,
                    1_i32,
                    1_i32,
                    flags,
                )
                .map_err(|e| ObsError::DisplayCreationError(format!("{:?}", e)))?;
            }

            // Update color space when window position changes
            drop(m); // Release the lock before calling run_with_obs
            self.update_color_space()?;
            return Ok(());
        }

        *DISPLAY_POSITIONS
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?
            .get_mut(&self.id)
            .ok_or_else(|| ObsError::LockError("Position not found".to_string()))? = (x, y);

        self.update_color_space()?;
        Ok(())
    }

    fn set_size(&self, width: u32, height: u32) -> Result<(), ObsError> {
        log::trace!("Set size {width} {height}");

        #[cfg(windows)]
        if let Some(m) = self.child_window_handler.as_ref() {
            let mut m = m
                .write()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

            assert!(
                m.obs_display.is_some(),
                "Invalid state. The display should have been created and set, but it wasn't."
            );

            m.width = width;
            m.height = height;

            unsafe {
                // SAFETY: The window handle is valid because it was created and stored in the struct during initialization.
                // SetWindowPos is a Windows API call that is safe when provided with a valid window handle and valid parameters.
                SetWindowPos(
                    m.window_handle.get_hwnd(),
                    None,
                    m.x,
                    m.y,
                    width as i32,
                    height as i32,
                    SWP_NOCOPYBITS | SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
                )
                .map_err(|e| ObsError::DisplayCreationError(format!("{:?}", e)))?;

                let _ = RedrawWindow(
                    Some(m.window_handle.get_hwnd()),
                    None,
                    None,
                    RDW_ERASE | RDW_INVALIDATE,
                );
            }
        }

        let pointer = self.as_ptr();
        run_with_obs!(self.runtime, (pointer), move || {
            unsafe {
                // Safety: The pointer is valid because we are using a smart pointer

                libobs::obs_display_resize(pointer.get_ptr(), width, height);
                // Update color space when window size changes
                libobs::obs_display_update_color_space(pointer.get_ptr());
            }
        })
        .map_err(|e| ObsError::InvocationError(format!("{:?}", e)))?;
        Ok(())
    }

    fn get_pos(&self) -> Result<(i32, i32), ObsError> {
        #[cfg(windows)]
        if let Some(m) = self.child_window_handler.as_ref() {
            let m = m
                .read()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
            return Ok((m.x, m.y));
        }

        let pos = DISPLAY_POSITIONS
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        let pos = pos
            .get(&self.id)
            .ok_or_else(|| ObsError::LockError("Position not found".to_string()))?;

        Ok(*pos)
    }

    fn get_size(&self) -> Result<(u32, u32), ObsError> {
        #[cfg(windows)]
        if let Some(m) = self.child_window_handler.as_ref() {
            let m = m
                .read()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

            return Ok((m.width, m.height));
        }

        let pointer = self.as_ptr();
        let (width, height) = run_with_obs!(self.runtime, (pointer), move || {
            let mut w: u32 = 0;
            let mut h: u32 = 0;
            unsafe {
                // Safety: The pointer is valid because we are using a smart pointer and because `w` and `h` are valid mutable references.
                libobs::obs_display_size(pointer.get_ptr(), &mut w, &mut h);
            };

            (w, h)
        })
        .map_err(|e| ObsError::InvocationError(format!("{:?}", e)))?;

        Ok((width, height))
    }
}
