//! Contains linux specific bindings to x11 and wayland

use std::{fs, os::raw::c_char};

extern "C" {
    // X11 functions
    pub(crate) fn XOpenDisplay(display_name: *const c_char) -> *mut std::os::raw::c_void;
    pub(crate) fn XCloseDisplay(display: *mut std::os::raw::c_void) -> i32;

    // Wayland functions
    pub(crate) fn wl_display_connect(name: *const c_char) -> *mut std::os::raw::c_void;
    pub(crate) fn wl_display_disconnect(display: *mut std::os::raw::c_void);
}
#[derive(Debug)]
pub struct LinuxGlibLoop {
    glib_loop: glib::MainLoop,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl LinuxGlibLoop {
    pub fn new() -> Self {
        let g_loop = glib::MainLoop::new(None, false);
        let g_loop_clone = g_loop.clone();
        let handle = std::thread::spawn(move || {
            g_loop_clone.run();
        });

        Self {
            glib_loop: g_loop,
            handle: Some(handle),
        }
    }
}

impl Default for LinuxGlibLoop {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LinuxGlibLoop {
    fn drop(&mut self) {
        if self.glib_loop.is_running() {
            self.glib_loop.quit();
        }

        if let Some(handle) = self.handle.take() {
            let r = handle.join();
            if std::thread::panicking() {
                if let Err(e) = r {
                    log::error!(
                        "[libobs-wrapper]: Thread panicked while dropping LinuxGlibLoop: {:?}",
                        e
                    );
                }
            } else {
                r.unwrap();
            }
        }
    }
}

/// # Safety
/// You must ensure that the proxy is valid.
pub(crate) unsafe fn wl_proxy_get_display(
    proxy: *mut std::os::raw::c_void,
) -> Result<*mut std::os::raw::c_void, libloading::Error> {
    let lib = unsafe {
        // Safety: We are loading a system library that should be present on Wayland systems.
        libloading::Library::new("libwayland-client.so")
            .or_else(|_e| libloading::Library::new("libwayland-client.so.0"))?
    };

    let sym: Result<
        libloading::Symbol<
            unsafe extern "C" fn(*mut ::std::os::raw::c_void) -> *mut ::std::os::raw::c_void,
        >,
        libloading::Error,
    > = unsafe {
        // Safety: The function signature matches the expected signature of wl_proxy_get_display, except that it is not a c_void ptr but instead ws_display and wl_proxy but that should work as well.
        lib.get(b"wl_proxy_get_display\0")
    };

    match sym {
        Ok(f) => Ok(unsafe {
            // Safety: The caller must have made sure that this proxy is valid.
            f(proxy)
        }),
        Err(e) => Err(e),
    }
}

/// We are trying to get the correct OpenGL library name for Linux systems derived from the obs binary, this is a bit hacky.
pub(crate) fn get_linux_opengl_lib_name() -> String {
    let obs_bin = "/usr/bin/obs"; // Default path, can be changed if needed
    if !std::path::Path::new(obs_bin).exists() {
        log::debug!("Couldn't find /usr/bin/obs, using fallback OpenGL lib name.");
        return "libobs-opengl.so".to_string(); // Fallback
    }

    let raw_strings = fs::read(obs_bin).unwrap_or_default();
    let search_str = "libobs-opengl";
    let search_bytes = search_str.as_bytes();
    let found_lib_name = "libobs-opengl.so".to_string(); // Fallback

    let mut idx = 0usize;
    while idx < raw_strings.len() {
        // Find next occurrence of "libobs-opengl"
        let rel = raw_strings[idx..]
            .windows(search_bytes.len())
            .position(|w| w == search_bytes);
        let start = match rel {
            Some(r) => idx + r,
            None => break,
        };

        // Extract bytes until the next NUL (C-string terminator) or EOF
        let end = match raw_strings[start..].iter().position(|&b| b == 0) {
            Some(p) => start + p,
            None => raw_strings.len(),
        };

        if end > start {
            if let Ok(s) = std::str::from_utf8(&raw_strings[start..end]) {
                if s.contains(".so") {
                    return s.to_string();
                }
            }
        }

        // Continue search after this occurrence
        idx = start + search_bytes.len();
    }

    log::debug!("Extracted OpenGL lib name: {}", found_lib_name);
    found_lib_name
}
