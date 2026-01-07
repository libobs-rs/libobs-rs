mod common;

use libobs_wrapper::{context::ObsContext, utils::StartupInfo};

/// Integration test: Test creating a scene
#[test]
pub fn test_scene_creation() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let mut context = ObsContext::new(StartupInfo::default()).unwrap();

    // Create a scene
    let scene = context.scene("test_scene", Some(0));
    assert!(scene.is_ok(), "Failed to create scene");
}

/// Integration test: Test setting scene to channel
#[test]
pub fn test_scene_set_to_channel() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let mut context = ObsContext::new(StartupInfo::default()).unwrap();

    let scene = context.scene("channel_test_scene", Some(0)).unwrap();

    // Set scene to channel 0
    let result = scene.set_to_channel(0);
    assert!(result.is_ok(), "Failed to set scene to channel 0");
}

/// Integration test: Test creating multiple scenes
#[test]
pub fn test_multiple_scenes() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let mut context = ObsContext::new(StartupInfo::default()).unwrap();

    // Create multiple scenes
    let scene1 = context.scene("scene1", Some(0));
    assert!(scene1.is_ok());

    let scene2 = context.scene("scene2", Some(1));
    assert!(scene2.is_ok());

    let scene3 = context.scene("scene3", Some(2));
    assert!(scene3.is_ok());
}

/// Integration test: Test scene lifecycle
#[test]
pub fn test_scene_lifecycle() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let mut context = ObsContext::new(StartupInfo::default()).unwrap();

    {
        // Scene is created within this scope
        let _scene = context.scene("lifecycle_scene", Some(0)).unwrap();

        // Scene should be usable here
    }

    // After scope ends, scene is dropped
    // Context should still be valid
    let scene2 = context.scene("another_scene", Some(1));
    assert!(scene2.is_ok());
}

/// Integration test: Test scene with different channel numbers
#[test]
pub fn test_scene_different_channels() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let mut context = ObsContext::new(StartupInfo::default()).unwrap();

    // Test setting scenes to different channels
    let scene0 = context.scene("channel_0_scene", Some(0)).unwrap();
    assert!(scene0.set_to_channel(0).is_ok());

    let scene1 = context.scene("channel_1_scene", Some(1)).unwrap();
    assert!(scene1.set_to_channel(1).is_ok());
}
