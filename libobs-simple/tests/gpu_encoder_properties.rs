#![allow(deprecated)]

mod common;

use libobs_wrapper::{
    context::ObsContext,
    data::properties::ObsPropertyObject,
    encoders::{ObsContextEncoders, ObsVideoEncoderType},
    utils::StartupInfo,
};

/// Integration test: Test accessing available video encoders
#[test]
pub fn test_available_video_encoders() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let context = ObsContext::new(StartupInfo::default()).unwrap();

    let encoders = context.available_video_encoders().unwrap();

    // We should have at least some encoders available
    assert!(!encoders.is_empty(), "No video encoders available");

    // Each encoder should have a valid ID
    for encoder in &encoders {
        let id = encoder.get_encoder_id();
        assert!(!format!("{:?}", id).is_empty());
    }
}

/// Integration test: Test creating and accessing encoder properties
#[test]
pub fn test_encoder_properties_access() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let context = ObsContext::new(StartupInfo::default()).unwrap();

    let encoders = context.available_video_encoders().unwrap();

    // Skip if no encoders available
    if encoders.is_empty() {
        eprintln!("Skipping test - no video encoders available");
        return;
    }

    let encoder = encoders.into_iter().next().unwrap();

    // Should be able to get properties
    let props = encoder.get_properties();
    assert!(props.is_ok(), "Failed to get encoder properties");
}

/// Integration test: Test encoder type identification
#[test]
pub fn test_encoder_type_identification() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let context = ObsContext::new(StartupInfo::default()).unwrap();

    let encoders = context.available_video_encoders().unwrap();

    for encoder in &encoders {
        let encoder_type = encoder.get_encoder_id();

        // Test that we can check for specific encoder types
        let _is_amf = matches!(
            encoder_type,
            ObsVideoEncoderType::OBS_NVENC_H264_TEX | ObsVideoEncoderType::H264_TEXTURE_AMF
        );

        let _is_nvenc = matches!(
            encoder_type,
            ObsVideoEncoderType::OBS_NVENC_H264_TEX | ObsVideoEncoderType::H264_TEXTURE_AMF
        );
    }
}
