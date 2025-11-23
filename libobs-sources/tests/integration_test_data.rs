use libobs_wrapper::{context::ObsContext, data::ObsDataGetters, utils::StartupInfo};

/// Integration test: Test encoder settings manipulation
#[test]
pub fn test_data_set() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let context = ObsContext::new(StartupInfo::default()).unwrap();
    let mut data = context.data().unwrap();

    // Test setting various data types
    data.set_int("test_int", 5000).unwrap();
    data.set_bool("test_bool", true).unwrap();
    data.set_string("test_str", "fast").unwrap();
    data.set_double("test_double", 1.5).unwrap();

    // Verify we can retrieve them
    let bitrate = data.get_int("test_int");
    assert_eq!(bitrate, Ok(Some(5000)));

    let psycho_aq = data.get_bool("test_bool");
    assert_eq!(psycho_aq, Ok(Some(true)));

    let preset = data.get_string("test_str");
    assert_eq!(preset, Ok(Some("fast".to_string())));

    let preset_double = data.get_double("test_double");
    assert_eq!(preset_double, Ok(Some(1.5)));
}

#[test]
pub fn test_data_get_nonexistent() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let context = ObsContext::new(StartupInfo::default()).unwrap();
    let data = context.data().unwrap();

    // Attempt to get a nonexistent key
    let nonexistent_int = data.get_int("nonexistent_key");
    assert_eq!(nonexistent_int, Ok(None));

    let nonexistent_bool = data.get_bool("nonexistent_key");
    assert_eq!(nonexistent_bool, Ok(None));

    let nonexistent_str = data.get_string("nonexistent_key");
    assert_eq!(nonexistent_str, Ok(None));

    let nonexistent_double = data.get_double("nonexistent_key");
    assert_eq!(nonexistent_double, Ok(None));
}
