#[cfg(test)]
mod tests {
    use libobs::{LIBOBS_API_MAJOR_VER, LIBOBS_API_MINOR_VER, LIBOBS_API_PATCH_VER};
    use semver::Version;

    use crate::version::should_update;

    #[test]
    fn test_should_update_same_version_is_false() {
        let target = Version::new(
            LIBOBS_API_MAJOR_VER as u64,
            LIBOBS_API_MINOR_VER as u64,
            LIBOBS_API_PATCH_VER as u64,
        );
        let installed = format!(
            "{}.{}.{}",
            LIBOBS_API_MAJOR_VER, LIBOBS_API_MINOR_VER, LIBOBS_API_PATCH_VER
        );

        let result = should_update(&installed, &target).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_should_update_invalid_format_two_parts() {
        let result = should_update("30.2", &Version::new(30, 2, 2));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid version string")
        );
    }

    #[test]
    fn test_should_update_invalid_format_four_parts() {
        let result = should_update("30.2.2.1", &Version::new(30, 2, 2));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid version string")
        );
    }

    #[test]
    fn test_should_update_invalid_format_empty() {
        let result = should_update("", &Version::new(30, 2, 2));
        assert!(result.is_err());
    }

    #[test]
    fn test_should_update_invalid_major_non_numeric() {
        let result = should_update("abc.2.2", &Version::new(30, 2, 2));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid version string")
        );
    }

    #[test]
    fn test_should_update_invalid_minor_non_numeric() {
        let result = should_update("30.abc.2", &Version::new(30, 2, 2));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid version string")
        );
    }

    #[test]
    fn test_should_update_invalid_patch_non_numeric() {
        let result = should_update("30.2.abc", &Version::new(30, 2, 2));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid version string")
        );
    }

    #[test]
    fn test_should_update_with_spaces() {
        let result = should_update("30 .2.2", &Version::new(30, 2, 2));
        assert!(result.is_err());
    }

    #[test]
    fn test_should_update_with_leading_zeros() {
        let result = should_update("030.002.002", &Version::new(30, 2, 2));
        assert!(result.is_err());
    }

    #[test]
    fn test_should_update_zero_version_same_major_target() {
        let target = Version::new(LIBOBS_API_MAJOR_VER as u64, 0, 1);
        let result = should_update("0.0.0", &target);
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_should_update_very_high_version() {
        let result = should_update("999.999.999", &Version::new(30, 2, 2));
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_should_update_negative_numbers() {
        let result = should_update("-1.2.3", &Version::new(30, 2, 2));
        assert!(result.is_err());
    }

    #[test]
    fn test_should_update_special_chars() {
        let result = should_update("30!2@2", &Version::new(30, 2, 2));
        assert!(result.is_err());
    }

    #[test]
    fn test_should_update_older_compatible_version() {
        let target = Version::new(
            LIBOBS_API_MAJOR_VER as u64,
            LIBOBS_API_MINOR_VER as u64 + 1,
            0,
        );
        let installed = format!("{}.{}.{}", LIBOBS_API_MAJOR_VER, LIBOBS_API_MINOR_VER, 0);
        let result = should_update(&installed, &target).unwrap();
        assert!(result);
    }

    #[test]
    fn test_should_update_latest_compatible_version_is_false() {
        let target = Version::new(30, 4, 2);
        let result = should_update("30.4.2", &target).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_should_update_major_mismatch_is_false() {
        let target = Version::new(30, 4, 2);
        let result = should_update("31.0.0", &target).unwrap();
        assert!(!result);
    }
}
