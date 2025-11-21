use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{RedrawWindow, RDW_ERASE, RDW_INVALIDATE},
    UI::WindowsAndMessaging::{
        SetWindowPos, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOCOPYBITS, SWP_NOSIZE, SWP_NOZORDER,
        SWP_SHOWWINDOW,
    },
};

use crate::display::window_manager::WindowPositionTrait;
use crate::utils::ObsError;
use crate::{display::ObsDisplayRef, run_with_obs};

impl WindowPositionTrait for ObsDisplayRef {
    fn set_render_at_bottom(&self, render_at_bottom: bool) -> Result<(), ObsError> {
        log::trace!("Set render bottom");
        let mut m = self
            .manager
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        m.render_at_bottom = render_at_bottom;
        Ok(())
    }

    fn get_render_at_bottom(&self) -> Result<bool, ObsError> {
        let m = self
            .manager
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        Ok(m.render_at_bottom)
    }

    fn set_pos(&self, x: i32, y: i32) -> Result<(), ObsError> {
        log::trace!("Set pos {x} {y}");
        let mut m = self
            .manager
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
        Ok(())
    }

    fn set_size(&self, width: u32, height: u32) -> Result<(), ObsError> {
        log::trace!("Set size {width} {height}");
        let mut m = self
            .manager
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        assert!(
            m.obs_display.is_some(),
            "Invalid state. The display should have been created and set, but it wasn't."
        );

        m.width = width;
        m.height = height;

        let pointer = m.obs_display.as_ref().unwrap().clone();
        unsafe {
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

        run_with_obs!(self.runtime, (pointer), move || unsafe {
            libobs::obs_display_resize(pointer, width, height);
            // Update color space when window size changes
            libobs::obs_display_update_color_space(pointer);
        })
        .map_err(|e| ObsError::InvocationError(format!("{:?}", e)))?;
        Ok(())
    }

    fn set_scale(&self, scale: f32) -> Result<(), ObsError> {
        log::trace!("Set scale {scale}");
        let mut m = self
            .manager
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        m.scale = scale;
        Ok(())
    }

    fn get_pos(&self) -> Result<(i32, i32), ObsError> {
        let m = self
            .manager
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        Ok((m.x, m.y))
    }

    fn get_size(&self) -> Result<(u32, u32), ObsError> {
        let m = self
            .manager
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        Ok((m.width, m.height))
    }

    fn get_scale(&self) -> Result<f32, ObsError> {
        let m = self
            .manager
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        Ok(m.scale)
    }
}
