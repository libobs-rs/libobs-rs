//! This is derived from the frontend/obs-main.cpp.

use crate::utils::initialization::NixDisplay;
use std::sync::{Arc, Once};

use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE, LUID},
        Security::{
            AdjustTokenPrivileges, LookupPrivilegeValueW, SE_DEBUG_NAME, SE_INC_BASE_PRIORITY_NAME,
            SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
        UI::HiDpi::{
            SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        },
    },
};

use crate::utils::ObsError;

fn become_dpi_aware() {
    static ENABLE_DPI_AWARENESS: Once = Once::new();
    ENABLE_DPI_AWARENESS.call_once(|| unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_err() {
            let res = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE);
            if res.is_err() {
                log::error!("Failed to set process DPI awareness: {res:?}");
            }
        }
    });
}

#[derive(Debug)]
pub(crate) struct PlatformSpecificGuard {}
pub fn platform_specific_setup(
    _display: Option<NixDisplay>,
) -> Result<Option<Arc<PlatformSpecificGuard>>, ObsError> {
    become_dpi_aware();

    unsafe {
        let flags = TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY;
        let mut tp = TOKEN_PRIVILEGES::default();
        let mut token = HANDLE::default();
        let mut val = LUID::default();

        if OpenProcessToken(GetCurrentProcess(), flags, &mut token).is_err() {
            return Ok(None);
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
    }

    Ok(None)
}
