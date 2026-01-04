//! This is derived from the frontend/obs-main.cpp.

use crate::utils::initialization::NixDisplay;
use std::{rc::Rc, sync::atomic::AtomicBool};

use lazy_static::lazy_static;
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE, LUID},
        Security::{
            AdjustTokenPrivileges, LookupPrivilegeValueW, SE_DEBUG_NAME, SE_INC_BASE_PRIORITY_NAME,
            SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
        UI::HiDpi::{SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
    },
};

use crate::utils::ObsError;

#[derive(Debug)]
pub(crate) struct PlatformSpecificGuard {
    previous_dpi_context: Option<*mut std::ffi::c_void>,
}

lazy_static! {
    static ref HAS_SET_DPI_AWARENESS: AtomicBool = AtomicBool::new(false);
}

impl PlatformSpecificGuard {
    /// Helper method to enable DPI awareness for the current thread.
    fn enable_dpi_awareness() -> Result<PlatformSpecificGuard, ObsError> {
        if HAS_SET_DPI_AWARENESS
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_err()
        {
            // DPI awareness has already been set for this process
            log::debug!("DPI awareness has already been set for this process");
            return Ok(PlatformSpecificGuard {
                previous_dpi_context: None,
            });
        }

        let previous_context = unsafe {
            // SAFETY: SetThreadDpiAwarenessContext is a Windows API call that operates on the current thread.
            // The call is safe and does not require synchronization as it only affects the calling thread.
            SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
        };

        if !previous_context.is_invalid() {
            log::debug!("DPI awareness enabled for current thread");
            Ok(PlatformSpecificGuard {
                previous_dpi_context: Some(previous_context.0),
            })
        } else {
            log::warn!("Could not set DPI awareness context");
            Ok(PlatformSpecificGuard {
                previous_dpi_context: None,
            })
        }
    }

    pub fn unset_dpi_awareness(&self) {
        if let Some(previous_context) = self.previous_dpi_context {
            log::debug!("Restoring previous DPI context");

            // SAFETY: We are restoring a previously saved DPI awareness context from the same thread.
            // This is safe as long as the guard is not moved between threads, which is guaranteed
            // because the struct is not Send or Sync
            unsafe {
                let dpi_context =
                    windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT(previous_context);
                let _ = SetThreadDpiAwarenessContext(dpi_context);
            }
        }
    }
}

impl Drop for PlatformSpecificGuard {
    fn drop(&mut self) {
        self.unset_dpi_awareness();
    }
}
/// # Safety
/// You must ensure that this function is running on the OBS runtime.
pub unsafe fn platform_specific_setup(
    _display: Option<NixDisplay>,
) -> Result<Option<Rc<PlatformSpecificGuard>>, ObsError> {
    // Enable DPI awareness for the current thread
    let platform_guard = PlatformSpecificGuard::enable_dpi_awareness()?;

    let flags = TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY;
    let mut tp = TOKEN_PRIVILEGES::default();
    let mut token = HANDLE::default();
    let mut val = LUID::default();

    if OpenProcessToken(GetCurrentProcess(), flags, &mut token).is_err() {
        return Ok(Some(Rc::new(platform_guard)));
    }

    if LookupPrivilegeValueW(PCWSTR::null(), SE_DEBUG_NAME, &mut val).is_ok() {
        tp.PrivilegeCount = 1;
        tp.Privileges[0].Luid = val;
        tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;

        let res = AdjustTokenPrivileges(
            token,
            false,
            Some(&tp),
            std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
            None,
            None,
        );
        if let Err(e) = res {
            log::error!("Could not set privilege to debug process: {e:?}");
        }
    }

    if LookupPrivilegeValueW(PCWSTR::null(), SE_INC_BASE_PRIORITY_NAME, &mut val).is_ok() {
        tp.PrivilegeCount = 1;
        tp.Privileges[0].Luid = val;
        tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;

        let res = AdjustTokenPrivileges(
            token,
            false,
            Some(&tp),
            std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
            None,
            None,
        );

        if let Err(e) = res {
            log::error!("Could not set privilege to increase GPU priority {e:?}");
        }
    }

    let _ = CloseHandle(token);

    Ok(Some(Rc::new(platform_guard)))
}
