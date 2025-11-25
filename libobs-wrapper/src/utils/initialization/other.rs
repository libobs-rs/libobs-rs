use std::ptr;

#[cfg(target_os = "linux")]
use std::sync::Arc;

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
}

impl Drop for PlatformSpecificGuard {
    fn drop(&mut self) {
        unsafe {
            match self.platform {
                PlatformType::X11 => {
                    let result = XCloseDisplay(self.display.0);
                    if result != 0 {
                        eprintln!(
                            "[libobs-wrapper]: Warning: XCloseDisplay returned non-zero: {}",
                            result
                        );
                    }
                }
                PlatformType::Wayland => {
                    wl_display_disconnect(self.display.0);
                }
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn platform_specific_setup() -> Result<Option<Arc<PlatformSpecificGuard>>, ObsError> {
    return Ok(None);
}

/// Detects the current display server and initializes OBS platform accordingly
#[cfg(target_os = "linux")]
pub(crate) fn platform_specific_setup(
    display: Option<NixDisplay>,
) -> Result<Option<Arc<PlatformSpecificGuard>>, ObsError> {
    let mut display_ptr = None;

    let platform_type = match display {
        Some(NixDisplay::X11(e)) => {
            display_ptr = Some(e);
            PlatformType::X11
        }
        Some(NixDisplay::Wayland(e)) => {
            display_ptr = Some(e);
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

    unsafe {
        match platform_type {
            PlatformType::X11 => {
                use crate::{logger::internal_log_global, utils::linux::XOpenDisplay};

                libobs::obs_set_nix_platform(
                    libobs::obs_nix_platform_type_OBS_NIX_PLATFORM_X11_EGL,
                );

                // Try to get X11 display - note: this may fail in headless environments
                let display = display_ptr
                    .map(|e| e.0)
                    .unwrap_or_else(|| XOpenDisplay(ptr::null()));
                if display.is_null() {
                    return Err(ObsError::PlatformInitError(
                        "Failed to open X11 display".to_string(),
                    ));
                }

                libobs::obs_set_nix_platform_display(display);

                internal_log_global(
                    crate::enums::ObsLogLevel::Info,
                    "[libobs-wrapper]: Detected Platform: EGL/X11".to_string(),
                );

                //TODO make sure when creating a display that the same platform is used
                Ok(Some(Arc::new(PlatformSpecificGuard {
                    display: Sendable(display),
                    platform: PlatformType::X11,
                })))
            }
            PlatformType::Wayland => {
                use crate::{
                    enums::ObsLogLevel, logger::internal_log_global,
                    utils::linux::wl_display_connect,
                };

                libobs::obs_set_nix_platform(
                    libobs::obs_nix_platform_type_OBS_NIX_PLATFORM_WAYLAND,
                );

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

                Ok(Some(Arc::new(PlatformSpecificGuard {
                    display: Sendable(display),
                    platform: PlatformType::Wayland,
                })))
            }
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[derive(Debug)]
enum PlatformType {
    X11,
    Wayland,
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
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
