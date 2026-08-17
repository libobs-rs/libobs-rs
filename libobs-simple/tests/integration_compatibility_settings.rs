use libobs_wrapper::{
    capabilities::{CompatibilityIssue, OutputCapabilities, OutputCompatibilityRequest},
    context::ObsContext,
    data::ObsDataGetters,
    settings::{EditableListEntry, FontSetting, PropertyValue},
    utils::{ObsError, StartupInfo},
};

#[test]
fn compatibility_planner_selects_complete_runtime_available_graph() {
    let context = ObsContext::new(StartupInfo::default()).expect("initialize OBS");
    let capabilities = context.capabilities().expect("discover capabilities");

    let request = OutputCompatibilityRequest::new()
        .protocol("RTMP")
        .video_codec("h264")
        .audio_codec("aac")
        .prefer_hardware_video(true)
        .require_output_capabilities(
            OutputCapabilities::ENCODED
                | OutputCapabilities::VIDEO
                | OutputCapabilities::AUDIO
                | OutputCapabilities::SERVICE,
        );

    let plan = capabilities
        .best_output_plan(&request)
        .expect("validation OBS has a compatible RTMP graph");
    assert!(plan.output().supports_protocol("RTMP"));
    assert_eq!(
        plan.video_encoder().and_then(|encoder| encoder.codec()),
        Some("h264")
    );
    assert_eq!(
        plan.audio_encoder().and_then(|encoder| encoder.codec()),
        Some("aac")
    );

    let impossible = OutputCompatibilityRequest::new()
        .video_codec("libobs-rs-not-a-codec")
        .require_output_capabilities(OutputCapabilities::ENCODED | OutputCapabilities::VIDEO);
    let report = capabilities
        .best_output_plan(&impossible)
        .expect_err("impossible codec must produce structured diagnostics");
    assert!(report
        .issues()
        .iter()
        .any(|issue| matches!(issue, CompatibilityIssue::NoVideoEncoder { codec } if codec == "libobs-rs-not-a-codec")));
    assert!(!report.summary().is_empty());
}

#[test]
fn property_schema_validates_and_applies_typed_settings() {
    let context = ObsContext::new(StartupInfo::default()).expect("initialize OBS");
    let x264 = context
        .encoder_type("obs_x264")
        .expect("discover encoders")
        .expect("validation OBS exposes x264");
    let mut settings = x264.default_settings_mut().expect("x264 defaults");
    let schema = x264.settings_schema_for(&settings).expect("x264 schema");

    let bitrate = schema.property("bitrate").expect("x264 bitrate property");
    assert!(bitrate.enabled && bitrate.visible);
    schema
        .set(&mut settings, "bitrate", PropertyValue::Integer(6_000))
        .expect("apply validated bitrate");
    assert_eq!(
        settings.get_int("bitrate").expect("read bitrate"),
        Some(6_000)
    );
    assert_eq!(
        schema.value(&settings, "bitrate").expect("typed readback"),
        Some(PropertyValue::Integer(6_000))
    );

    let snapshot = x264
        .settings_snapshot_for(&settings)
        .expect("form-ready settings snapshot");
    let bitrate_state = snapshot.state("bitrate").expect("bitrate state");
    assert_eq!(
        bitrate_state.current_value,
        Some(PropertyValue::Integer(6_000))
    );
    assert!(bitrate_state.default_value.is_some());
    assert_eq!(bitrate_state.metadata.name, "bitrate");

    let wrong_type = schema
        .set(
            &mut settings,
            "bitrate",
            PropertyValue::String("fast".into()),
        )
        .expect_err("wrong property type must be rejected before FFI mutation");
    assert!(matches!(
        wrong_type,
        ObsError::PropertyValueTypeMismatch { .. }
    ));

    let unknown = schema
        .set(
            &mut settings,
            "libobs-rs-missing",
            PropertyValue::Boolean(true),
        )
        .expect_err("unknown properties must be diagnosed");
    assert!(matches!(unknown, ObsError::PropertyNotFound { .. }));

    if let Some(rate_control) = schema.property("rate_control") {
        if let Some(first) = rate_control.enabled_list_values().first().cloned() {
            schema
                .set(&mut settings, "rate_control", first)
                .expect("apply valid list value");
            let refreshed = x264
                .settings_schema_for(&settings)
                .expect("callbacks applied to refreshed schema");
            assert!(refreshed.property("rate_control").is_some());
        }
    }
}

#[test]
fn complex_property_values_round_trip_through_obs_data() {
    let context = ObsContext::new(StartupInfo::default()).expect("initialize OBS");

    let slideshow = context
        .source_type("slideshow")
        .expect("discover sources")
        .expect("validation OBS exposes slideshow");
    let mut slideshow_settings = slideshow
        .default_settings_mut()
        .expect("slideshow defaults");
    let slideshow_schema = slideshow
        .settings_schema_for(&slideshow_settings)
        .expect("slideshow schema");
    let entries = vec![
        EditableListEntry {
            value: "/tmp/first.png".into(),
            uuid: Some("first-entry".into()),
            selected: true,
            hidden: false,
        },
        EditableListEntry::new("/tmp/second.png"),
    ];
    slideshow_schema
        .set(
            &mut slideshow_settings,
            "files",
            PropertyValue::EditableList(entries.clone()),
        )
        .expect("set editable-list property");
    assert_eq!(
        slideshow_schema
            .value(&slideshow_settings, "files")
            .expect("read editable-list property"),
        Some(PropertyValue::EditableList(entries))
    );

    let font_source = ["text_ft2_source", "text_gdiplus"]
        .into_iter()
        .find_map(|id| context.source_type(id).ok().flatten())
        .expect("validation OBS exposes a text source");
    let mut font_settings = font_source.default_settings_mut().expect("text defaults");
    let font_schema = font_source
        .settings_schema_for(&font_settings)
        .expect("text schema");
    let font = FontSetting {
        face: "Sans".into(),
        style: "Regular".into(),
        size: 28,
        flags: 0,
    };
    font_schema
        .set(
            &mut font_settings,
            "font",
            PropertyValue::Font(font.clone()),
        )
        .expect("set font property");
    assert_eq!(
        font_schema
            .value(&font_settings, "font")
            .expect("read font property"),
        Some(PropertyValue::Font(font))
    );
}
