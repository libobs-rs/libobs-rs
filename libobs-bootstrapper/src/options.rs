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
}

impl ObsBootstrapperOptions {
    pub fn new() -> Self {
        ObsBootstrapperOptions {
            repository: GITHUB_REPO.to_string(),
            update: true,
            restart_after_update: true,
            update_target_mode: UpdateTargetMode::LatestCompatibleSameMajor,
        }
    }

    pub fn set_repository(mut self, repository: &str) -> Self {
        self.repository = repository.to_string();
        self
    }

    pub fn get_repository(&self) -> &str {
        &self.repository
    }

    /// `true` if the updater should check for updates and download them if available.
    /// `false` if the updater should not check for updates and only install OBS if required.
    pub fn set_update(mut self, update: bool) -> Self {
        self.update = update;
        self
    }

    /// Controls which compatible release line is considered when checking for updates.
    pub fn set_update_target_mode(mut self, update_target_mode: UpdateTargetMode) -> Self {
        self.update_target_mode = update_target_mode;
        self
    }

    /// Disables the automatic restart of the application after the update is applied.
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
