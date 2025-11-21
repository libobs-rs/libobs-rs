use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use std::sync::Arc;

use crate::unsafe_send::Sendable;
use crate::utils::ObsError;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
extern "C" {
    // X11 functions
    fn XOpenDisplay(display_name: *const c_char) -> *mut std::os::raw::c_void;
    fn XCloseDisplay(display: *mut std::os::raw::c_void) -> i32;

    // Wayland functions
    fn wl_display_connect(name: *const c_char) -> *mut std::os::raw::c_void;
    fn wl_display_disconnect(display: *mut std::os::raw::c_void);
}

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

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) fn platform_specific_setup() -> Result<Option<Arc<PlatformSpecificGuard>>, ObsError> {
    return Ok(None);
}

/// Detects the current display server and initializes OBS platform accordingly
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) fn platform_specific_setup() -> Result<Option<Arc<PlatformSpecificGuard>>, ObsError> {
    let platform_type = detect_platform();
    if platform_type.is_none() {
        return Err(ObsError::PlatformInitError(
            "Could not detect display server platform".to_string(),
        ));
    }

    let platform_type = platform_type.unwrap();
    // Try to detect and initialize the platform
    unsafe {
        match platform_type {
            PlatformType::X11 => {
                libobs::obs_set_nix_platform(
                    libobs::obs_nix_platform_type_OBS_NIX_PLATFORM_X11_EGL,
                );

                // Try to get X11 display - note: this may fail in headless environments
                let display = XOpenDisplay(ptr::null());
                if display.is_null() {
                    return Err(ObsError::PlatformInitError(
                        "Failed to open X11 display".to_string(),
                    ));
                }

                libobs::obs_set_nix_platform_display(display);

                let message = CString::new("[libobs-wrapper]: Detected Platform: EGL/X11").unwrap();
                libobs::blog(libobs::LOG_INFO as i32, message.as_ptr());

                Ok(Some(Arc::new(PlatformSpecificGuard {
                    display: Sendable(display),
                    platform: PlatformType::X11,
                })))
            }
            PlatformType::Wayland => {
                libobs::obs_set_nix_platform(
                    libobs::obs_nix_platform_type_OBS_NIX_PLATFORM_WAYLAND,
                );

                // Try to get Wayland display - note: this may fail in headless environments
                let display = wl_display_connect(ptr::null());
                if display.is_null() {
                    return Err(ObsError::PlatformInitError(
                        "Failed to connect to Wayland display".to_string(),
                    ));
                }

                libobs::obs_set_nix_platform_display(display);

                let message = CString::new("[libobs-wrapper]: Detected Platform: Wayland").unwrap();
                libobs::blog(libobs::LOG_INFO as i32, message.as_ptr());

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
