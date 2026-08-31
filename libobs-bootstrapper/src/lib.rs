#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![allow(unknown_lints, require_safety_comments_on_unsafe)]

use std::{env, fs, path::PathBuf};

use cargo_obs_build::{
    ObsBuildConfig, build_obs_binaries_verified, resolve_latest_compatible_release,
};
use semver::Version;

pub mod build;
mod error;
mod options;
pub mod status_handler;
mod version;

#[cfg(test)]
mod options_tests;
#[cfg(test)]
mod version_tests;

pub use error::ObsBootstrapError;
pub use options::{DEFAULT_OBS_VERSION, GITHUB_REPO, ObsBootstrapperOptions, UpdateTargetMode};

use crate::status_handler::{ObsBootstrapConsoleHandler, ObsBootstrapStatusHandler};

/// Progress states retained for callers that model bootstrap UI separately.
pub enum BootstrapStatus {
    Downloading(f32, String),
    Extracting(f32, String),
    Error(ObsBootstrapError),
    /// Legacy state from the old PowerShell updater flow. The current
    /// bootstrapper prepares OBS before it is loaded and never emits this.
    RestartRequired,
}

/// Explicit OBS runtime provisioning and local-installation inspection.
///
/// Calling [`ObsBootstrapper::bootstrap`] is the opt-in operation that may use
/// the network. Merely depending on this crate or inspecting an installation
/// never downloads or executes anything.
pub struct ObsBootstrapper {}

#[derive(Debug)]
struct BootstrapPlan {
    install_dir: PathBuf,
    cache_dir: PathBuf,
    repository: String,
    target_version: Version,
}

fn default_install_dir() -> Result<PathBuf, ObsBootstrapError> {
    let executable =
        env::current_exe().map_err(|e| ObsBootstrapError::IoError("Getting current exe", e))?;
    executable.parent().map(PathBuf::from).ok_or_else(|| {
        ObsBootstrapError::IoError(
            "Failed to get executable parent directory",
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

fn resolve_cache_dir(options: &ObsBootstrapperOptions, install_dir: &std::path::Path) -> PathBuf {
    options
        .get_cache_dir()
        .cloned()
        .unwrap_or_else(|| install_dir.join(".libobs-bootstrap-cache"))
}

fn get_obs_library_path(install_dir: &std::path::Path) -> Result<PathBuf, ObsBootstrapError> {
    match env::consts::OS {
        "windows" => Ok(install_dir.join("obs.dll")),
        "macos" => Ok(install_dir.join("libobs.framework/Versions/A/libobs")),
        "linux" => Err(ObsBootstrapError::UnsupportedPlatform(
            "Linux uses the system/source libobs integration rather than a portable runtime"
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

#[cfg(target_os = "windows")]
fn ensure_obs_not_loaded() -> Result<(), ObsBootstrapError> {
    if libloading::os::windows::Library::open_already_loaded("obs.dll").is_ok() {
        return Err(ObsBootstrapError::RuntimeAlreadyLoaded);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn ensure_obs_not_loaded() -> Result<(), ObsBootstrapError> {
    use std::ffi::CStr;

    unsafe extern "C" {
        fn _dyld_image_count() -> u32;
        fn _dyld_get_image_name(image_index: u32) -> *const std::ffi::c_char;
    }

    // A directly linked macOS executable has libobs.framework mapped by dyld
    // before main(). Refuse to replace it in-process; a bootstrap launcher that
    // does not link libobs will not contain this image and may proceed safely.
    unsafe {
        for index in 0.._dyld_image_count() {
            let image = _dyld_get_image_name(index);
            if image.is_null() {
                continue;
            }
            let image = CStr::from_ptr(image).to_string_lossy();
            if image.contains("/libobs.framework/") {
                return Err(ObsBootstrapError::RuntimeAlreadyLoaded);
            }
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn ensure_obs_not_loaded() -> Result<(), ObsBootstrapError> {
    Ok(())
}

fn resolve_target_version(
    options: &ObsBootstrapperOptions,
    cache_dir: &std::path::Path,
) -> Result<Version, ObsBootstrapError> {
    let configured = options.get_target_version();
    let resolved_tag = match options.get_update_target_mode() {
        UpdateTargetMode::Exact => return Ok(configured.clone()),
        UpdateTargetMode::LatestCompatibleSameMajor => resolve_latest_compatible_release(
            options.get_repository(),
            configured.major as u32,
            None,
            cache_dir,
        ),
        UpdateTargetMode::LatestCompatibleSameMajorMinor => resolve_latest_compatible_release(
            options.get_repository(),
            configured.major as u32,
            Some(configured.minor as u32),
            cache_dir,
        ),
    }
    .map_err(|e| ObsBootstrapError::GeneralError(e.to_string()))?
    .ok_or_else(|| {
        ObsBootstrapError::VersionError(format!(
            "No compatible OBS release found for {} using {:?}",
            configured,
            options.get_update_target_mode()
        ))
    })?;

    let resolved = Version::parse(resolved_tag.trim_start_matches('v')).map_err(|e| {
        ObsBootstrapError::VersionError(format!("Invalid OBS release tag {resolved_tag:?}: {e}"))
    })?;
    if resolved.major != configured.major
        || matches!(
            options.get_update_target_mode(),
            UpdateTargetMode::LatestCompatibleSameMajorMinor
        ) && resolved.minor != configured.minor
    {
        return Err(ObsBootstrapError::VersionError(format!(
            "Resolved OBS release {resolved} is outside the requested compatibility line"
        )));
    }
    Ok(resolved)
}

fn installation_satisfies_policy(
    installed: &str,
    target: &Version,
    mode: UpdateTargetMode,
) -> Result<bool, ObsBootstrapError> {
    let installed_version = version::parse_version(installed)?;
    Ok(match mode {
        UpdateTargetMode::Exact => installed_version == *target,
        UpdateTargetMode::LatestCompatibleSameMajor => {
            installed_version.major == target.major && !version::should_update(installed, target)?
        }
        UpdateTargetMode::LatestCompatibleSameMajorMinor => {
            installed_version.major == target.major
                && installed_version.minor == target.minor
                && !version::should_update(installed, target)?
        }
    })
}

fn needs_provision(
    installed: Option<&str>,
    target: &Version,
    mode: UpdateTargetMode,
    update_enabled: bool,
) -> Result<bool, ObsBootstrapError> {
    let Some(installed) = installed else {
        return Ok(true);
    };
    if installation_satisfies_policy(installed, target, mode)? {
        return Ok(false);
    }
    if !update_enabled {
        return Err(ObsBootstrapError::VersionError(format!(
            "Installed OBS {installed} does not satisfy target {target} under {mode:?}, and updates are disabled"
        )));
    }
    Ok(true)
}

fn plan_bootstrap(
    options: &ObsBootstrapperOptions,
) -> Result<Option<BootstrapPlan>, ObsBootstrapError> {
    match env::consts::OS {
        "windows" | "macos" => {}
        "linux" => {
            return Err(ObsBootstrapError::UnsupportedPlatform(
                "Linux should install a compatible system libobs instead of runtime bootstrapping"
                    .to_string(),
            ));
        }
        other => return Err(ObsBootstrapError::UnsupportedPlatform(other.to_string())),
    }

    ensure_obs_not_loaded()?;
    let install_dir = resolve_install_dir(options)?;
    let cache_dir = resolve_cache_dir(options, &install_dir);
    let target_version = resolve_target_version(options, &cache_dir)?;
    let library_path = get_obs_library_path(&install_dir)?;

    let installed = match version::get_installed_version(&library_path) {
        Ok(version) => version,
        Err(error) if options.get_update() => {
            log::warn!(
                "Existing OBS runtime at {} could not be inspected ({error}); replacing it",
                library_path.display()
            );
            None
        }
        Err(error) => return Err(error),
    };

    if !needs_provision(
        installed.as_deref(),
        &target_version,
        options.get_update_target_mode(),
        options.get_update(),
    )? {
        return Ok(None);
    }

    Ok(Some(BootstrapPlan {
        install_dir,
        cache_dir,
        repository: options.get_repository().to_string(),
        target_version,
    }))
}

fn execute_bootstrap(plan: BootstrapPlan) -> Result<Version, ObsBootstrapError> {
    fs::create_dir_all(&plan.install_dir)
        .map_err(|e| ObsBootstrapError::IoError("Creating OBS install directory", e))?;
    fs::create_dir_all(&plan.cache_dir)
        .map_err(|e| ObsBootstrapError::IoError("Creating OBS cache directory", e))?;

    let config = ObsBuildConfig {
        out_dir: plan.install_dir.clone(),
        cache_dir: Some(plan.cache_dir),
        repo_id: Some(plan.repository),
        override_zip: None,
        rebuild: false,
        browser: false,
        tag: Some(plan.target_version.to_string()),
        // The bootstrapper itself owns the target version decision, and does
        // not depend on the native libobs crate or Cargo metadata at runtime.
        skip_compatibility_check: true,
        remove_pdbs: true,
    };
    build_obs_binaries_verified(config)
        .map_err(|e| ObsBootstrapError::GeneralError(e.to_string()))?;

    let library_path = get_obs_library_path(&plan.install_dir)?;
    let installed = version::get_installed_version(&library_path)?
        .ok_or_else(|| ObsBootstrapError::InvalidState)?;
    let installed = version::parse_version(&installed)?;
    if installed != plan.target_version {
        return Err(ObsBootstrapError::VersionError(format!(
            "Prepared OBS runtime is {installed}, expected exactly {}",
            plan.target_version
        )));
    }

    Ok(installed)
}

pub enum ObsBootstrapperResult {
    /// The already-installed runtime satisfied the configured requirement.
    None,
    /// A verified OBS runtime was installed or updated and the process may
    /// proceed to its first OBS call.
    Provisioned,
    /// Legacy variant from the updater-script design. The current implementation
    /// never emits this because files are prepared before OBS is loaded.
    #[deprecated(note = "runtime provisioning no longer requires an automatic restart")]
    Restart,
}

impl ObsBootstrapper {
    /// Checks the default executable-adjacent installation location without
    /// performing network I/O.
    pub fn is_valid_installation() -> Result<bool, ObsBootstrapError> {
        Self::is_valid_installation_with_options(&ObsBootstrapperOptions::default())
    }

    /// Checks the installation location selected by `options` without network I/O.
    pub fn is_valid_installation_with_options(
        options: &ObsBootstrapperOptions,
    ) -> Result<bool, ObsBootstrapError> {
        Ok(version::get_installed_version(&get_obs_library_path_with_options(options)?)?.is_some())
    }

    /// Compares the default local runtime with the configured default target.
    /// This is local-only and does not query GitHub.
    pub fn is_update_available() -> Result<bool, ObsBootstrapError> {
        Self::is_update_available_with_options(&ObsBootstrapperOptions::default())
    }

    /// Compares a local runtime with `options.target_version` without network I/O.
    pub fn is_update_available_with_options(
        options: &ObsBootstrapperOptions,
    ) -> Result<bool, ObsBootstrapError> {
        let Some(installed) =
            version::get_installed_version(&get_obs_library_path_with_options(options)?)?
        else {
            return Ok(true);
        };
        Ok(!installation_satisfies_policy(
            &installed,
            options.get_target_version(),
            options.get_update_target_mode(),
        )?)
    }

    /// Explicitly provisions OBS using the configured release policy.
    ///
    /// This is the operation that may access the network. Downloaded official
    /// release assets must advertise a SHA-256 checksum/digest and are verified
    /// before extraction. No helper process is spawned and the application is
    /// never restarted automatically.
    pub async fn bootstrap(
        options: &ObsBootstrapperOptions,
    ) -> Result<ObsBootstrapperResult, ObsBootstrapError> {
        Self::bootstrap_with_handler(options, Box::new(ObsBootstrapConsoleHandler::default())).await
    }

    /// Same as [`Self::bootstrap`], with custom progress notifications.
    pub async fn bootstrap_with_handler<E: Send + Sync + 'static + std::error::Error>(
        options: &ObsBootstrapperOptions,
        mut handler: Box<dyn ObsBootstrapStatusHandler<Error = E>>,
    ) -> Result<ObsBootstrapperResult, ObsBootstrapError> {
        let options_for_plan = options.clone();
        let plan = tokio::task::spawn_blocking(move || plan_bootstrap(&options_for_plan))
            .await
            .map_err(|e| {
                ObsBootstrapError::GeneralError(format!("Bootstrap planning task failed: {e}"))
            })??;

        let Some(plan) = plan else {
            return Ok(ObsBootstrapperResult::None);
        };

        handler
            .handle_downloading(0.0, "Preparing verified OBS release".to_string())
            .map_err(|e| ObsBootstrapError::Abort(Box::new(e)))?;
        handler
            .handle_extraction(0.0, "Staging OBS runtime".to_string())
            .map_err(|e| ObsBootstrapError::Abort(Box::new(e)))?;

        let installed = tokio::task::spawn_blocking(move || execute_bootstrap(plan))
            .await
            .map_err(|e| {
                ObsBootstrapError::GeneralError(format!("Bootstrap worker failed: {e}"))
            })??;

        handler
            .handle_downloading(1.0, format!("Verified OBS {installed}"))
            .map_err(|e| ObsBootstrapError::Abort(Box::new(e)))?;
        handler
            .handle_extraction(1.0, format!("OBS {installed} is ready"))
            .map_err(|e| ObsBootstrapError::Abort(Box::new(e)))?;

        Ok(ObsBootstrapperResult::Provisioned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provisioning_decision_respects_version_policy() {
        let target = Version::new(32, 1, 0);
        assert!(!needs_provision(Some("32.1.0"), &target, UpdateTargetMode::Exact, false).unwrap());
        assert!(needs_provision(None, &target, UpdateTargetMode::Exact, false).unwrap());
        assert!(needs_provision(Some("32.2.0"), &target, UpdateTargetMode::Exact, true).unwrap());
        assert!(
            !needs_provision(
                Some("32.2.0"),
                &target,
                UpdateTargetMode::LatestCompatibleSameMajor,
                false
            )
            .unwrap()
        );
        assert!(
            needs_provision(
                Some("32.2.0"),
                &target,
                UpdateTargetMode::LatestCompatibleSameMajorMinor,
                true
            )
            .unwrap()
        );
        assert!(needs_provision(Some("32.0.0"), &target, UpdateTargetMode::Exact, false).is_err());
        assert!(needs_provision(Some("31.9.9"), &target, UpdateTargetMode::Exact, false).is_err());
    }

    #[test]
    fn default_version_stays_in_sync_with_vendored_obs_headers() {
        let header = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../libobs/headers/obs/obs-config.h");
        if !header.is_file() {
            return;
        }
        let text = std::fs::read_to_string(header).unwrap();
        let number = |name: &str| -> u64 {
            text.lines()
                .find_map(|line| {
                    line.strip_prefix(&format!("#define {name} "))
                        .and_then(|value| value.trim().parse().ok())
                })
                .unwrap()
        };
        let expected = Version::new(
            number("LIBOBS_API_MAJOR_VER"),
            number("LIBOBS_API_MINOR_VER"),
            number("LIBOBS_API_PATCH_VER"),
        );
        assert_eq!(Version::parse(DEFAULT_OBS_VERSION).unwrap(), expected);
    }
}
