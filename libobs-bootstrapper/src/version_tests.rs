#[cfg(test)]
mod tests {
    use semver::Version;

    use crate::{version::should_update, DEFAULT_OBS_VERSION};

    #[test]
    fn same_version_is_not_an_update() {
        let target = Version::parse(DEFAULT_OBS_VERSION).unwrap();
        assert!(!should_update(DEFAULT_OBS_VERSION, &target).unwrap());
    }

    #[test]
    fn older_compatible_version_updates() {
        let target = Version::new(32, 1, 5);
        assert!(should_update("32.1.0", &target).unwrap());
    }

    #[test]
    fn newer_same_major_version_does_not_downgrade() {
        let target = Version::new(32, 1, 0);
        assert!(!should_update("32.2.0", &target).unwrap());
    }

    #[test]
    fn incompatible_major_is_replaced() {
        let target = Version::new(32, 1, 0);
        assert!(should_update("31.9.9", &target).unwrap());
        assert!(should_update("33.0.0", &target).unwrap());
    }

    #[test]
    fn malformed_versions_are_rejected() {
        for invalid in ["", "30.2", "30.2.2.1", "abc.2.2", "30 .2.2", "-1.2.3"] {
            assert!(should_update(invalid, &Version::new(32, 1, 0)).is_err());
        }
    }
}
