//! Startup paths for the prepared OBS runtime named by `LIBOBS_TEST_RUNTIME_DIR`.
//!
//! The runtime layout differs per platform:
//! - Windows (`cargo obs-build`): `data/libobs`, `obs-plugins/64bit`, `data/obs-plugins/%module%`
//! - macOS (`cargo obs-build`): `data/libobs`, `obs-plugins/%module%.plugin/Contents/MacOS`,
//!   `data/obs-plugins/%module%`
//! - Linux (CMake install prefix): `share/obs/libobs`, `lib[/<multiarch>]/obs-plugins/%module%`,
//!   `share/obs/obs-plugins/%module%`
//!
//! Include with `#[path = "support/prepared_runtime.rs"] mod prepared_runtime;`.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use libobs_wrapper::utils::{ObsPath, StartupPaths};

/// The prepared runtime root, if the test environment configured one.
pub fn runtime_dir() -> Option<PathBuf> {
    std::env::var_os("LIBOBS_TEST_RUNTIME_DIR").map(PathBuf::from)
}

fn obs_path(path: PathBuf) -> ObsPath {
    ObsPath::new(path.to_string_lossy().as_ref())
}

#[cfg(target_os = "windows")]
pub fn startup_paths(root: &Path) -> StartupPaths {
    StartupPaths::new(
        obs_path(root.join("data/libobs")),
        obs_path(root.join("obs-plugins/64bit")),
        obs_path(root.join("data/obs-plugins/%module%")),
    )
}

#[cfg(target_os = "macos")]
pub fn startup_paths(root: &Path) -> StartupPaths {
    StartupPaths::new(
        obs_path(root.join("data/libobs")),
        obs_path(root.join("obs-plugins/%module%.plugin/Contents/MacOS")),
        obs_path(root.join("data/obs-plugins/%module%")),
    )
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn startup_paths(root: &Path) -> StartupPaths {
    let multiarch = match std::env::consts::ARCH {
        "x86_64" => "x86_64-linux-gnu",
        "aarch64" => "aarch64-linux-gnu",
        "arm" => "arm-linux-gnueabihf",
        _ => "",
    };
    let plugin_dir = [
        root.join("lib").join(multiarch).join("obs-plugins"),
        root.join("lib/obs-plugins"),
    ]
    .into_iter()
    .find(|path| path.is_dir())
    .expect("prepared OBS runtime should contain an obs-plugins directory");
    StartupPaths::new(
        obs_path(root.join("share/obs/libobs")),
        obs_path(plugin_dir.join("%module%")),
        obs_path(root.join("share/obs/obs-plugins/%module%")),
    )
}
