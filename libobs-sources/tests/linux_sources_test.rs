//! Compilation tests for Linux sources

#[cfg(target_os = "linux")]
#[cfg(test)]
mod linux_tests {
    // Just test that all the source types can be imported and used
    use libobs_sources::linux::*;

    #[test]
    fn test_linux_sources_exist() {
        // Test that all the types are available and can be constructed
        // This is mainly a compilation test

        // Test enum types
        let _color_range = ObsV4L2ColorRange::Full;
        let _source_type = ObsPipeWireSourceType::DesktopCapture;

        // Test that StringEnum trait is implemented
        use libobs_wrapper::data::StringEnum;
        assert_eq!(ObsV4L2ColorRange::Full.to_str(), "Full");
        assert_eq!(ObsV4L2ColorRange::Partial.to_str(), "Partial");
        assert_eq!(ObsV4L2ColorRange::Default.to_str(), "Default");
    }

    #[test]
    fn test_v4l2_color_range_values() {
        use num_traits::ToPrimitive;

        assert_eq!(ObsV4L2ColorRange::Default.to_i64(), Some(0));
        assert_eq!(ObsV4L2ColorRange::Partial.to_i64(), Some(1));
        assert_eq!(ObsV4L2ColorRange::Full.to_i64(), Some(2));
    }
}
