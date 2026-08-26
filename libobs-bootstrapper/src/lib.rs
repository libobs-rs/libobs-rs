#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![allow(unknown_lints, require_safety_comments_on_unsafe)]

use std::{env, path::PathBuf};

use libobs::{LIBOBS_API_MAJOR_VER, LIBOBS_API_MINOR_VER, LIBOBS_API_PATCH_VER};
use semver::Version;

mod error;
mod options;
pub mod status_handler;
mod version;

#[cfg(test)]
mod options_tests;
#[cfg(test)]
mod version_tests;

pub use error::ObsBootstrapError;
pub use options::{ObsBootstrapperOptions, UpdateTargetMode};

use crate::status_handler::ObsBootstrapStatusHandler;

/// Legacy progress states retained for source compatibility.
///
/// Runtime OBS installation is disabled, so current bootstrap entry points do
/// not emit these states.
pub enum BootstrapStatus {
    Downloading(f32, String),
    Extracting(f32, String),
    Error(ObsBootstrapError),
    RestartRequired,
}

/// Runtime bootstrap entry points and local-installation inspection helpers.
///
/// Network runtime installation is intentionally disabled. OBS must be
/// authenticated and packaged before process startup (for example with
/// `cargo-obs-build`, a signed installer, or the system package manager).
pub struct ObsBootstrapper {}

fn default_install_dir() -> Result<PathBuf, ObsBootstrapError> {
    let executable =
        env::current_exe().map_err(|e| ObsBootstrapError::IoError("Getting current exe", e))?;
    executable.parent().map(PathBuf::from).ok_or_else(|| {
        ObsBootstrapError::IoError(
            "Failed to get parent directory",
            std::io::Error::from(std::io::ErrorKind::InvalidInput),
        )
    })
}

fn resolve_install_dir(options: &ObsBootstrapperOptions) -> Result<PathBuf, ObsBootstrapError> {
    options
        .get_install_dir()
        .cloned()
        .map(Ok)
        .unwrap_or_else(default_install_dir)
}

fn get_obs_library_path(install_dir: &std::path::Path) -> Result<PathBuf, ObsBootstrapError> {
    match std::env::consts::OS {
        "windows" => Ok(install_dir.join("obs.dll")),
        "macos" => Ok(install_dir.join("libobs.framework/Versions/A/libobs")),
        "linux" => Err(ObsBootstrapError::UnsupportedPlatform(
            "Linux uses system/source libobs integration; inspect it with pkg-config instead"
                .to_string(),
        )),
        other => Err(ObsBootstrapError::UnsupportedPlatform(other.to_string())),
    }
}

fn get_obs_library_path_with_options(
    options: &ObsBootstrapperOptions,
) -> Result<PathBuf, ObsBootstrapError> {
    get_obs_library_path(&resolve_install_dir(options)?)
}

fn target_obs_version() -> Version {
    Version::new(
        LIBOBS_API_MAJOR_VER as u64,
        LIBOBS_API_MINOR_VER as u64,
        LIBOBS_API_PATCH_VER as u64,
    )
}

fn runtime_bootstrap_disabled(options: &ObsBootstrapperOptions) -> ObsBootstrapError {
    // Keep the migration-era option fields considered used while preserving the
    // public builder API. None of them can re-enable runtime network execution.
    let _ = (
        options.get_repository(),
        options.update,
        options.restart_after_update,
        options.update_target_mode,
        options.get_install_dir(),
    );
    ObsBootstrapError::RuntimeBootstrapDisabled
}

pub enum ObsBootstrapperResult {
    None,
    Restart,
}

impl ObsBootstrapper {
    /// Checks the default executable-adjacent installation location.
    pub fn is_valid_installation() -> Result<bool, ObsBootstrapError> {
        Self::is_valid_installation_with_options(&ObsBootstrapperOptions::default())
    }

    /// Checks the installation location selected by `options`.
    pub fn is_valid_installation_with_options(
        options: &ObsBootstrapperOptions,
    ) -> Result<bool, ObsBootstrapError> {
        Ok(version::get_installed_version(&get_obs_library_path_with_options(options)?)?.is_some())
    }

    /// Checks whether the default local installation is older than the libobs
    /// ABI version this crate was generated against.
    pub fn is_update_available() -> Result<bool, ObsBootstrapError> {
        Self::is_update_available_with_options(&ObsBootstrapperOptions::default())
    }

    /// Checks whether the installation selected by `options` is absent or older
    /// than the libobs ABI version this crate was generated against.
    ///
    /// This is a local comparison only; it never queries a release server.
    pub fn is_update_available_with_options(
        options: &ObsBootstrapperOptions,
    ) -> Result<bool, ObsBootstrapError> {
        let Some(installed) =
            version::get_installed_version(&get_obs_library_path_with_options(options)?)?
        else {
            return Ok(true);
        };

        if !version::is_compatible_major(&installed)? {
            return Ok(false);
        }

        version::should_update(&installed, &target_obs_version())
    }

    /// Runtime network installation is disabled for provenance/security reasons.
    pub async fn bootstrap(
        options: &ObsBootstrapperOptions,
    ) -> Result<ObsBootstrapperResult, ObsBootstrapError> {
        Err(runtime_bootstrap_disabled(options))
    }

    /// Runtime network installation is disabled for provenance/security reasons.
    pub async fn bootstrap_with_handler<E: Send + Sync + 'static + std::error::Error>(
        options: &ObsBootstrapperOptions,
        handler: Box<dyn ObsBootstrapStatusHandler<Error = E>>,
    ) -> Result<ObsBootstrapperResult, ObsBootstrapError> {
        let _ = handler;
        Err(runtime_bootstrap_disabled(options))
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;

    #[tokio::test]
    async fn runtime_bootstrap_is_disabled_before_io() {
        let options = ObsBootstrapperOptions::new()
            .set_repository("example.invalid/should-never-be-contacted")
            .set_install_dir("/definitely/not/a/real/obs/runtime");
        assert!(matches!(
            ObsBootstrapper::bootstrap(&options).await,
            Err(ObsBootstrapError::RuntimeBootstrapDisabled)
        ));
    }
}
