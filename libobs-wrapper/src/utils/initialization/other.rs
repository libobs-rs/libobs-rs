use std::ptr;

#[cfg(target_os = "linux")]
use std::rc::Rc;

use crate::unsafe_send::Sendable;

#[cfg(target_os = "linux")]
use crate::utils::initialization::NixDisplay;
use crate::utils::ObsError;

#[cfg(target_os = "linux")]
use crate::utils::linux::{wl_display_disconnect, XCloseDisplay};

#[derive(Debug)]
pub(crate) struct PlatformSpecificGuard {
    display: Sendable<*mut std::os::raw::c_void>,
    platform: PlatformType,
    /// Whether the guard owns the display connection and should close it on drop
    owned: bool,
}

impl Drop for PlatformSpecificGuard {
    fn drop(&mut self) {
        if !self.owned {
            // Display connection was provided by the caller (e.g. winit); do not close it.
            return;
        }
        match self.platform {
            PlatformType::X11 => {
                let result = unsafe {
                    // Safety: We do own the display connection, so we can close it.
                    XCloseDisplay(self.display.0)
                };
                if result != 0 {
                    eprintln!(
                        "[libobs-wrapper]: Warning: XCloseDisplay returned non-zero: {}",
                        result
                    );
                }
            }
            PlatformType::Wayland => {
                unsafe {
                    // Safety: We do own the display connection, so we can disconnect it.
                    wl_display_disconnect(self.display.0);
                };
            }
            _ => {}
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn platform_specific_setup() -> Result<Option<Rc<PlatformSpecificGuard>>, ObsError> {
    return Ok(None);
}

/// Detects the current display server and initializes OBS platform accordingly
/// # Safety
/// You must ensure that the display is valid, if set and that this function
/// is running on the OBS runtime.
#[cfg(target_os = "linux")]
#[allow(unknown_lints, ensure_obs_call_in_runtime)]
pub(crate) unsafe fn platform_specific_setup(
    display: Option<NixDisplay>,
) -> Result<Option<Rc<PlatformSpecificGuard>>, ObsError> {
    let mut display_ptr = None;
    let mut owned = true;

    let platform_type = match display {
        Some(NixDisplay::X11(e)) => {
            display_ptr = Some(e);
            owned = false;
            PlatformType::X11
        }
        Some(NixDisplay::Wayland(e)) => {
            display_ptr = Some(e);
            owned = false;
            PlatformType::Wayland
        }
        None => {
            // Auto-detect platform
            match detect_platform() {
                Some(plat) => plat,
                None => {
                    return Err(ObsError::PlatformInitError(
                        "Could not detect display server platform".to_string(),
                    ))
                }
            }
        }
    };

    match platform_type {
        PlatformType::X11 => {
            use crate::{logger::internal_log_global, utils::linux::XOpenDisplay};

            unsafe {
                // Safety: We are in the runtime and we are using X11, so we set the proper platform
                libobs::obs_set_nix_platform(
                    libobs::obs_nix_platform_type_OBS_NIX_PLATFORM_X11_EGL,
                );
            }

            // Try to get X11 display - note: this may fail in headless environments
            let display = display_ptr.map(|e| e.0).unwrap_or_else(|| unsafe {
                // Safety: We are in the runtime and using X11, so we can open the display and the display name should be inherited from env variables.
                XOpenDisplay(ptr::null())
            });
            if display.is_null() {
                return Err(ObsError::PlatformInitError(
                    "Failed to open X11 display".to_string(),
                ));
            }

            unsafe {
                // Safety: We are in the runtime and using X11, so we can set the display because it was opened by us
                libobs::obs_set_nix_platform_display(display);
            }

            internal_log_global(
                crate::enums::ObsLogLevel::Info,
                "[libobs-wrapper]: Detected Platform: EGL/X11".to_string(),
            );

            //TODO make sure when creating a display that the same platform is used
            Ok(Some(Rc::new(PlatformSpecificGuard {
                display: Sendable(display),
                platform: PlatformType::X11,
                owned,
            })))
        }
        PlatformType::Wayland => {
            use crate::{
                enums::ObsLogLevel, logger::internal_log_global, utils::linux::wl_display_connect,
            };

            libobs::obs_set_nix_platform(libobs::obs_nix_platform_type_OBS_NIX_PLATFORM_WAYLAND);

            // Try to get Wayland display - note: this may fail in headless environments
            let display = display_ptr
                .map(|e| e.0)
                .unwrap_or_else(|| wl_display_connect(ptr::null()));

            if display.is_null() {
                return Err(ObsError::PlatformInitError(
                    "Failed to connect to Wayland display".to_string(),
                ));
            }

            libobs::obs_set_nix_platform_display(display);

            internal_log_global(
                ObsLogLevel::Info,
                "[libobs-wrapper]: Detected Platform: Wayland".to_string(),
            );

            Ok(Some(Rc::new(PlatformSpecificGuard {
                display: Sendable(display),
                platform: PlatformType::Wayland,
                owned,
            })))
        }
        PlatformType::Invalid => unreachable!(),
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Debug)]
pub enum PlatformType {
    X11,
    Wayland,
    Invalid,
}

#[cfg(target_os = "linux")]
fn detect_platform() -> Option<PlatformType> {
    // Check for Wayland first
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return Some(PlatformType::Wayland);
    }

    // Check for X11
    if std::env::var("DISPLAY").is_ok() {
        // Could be XWayland, check XDG_SESSION_TYPE for more accuracy
        if let Ok(session_type) = std::env::var("XDG_SESSION_TYPE") {
            return match session_type.as_str() {
                "wayland" => Some(PlatformType::Wayland),
                "x11" => Some(PlatformType::X11),
                _ => None,
            };
        }
        return Some(PlatformType::X11);
    }

    None
}
