use std::path::PathBuf;

#[cfg(target_os = "macos")]
pub const GITHUB_REPO: &str = "obsproject/obs-studio";
#[cfg(not(target_os = "macos"))]
pub const GITHUB_REPO: &str = "libobs-rs/libobs-builds";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpdateTargetMode {
    #[default]
    LatestCompatibleSameMajor,
    LatestCompatibleSameMajorMinor,
}

#[derive(Debug, Clone)]
pub struct ObsBootstrapperOptions {
    pub(crate) repository: String,
    pub(crate) update: bool,
    pub(crate) restart_after_update: bool,
    pub(crate) update_target_mode: UpdateTargetMode,
    pub(crate) install_dir: Option<PathBuf>,
}

impl ObsBootstrapperOptions {
    pub fn new() -> Self {
        ObsBootstrapperOptions {
            repository: GITHUB_REPO.to_string(),
            update: true,
            restart_after_update: true,
            update_target_mode: UpdateTargetMode::LatestCompatibleSameMajor,
            install_dir: None,
        }
    }

    /// Legacy runtime-bootstrap setting retained for source compatibility.
    /// Runtime network installation is disabled, so this value is not contacted.
    pub fn set_repository(mut self, repository: &str) -> Self {
        self.repository = repository.to_string();
        self
    }

    pub fn get_repository(&self) -> &str {
        &self.repository
    }

    /// Legacy runtime-bootstrap setting retained for source compatibility.
    /// Runtime network installation is disabled, so this value does not trigger updates.
    pub fn set_update(mut self, update: bool) -> Self {
        self.update = update;
        self
    }

    /// Legacy runtime-bootstrap setting retained for source compatibility.
    /// Local version inspection does not query a release line.
    pub fn set_update_target_mode(mut self, update_target_mode: UpdateTargetMode) -> Self {
        self.update_target_mode = update_target_mode;
        self
    }

    /// Overrides the directory inspected for a pre-packaged OBS runtime.
    /// Defaults to the executable directory.
    pub fn set_install_dir<P: Into<PathBuf>>(mut self, install_dir: P) -> Self {
        self.install_dir = Some(install_dir.into());
        self
    }

    pub fn get_install_dir(&self) -> Option<&PathBuf> {
        self.install_dir.as_ref()
    }

    /// Legacy runtime-bootstrap setting retained for source compatibility.
    /// Runtime network installation/restart is disabled regardless of this value.
    pub fn set_no_restart(mut self) -> Self {
        self.restart_after_update = false;
        self
    }
}

impl Default for ObsBootstrapperOptions {
    fn default() -> Self {
        ObsBootstrapperOptions::new()
    }
}
