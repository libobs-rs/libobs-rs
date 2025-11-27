use std::{
    ffi::{CStr, CString},
    fmt::Debug,
};

use crate::{
    context::ObsContext, enums::ObsLogLevel, logger::internal_log_global, run_with_obs,
    runtime::ObsRuntime, unsafe_send::Sendable, utils::StartupPaths,
};
use libobs::obs_module_failure_info;

pub struct ObsModules {
    paths: StartupPaths,

    /// A pointer to the module failure info structure.
    info: Option<Sendable<obs_module_failure_info>>,
    pub(crate) runtime: Option<ObsRuntime>,
}

impl Debug for ObsModules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObsModules")
            .field("paths", &self.paths)
            .field("info", &"(internal obs_module_failure_info)")
            .finish()
    }
}

// List of all modules, this is for compatibility for obs versions below 32.0.0
static SAFE_MODULES: &str = "decklink|image-source|linux-alsa|linux-capture|linux-pipewire|linux-pulseaudio|linux-v4l2|obs-ffmpeg|obs-filters|obs-nvenc|obs-outputs|obs-qsv11|obs-transitions|obs-vst|obs-websocket|obs-x264|rtmp-services|text-freetype2|vlc-video|decklink-captions|decklink-output-ui|obslua|obspython|frontend-tools";

impl ObsModules {
    pub fn add_paths(paths: &StartupPaths) -> Self {
        unsafe {
            internal_log_global(
                ObsLogLevel::Info,
                "[libobs-wrapper]: Adding module paths:".to_string(),
            );
            internal_log_global(
                ObsLogLevel::Info,
                format!(
                    "[libobs-wrapper]:   libobs data path: {}",
                    paths.libobs_data_path()
                ),
            );
            internal_log_global(
                ObsLogLevel::Info,
                format!(
                    "[libobs-wrapper]:   plugin bin path: {}",
                    paths.plugin_bin_path()
                ),
            );
            internal_log_global(
                ObsLogLevel::Info,
                format!(
                    "[libobs-wrapper]:   plugin data path: {}",
                    paths.plugin_data_path()
                ),
            );

            libobs::obs_add_data_path(paths.libobs_data_path().as_ptr().0);
            libobs::obs_add_module_path(
                paths.plugin_bin_path().as_ptr().0,
                paths.plugin_data_path().as_ptr().0,
            );

            #[allow(unused_mut)]
            let mut disabled_plugins = vec!["obs-websocket", "frontend-tools"];

            #[cfg(feature = "__test_environment")]
            {
                disabled_plugins.extend(&["decklink-output-ui", "decklink-captions", "decklink"]);
            }

            let version = ObsContext::get_version_global().unwrap_or_default();
            let version_parts: Vec<&str> = version.split('.').collect();
            let major = version_parts
                .first()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);

            // Check if obs_add_disabled_module exists at runtime
            #[cfg(target_os = "linux")]
            let has_disabled_module_fn = {
                // Try to find symbol in already loaded libraries
                let symbol_name = CString::new("obs_add_disabled_module").unwrap();
                let sym = libc::dlsym(libc::RTLD_DEFAULT, symbol_name.as_ptr());
                let found = !sym.is_null();

                if !found && major >= 32 {
                    log::warn!("OBS version >= 32 but obs_add_disabled_module symbol not found, falling back to safe modules");
                }

                found
            };
            #[cfg(not(target_os = "linux"))]
            let has_disabled_module_fn = major >= 32;

            if major >= 32 && has_disabled_module_fn {
                for plugin in disabled_plugins {
                    let c_str = CString::new(plugin).unwrap();
                    #[cfg(target_os = "linux")]
                    {
                        let symbol_name = CString::new("obs_add_disabled_module").unwrap();
                        let func = libc::dlsym(libc::RTLD_DEFAULT, symbol_name.as_ptr());
                        if !func.is_null() {
                            let add_disabled: extern "C" fn(*const std::os::raw::c_char) =
                                std::mem::transmute(func);
                            add_disabled(c_str.as_ptr());
                        }
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        libobs::obs_add_disabled_module(c_str.as_ptr());
                    }
                }
            } else {
                for plugin in SAFE_MODULES.split('|') {
                    if disabled_plugins.contains(&plugin) {
                        continue;
                    }
                    let c_str = CString::new(plugin).unwrap();
                    libobs::obs_add_safe_module(c_str.as_ptr());
                }
            }
        }

        Self {
            paths: paths.clone(),
            info: None,
            runtime: None,
        }
    }

    pub fn load_modules(&mut self) {
        unsafe {
            let mut failure_info: obs_module_failure_info = std::mem::zeroed();
            internal_log_global(
                ObsLogLevel::Info,
                "---------------------------------".to_string(),
            );
            libobs::obs_load_all_modules2(&mut failure_info);
            internal_log_global(
                ObsLogLevel::Info,
                "---------------------------------".to_string(),
            );
            libobs::obs_log_loaded_modules();
            internal_log_global(
                ObsLogLevel::Info,
                "---------------------------------".to_string(),
            );
            libobs::obs_post_load_modules();
            self.info = Some(Sendable(failure_info));
        }

        self.log_if_failed();
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn log_if_failed(&self) {
        if self.info.as_ref().is_none_or(|x| x.0.count == 0) {
            return;
        }

        let info = &self.info.as_ref().unwrap().0;
        let mut failed_modules = Vec::new();
        for i in 0..info.count {
            let module = unsafe { info.failed_modules.add(i) };
            let plugin_name = unsafe { CStr::from_ptr(*module) };
            failed_modules.push(plugin_name.to_string_lossy());
        }

        internal_log_global(
            ObsLogLevel::Warning,
            format!("Failed to load modules: {}", failed_modules.join(", ")),
        );
    }
}

impl Drop for ObsModules {
    fn drop(&mut self) {
        log::trace!("Dropping ObsModules and removing module paths...");

        let paths = self.paths.clone();
        let runtime = self.runtime.take().unwrap();

        #[cfg(any(
            not(feature = "no_blocking_drops"),
            test,
            feature = "__test_environment"
        ))]
        {
            let r = run_with_obs!(runtime, move || unsafe {
                libobs::obs_remove_data_path(paths.libobs_data_path().as_ptr().0);
            });

            if std::thread::panicking() {
                return;
            }

            r.unwrap();
        }

        #[cfg(all(
            feature = "no_blocking_drops",
            not(test),
            not(feature = "__test_environment")
        ))]
        {
            let _ = tokio::task::spawn_blocking(move || {
                run_with_obs!(runtime, move || unsafe {
                    libobs::obs_remove_data_path(paths.libobs_data_path().as_ptr().0);
                })
                .unwrap();
            });
        }
    }
}
