mod common;

use libobs_wrapper::{
    context::ObsContext,
    data::ObsDataSetters,
    utils::{ObsString, OutputInfo, StartupInfo},
};

/// Integration test: Test output creation
#[test]
pub fn test_output_creation() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let mut context = ObsContext::new(StartupInfo::default()).unwrap();

    let mut output_settings = context.data().unwrap();
    output_settings
        .set_string("path", ObsString::new("test_output.mp4"))
        .unwrap();

    let output_info = OutputInfo::new("ffmpeg_muxer", "test_output", Some(output_settings), None);

    let output = context.output(output_info);
    assert!(output.is_ok(), "Failed to create output");
}

/// Integration test: Test multiple output creation
#[test]
pub fn test_multiple_outputs() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let mut context = ObsContext::new(StartupInfo::default()).unwrap();

    // Create first output
    let mut settings1 = context.data().unwrap();
    settings1
        .set_string("path", ObsString::new("output1.mp4"))
        .unwrap();
    let info1 = OutputInfo::new("ffmpeg_muxer", "output1", Some(settings1), None);
    let output1 = context.output(info1);
    assert!(output1.is_ok());

    // Create second output
    let mut settings2 = context.data().unwrap();
    settings2
        .set_string("path", ObsString::new("output2.mp4"))
        .unwrap();
    let info2 = OutputInfo::new("ffmpeg_muxer", "output2", Some(settings2), None);
    let output2 = context.output(info2);
    assert!(output2.is_ok());
}

/// Integration test: Test output lifecycle
#[test]
pub fn test_output_lifecycle() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let mut context = ObsContext::new(StartupInfo::default()).unwrap();

    {
        let mut settings = context.data().unwrap();
        settings
            .set_string("path", ObsString::new("lifecycle_test.mp4"))
            .unwrap();
        let info = OutputInfo::new("ffmpeg_muxer", "lifecycle_output", Some(settings), None);

        let _output = context.output(info).unwrap();
        // Output is used within this scope
    }

    // After scope, output is dropped
    // Context should still be usable
    let scene = context.scene("test_scene", Some(0));
    assert!(scene.is_ok());
}

/// Integration test: Test output with different settings
#[test]
pub fn test_output_different_settings() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let mut context = ObsContext::new(StartupInfo::default()).unwrap();

    // Test with minimal settings
    let mut settings1 = context.data().unwrap();
    settings1
        .set_string("path", ObsString::new("minimal.mp4"))
        .unwrap();
    let info1 = OutputInfo::new("ffmpeg_muxer", "minimal_output", Some(settings1), None);
    assert!(context.output(info1).is_ok());

    // Test with additional settings
    let mut settings2 = context.data().unwrap();
    settings2
        .set_string("path", ObsString::new("configured.mp4"))
        .unwrap();
    settings2
        .set_string("format_name", ObsString::new("mp4"))
        .unwrap();
    let info2 = OutputInfo::new("ffmpeg_muxer", "configured_output", Some(settings2), None);
    assert!(context.output(info2).is_ok());
}
