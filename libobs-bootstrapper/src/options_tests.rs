#[cfg(test)]
mod tests {
    use semver::Version;

    use crate::{ObsBootstrapperOptions, UpdateTargetMode, DEFAULT_OBS_VERSION, GITHUB_REPO};

    #[test]
    fn default_options_are_pinned_and_explicit() {
        let options = ObsBootstrapperOptions::new();
        assert_eq!(options.get_repository(), GITHUB_REPO);
        assert!(options.get_update());
        assert_eq!(options.get_update_target_mode(), UpdateTargetMode::Exact);
        assert_eq!(
            options.get_target_version(),
            &Version::parse(DEFAULT_OBS_VERSION).unwrap()
        );
        assert!(options.get_install_dir().is_none());
        assert!(options.get_cache_dir().is_none());
    }

    #[test]
    fn repository_is_configurable() {
        let options = ObsBootstrapperOptions::new().set_repository("custom/repo");
        assert_eq!(options.get_repository(), "custom/repo");
    }

    #[test]
    fn update_policy_is_configurable() {
        let options = ObsBootstrapperOptions::new()
            .set_update(false)
            .set_update_target_mode(UpdateTargetMode::LatestCompatibleSameMajorMinor);
        assert!(!options.get_update());
        assert_eq!(
            options.get_update_target_mode(),
            UpdateTargetMode::LatestCompatibleSameMajorMinor
        );
    }

    #[test]
    fn install_and_cache_directories_are_configurable() {
        let options = ObsBootstrapperOptions::new()
            .set_install_dir("custom-obs")
            .set_cache_dir("custom-cache");
        assert_eq!(
            options.get_install_dir().map(|p| p.as_path()),
            Some(std::path::Path::new("custom-obs"))
        );
        assert_eq!(
            options.get_cache_dir().map(|p| p.as_path()),
            Some(std::path::Path::new("custom-cache"))
        );
    }

    #[test]
    fn target_version_is_configurable() {
        let version = Version::new(32, 1, 7);
        let options = ObsBootstrapperOptions::new().set_target_version(version.clone());
        assert_eq!(options.get_target_version(), &version);
    }

    #[test]
    fn clone_preserves_configuration() {
        let options = ObsBootstrapperOptions::new()
            .set_repository("test/repo")
            .set_install_dir("runtime");
        let cloned = options.clone();
        assert_eq!(options.get_repository(), cloned.get_repository());
        assert_eq!(options.get_install_dir(), cloned.get_install_dir());
    }
}
