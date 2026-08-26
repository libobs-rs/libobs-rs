#[cfg(test)]
mod tests {
    use crate::{
        ObsBootstrapperOptions,
        options::{GITHUB_REPO, UpdateTargetMode},
    };

    #[test]
    fn test_default_options() {
        let options = ObsBootstrapperOptions::new();
        assert_eq!(options.get_repository(), GITHUB_REPO);
        assert!(options.update);
        assert!(options.restart_after_update);
        assert_eq!(
            options.update_target_mode,
            UpdateTargetMode::LatestCompatibleSameMajor
        );
    }

    #[test]
    fn test_set_repository() {
        let options = ObsBootstrapperOptions::new().set_repository("custom/repo");
        assert_eq!(options.get_repository(), "custom/repo");
    }

    #[test]
    fn test_set_update_true() {
        let options = ObsBootstrapperOptions::new().set_update(true);
        assert!(options.update);
    }

    #[test]
    fn test_set_update_false() {
        let options = ObsBootstrapperOptions::new().set_update(false);
        assert!(!options.update);
    }

    #[test]
    fn test_set_no_restart() {
        let options = ObsBootstrapperOptions::new().set_no_restart();
        assert!(!options.restart_after_update);
    }

    #[test]
    fn test_chaining() {
        let options = ObsBootstrapperOptions::new()
            .set_repository("test/repo")
            .set_update(false)
            .set_update_target_mode(UpdateTargetMode::LatestCompatibleSameMajorMinor)
            .set_no_restart();

        assert_eq!(options.get_repository(), "test/repo");
        assert!(!options.update);
        assert!(!options.restart_after_update);
        assert_eq!(
            options.update_target_mode,
            UpdateTargetMode::LatestCompatibleSameMajorMinor
        );
    }

    #[test]
    fn test_default_trait() {
        let options = ObsBootstrapperOptions::default();
        assert_eq!(options.get_repository(), GITHUB_REPO);
        assert!(options.update);
        assert!(options.restart_after_update);
        assert_eq!(
            options.update_target_mode,
            UpdateTargetMode::LatestCompatibleSameMajor
        );
    }

    #[test]
    fn test_set_update_target_mode() {
        let options = ObsBootstrapperOptions::new()
            .set_update_target_mode(UpdateTargetMode::LatestCompatibleSameMajorMinor);
        assert_eq!(
            options.update_target_mode,
            UpdateTargetMode::LatestCompatibleSameMajorMinor
        );
    }

    #[test]
    fn test_set_install_dir() {
        let options = ObsBootstrapperOptions::new().set_install_dir("custom-obs");
        assert_eq!(
            options.get_install_dir().map(|p| p.as_path()),
            Some(std::path::Path::new("custom-obs"))
        );
    }

    #[test]
    fn test_clone() {
        let options1 = ObsBootstrapperOptions::new().set_repository("test/repo");
        let options2 = options1.clone();
        assert_eq!(options1.get_repository(), options2.get_repository());
    }

    #[test]
    fn test_debug() {
        let options = ObsBootstrapperOptions::new();
        let debug_str = format!("{:?}", options);
        assert!(debug_str.contains("ObsBootstrapperOptions"));
    }
}
