use std::path::PathBuf;

use libobs_simple::{
    scene_collection::{RestoreMode, SceneCollectionDocument},
    wrapper::{
        data::ObsDataSetters,
        graphics::Vec2,
        scenes::SceneItemTrait,
        sources::ObsSourceTrait,
        utils::{ObsPath, StartupInfo, StartupPaths},
    },
};

fn test_runtime() -> Option<PathBuf> {
    std::env::var_os("LIBOBS_TEST_RUNTIME_DIR").map(PathBuf::from)
}

fn start_context() -> Option<libobs_simple::wrapper::context::ObsContext> {
    let root = test_runtime()?;
    let lib_dir = root.join("lib/x86_64-linux-gnu");
    let paths = StartupPaths::new(
        ObsPath::new(root.join("share/obs/libobs").to_string_lossy().as_ref()),
        ObsPath::new(
            lib_dir
                .join("obs-plugins/%module%")
                .to_string_lossy()
                .as_ref(),
        ),
        ObsPath::new(
            root.join("share/obs/obs-plugins/%module%")
                .to_string_lossy()
                .as_ref(),
        ),
    );
    let config = std::env::temp_dir().join(format!(
        "libobs-rs-scene-collection-config-{}",
        std::process::id()
    ));
    match StartupInfo::new()
        .set_startup_paths(paths)
        .set_module_config_path(ObsPath::new(config.to_string_lossy().as_ref()))
        .start()
    {
        Ok(context) => Some(context),
        Err(error) => panic!("prepared OBS runtime failed to initialize: {error}"),
    }
}

fn capture_fixture() -> SceneCollectionDocument {
    let mut context = start_context().expect("prepared OBS test runtime should initialize");
    let color_type = context
        .source_type("color_source_v3")
        .expect("capability query")
        .expect("image-source color source should be installed");
    let mut settings = color_type.default_settings_mut().unwrap();
    settings
        .set_int("width", 640)
        .unwrap()
        .set_int("height", 360)
        .unwrap()
        .set_int("color", 0xFF3366CC_u32 as i64)
        .unwrap();
    let source = context
        .create_source(&color_type, "Shared Color", Some(settings))
        .unwrap();

    if let Some(filter_type) = context.source_type("crop_filter").unwrap() {
        let mut filter_settings = filter_type.default_settings_mut().unwrap();
        filter_settings.set_int("left", 5).unwrap();
        let filter = context
            .create_filter(&filter_type, "Shared Crop", Some(filter_settings))
            .unwrap();
        source.apply_filter(&filter).unwrap();
    }

    let mut scene_a = context.scene("Persist A", None).unwrap();
    let first = scene_a.add(source.clone()).unwrap();
    first.set_position(Vec2::new(10.0, 20.0)).unwrap();
    first.set_locked(true).unwrap();

    let second = scene_a.add(source.clone()).unwrap();
    second.set_position(Vec2::new(300.0, 200.0)).unwrap();
    second.set_visible(false).unwrap();
    let group = scene_a.create_group("Nested Group").unwrap();
    group.add_item(&second).unwrap();
    second.set_position(Vec2::new(30.0, 40.0)).unwrap();

    let mut scene_b = context.scene("Persist B", None).unwrap();
    let third = scene_b.add(source).unwrap();
    third.set_position(Vec2::new(50.0, 60.0)).unwrap();

    let document = SceneCollectionDocument::capture(&[scene_a, scene_b]).unwrap();
    drop(context);
    document
}

#[test]
fn native_round_trip_preserves_shared_source_and_group_structure() {
    if test_runtime().is_none() {
        eprintln!("skipping: LIBOBS_TEST_RUNTIME_DIR is not configured");
        return;
    }
    let document = capture_fixture();
    assert_eq!(
        document.sources.len(),
        1,
        "shared source should serialize once"
    );
    assert_eq!(document.scenes.len(), 2);
    assert!(document.scenes[0].items.iter().any(|node| matches!(
        node,
        libobs_simple::scene_collection::PersistedSceneNode::Group(_)
    )));

    let mut context = start_context().expect("second OBS context should initialize after cleanup");
    let restored = document
        .restore(&mut context, RestoreMode::Strict)
        .expect("scene collection should restore");
    assert!(restored.diagnostics.is_empty());
    assert_eq!(restored.sources.len(), 1);
    assert_eq!(restored.scenes.len(), 2);

    let scene_a = restored.scenes.get(&document.scenes[0].id).unwrap();
    let scene_b = restored.scenes.get(&document.scenes[1].id).unwrap();
    assert_eq!(scene_a.groups().unwrap().len(), 1);
    assert_eq!(
        scene_a.groups().unwrap()[0].items_in_order().unwrap().len(),
        1
    );

    let source_a = scene_a
        .managed_sources()
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let source_b = scene_b
        .managed_sources()
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(
        source_a.object_id(),
        source_b.object_id(),
        "both restored scenes must share one managed native source"
    );
    assert_eq!(
        source_a.get_active_filters().unwrap().len(),
        document.sources[0].filters.len()
    );

    let top = scene_a.items_in_order().unwrap();
    let source_item = top
        .iter()
        .find(|item| item.source_persistent_uuid().unwrap() == source_a.persistent_uuid().unwrap())
        .expect("top-level shared source item");
    assert_eq!(source_item.position().unwrap(), Vec2::new(10.0, 20.0));
    assert!(source_item.is_locked().unwrap());
}

#[test]
fn native_restore_reports_missing_plugins_and_files() {
    if test_runtime().is_none() {
        eprintln!("skipping: LIBOBS_TEST_RUNTIME_DIR is not configured");
        return;
    }
    let mut document = capture_fixture();
    document.sources[0].plugin_id = "libobs_rs_definitely_missing_source".into();
    document.sources[0].settings["local_file"] =
        serde_json::Value::String("/libobs-rs/definitely/missing/file.mp4".into());

    let mut context = start_context().unwrap();
    assert!(document.restore(&mut context, RestoreMode::Strict).is_err());
    drop(context);

    let mut context = start_context().unwrap();
    let restored = document
        .restore(&mut context, RestoreMode::BestEffort)
        .expect("best-effort restore should keep the scene collection usable");
    assert!(restored.sources.is_empty());
    assert!(restored.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind
            == libobs_simple::scene_collection::RestoreDiagnosticKind::MissingSourcePlugin
    }));
    assert!(restored.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == libobs_simple::scene_collection::RestoreDiagnosticKind::SkippedSceneItem
    }));
}
