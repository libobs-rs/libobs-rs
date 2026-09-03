#[cfg(target_os = "windows")]
use std::path::PathBuf;

use env_logger::Env;
use libobs_simple::output::simple::ObsContextSimpleExt;
use libobs_wrapper::{
    context::ObsContext,
    data::output::ObsOutputRef,
    utils::{ObsPath, StartupInfo},
};

#[cfg(target_os = "windows")]
use libobs_wrapper::utils::StartupPaths;

#[cfg(target_os = "windows")]
fn startup_info_for_test_runtime() -> StartupInfo {
    let runtime_dir = std::env::var_os("LIBOBS_TEST_RUNTIME_DIR")
        .map(PathBuf::from)
        .expect("LIBOBS_TEST_RUNTIME_DIR must point to the prepared OBS runtime");
    let paths = StartupPaths::new(
        ObsPath::new(runtime_dir.join("data/libobs").to_string_lossy().as_ref()),
        ObsPath::new(
            runtime_dir
                .join("obs-plugins/64bit")
                .to_string_lossy()
                .as_ref(),
        ),
        ObsPath::new(
            runtime_dir
                .join("data/obs-plugins/%module%")
                .to_string_lossy()
                .as_ref(),
        ),
    );

    StartupInfo::new().set_startup_paths(paths)
}

#[cfg(not(target_os = "windows"))]
fn startup_info_for_test_runtime() -> StartupInfo {
    StartupInfo::default()
}

/// The string returned is the name of the obs output
#[allow(dead_code)]
pub fn initialize_obs<T: Into<ObsPath> + Send + Sync>(rec_file: T) -> (ObsContext, ObsOutputRef) {
    let _ = env_logger::Builder::from_env(Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    #[allow(unused_mut)]
    let mut context = ObsContext::new(startup_info_for_test_runtime()).unwrap();

    let rec_file: ObsPath = rec_file.into();
    let output = context
        .simple_output_builder("test_obs_output", rec_file)
        .build()
        .unwrap();

    (context, output)
}
