#[cfg(test)]
mod tests {
    use libobs::LIBOBS_API_MAJOR_VER;

    use crate::{
        download::select_latest_compatible_release, github_types::Root2, options::UpdateTargetMode,
    };

    fn release(tag_name: &str, draft: bool, prerelease: bool) -> Root2 {
        Root2 {
            tag_name: tag_name.to_string(),
            draft,
            prerelease,
            ..Default::default()
        }
    }

    #[test]
    fn test_select_latest_compatible_release_same_major_picks_highest_minor_patch() {
        let major = LIBOBS_API_MAJOR_VER;
        let releases = vec![
            release(&format!("obs-build-{}.1.5", major), false, false),
            release(&format!("obs-build-{}.9.1", major), false, false),
            release(&format!("obs-build-{}.7.10", major), false, false),
        ];

        let (_, version) = select_latest_compatible_release(
            &releases,
            UpdateTargetMode::LatestCompatibleSameMajor,
        )
        .unwrap();

        assert_eq!(version.major, major as u64);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 1);
    }

    #[test]
    fn test_select_latest_compatible_release_same_major_minor_filters_minor() {
        let major = LIBOBS_API_MAJOR_VER;
        let target_minor = libobs::LIBOBS_API_MINOR_VER;

        let releases = vec![
            release(
                &format!("obs-build-{}.{}.1", major, target_minor),
                false,
                false,
            ),
            release(
                &format!("obs-build-{}.{}.9", major, target_minor),
                false,
                false,
            ),
            release(&format!("obs-build-{}.99.99", major), false, false),
        ];

        let (_, version) = select_latest_compatible_release(
            &releases,
            UpdateTargetMode::LatestCompatibleSameMajorMinor,
        )
        .unwrap();

        assert_eq!(version.major, major as u64);
        assert_eq!(version.minor, target_minor as u64);
        assert_eq!(version.patch, 9);
    }

    #[test]
    fn test_select_latest_compatible_release_ignores_drafts_and_prereleases() {
        let major = LIBOBS_API_MAJOR_VER;
        let releases = vec![
            release(&format!("obs-build-{}.8.1", major), true, false),
            release(&format!("obs-build-{}.9.1", major), false, true),
            release(&format!("obs-build-{}.7.5", major), false, false),
        ];

        let (_, version) = select_latest_compatible_release(
            &releases,
            UpdateTargetMode::LatestCompatibleSameMajor,
        )
        .unwrap();

        assert_eq!(version.major, major as u64);
        assert_eq!(version.minor, 7);
        assert_eq!(version.patch, 5);
    }

    #[test]
    fn test_select_latest_compatible_release_errors_when_none_match() {
        let incompatible_major = LIBOBS_API_MAJOR_VER as u64 + 1;
        let releases = vec![release(
            &format!("obs-build-{}.0.0", incompatible_major),
            false,
            false,
        )];

        let result = select_latest_compatible_release(
            &releases,
            UpdateTargetMode::LatestCompatibleSameMajor,
        );

        assert!(result.is_err());
    }
}
