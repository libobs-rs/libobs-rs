use std::path::PathBuf;

use semver::Version;

/// OBS ABI version that this release of libobs-rs is built against.
///
/// This crate intentionally does not depend on the native `libobs` crate: a
/// bootstrap helper must be able to start before `obs.dll`/`libobs.framework`
/// exists. Keep this value synchronized with `libobs/headers/obs/obs-config.h`.
pub const DEFAULT_OBS_VERSION: &str = "32.1.0";
pub const GITHUB_REPO: &str = "obsproject/obs-studio";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpdateTargetMode {
    /// Provision the exact OBS version configured in `target_version`.
    #[default]
    Exact,
    /// Resolve the newest stable release with the same OBS major version.
    LatestCompatibleSameMajor,
    /// Resolve the newest stable patch release with the same major/minor.
    LatestCompatibleSameMajorMinor,
}

#[derive(Debug, Clone)]
pub struct ObsBootstrapperOptions {
    pub(crate) repository: String,
    pub(crate) update: bool,
    pub(crate) update_target_mode: UpdateTargetMode,
    pub(crate) install_dir: Option<PathBuf>,
    pub(crate) cache_dir: Option<PathBuf>,
    pub(crate) target_version: Version,
}

impl ObsBootstrapperOptions {
    pub fn new() -> Self {
        Self {
            repository: GITHUB_REPO.to_string(),
            update: true,
            update_target_mode: UpdateTargetMode::Exact,
            install_dir: None,
            cache_dir: None,
            target_version: Version::parse(DEFAULT_OBS_VERSION)
                .expect("DEFAULT_OBS_VERSION must be valid semver"),
        }
    }

    /// Selects the GitHub repository used for runtime provisioning.
    ///
    /// The default is the official `obsproject/obs-studio` repository. Changing
    /// this is an explicit trust decision by the caller.
    pub fn set_repository(mut self, repository: &str) -> Self {
        self.repository = repository.to_string();
        self
    }

    pub fn get_repository(&self) -> &str {
        &self.repository
    }

    /// Controls whether an existing older/incompatible runtime may be replaced.
    /// A missing runtime is still installed even when this is `false`.
    pub fn set_update(mut self, update: bool) -> Self {
        self.update = update;
        self
    }

    pub fn get_update(&self) -> bool {
        self.update
    }

    pub fn set_update_target_mode(mut self, update_target_mode: UpdateTargetMode) -> Self {
        self.update_target_mode = update_target_mode;
        self
    }

    pub fn get_update_target_mode(&self) -> UpdateTargetMode {
        self.update_target_mode
    }

    /// Overrides the directory where the prepared OBS runtime is installed.
    /// Defaults to the executable directory.
    pub fn set_install_dir<P: Into<PathBuf>>(mut self, install_dir: P) -> Self {
        self.install_dir = Some(install_dir.into());
        self
    }

    pub fn get_install_dir(&self) -> Option<&PathBuf> {
        self.install_dir.as_ref()
    }

    /// Overrides the cache used for downloaded/extracted OBS release assets.
    pub fn set_cache_dir<P: Into<PathBuf>>(mut self, cache_dir: P) -> Self {
        self.cache_dir = Some(cache_dir.into());
        self
    }

    pub fn get_cache_dir(&self) -> Option<&PathBuf> {
        self.cache_dir.as_ref()
    }

    /// Overrides the OBS version this application is built against.
    pub fn set_target_version(mut self, version: Version) -> Self {
        self.target_version = version;
        self
    }

    pub fn get_target_version(&self) -> &Version {
        &self.target_version
    }

    /// Compatibility no-op retained from the old updater-script design.
    /// Runtime provisioning no longer spawns or restarts the application.
    #[deprecated(note = "runtime bootstrap no longer restarts the application")]
    pub fn set_no_restart(self) -> Self {
        self
    }
}

impl Default for ObsBootstrapperOptions {
    fn default() -> Self {
        Self::new()
    }
}
