use std::ffi::{CStr, CString};

use crate::{
    enums::ObsLogLevel, logger::internal_log_global, run_with_obs, runtime::ObsRuntime,
    unsafe_send::Sendable, utils::StartupPaths,
};
use libobs::obs_module_failure_info;

#[derive(Debug)]
pub struct ObsModules {
    paths: StartupPaths,

    /// A pointer to the module failure info structure.
    info: Option<Sendable<obs_module_failure_info>>,
    pub(crate) runtime: Option<ObsRuntime>,
}

impl ObsModules {
    pub fn add_paths(paths: &StartupPaths) -> Self {
        unsafe {
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

            for plugin in disabled_plugins {
                let c_str = CString::new(plugin).unwrap();
                libobs::obs_add_disabled_module(c_str.as_ptr());
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
