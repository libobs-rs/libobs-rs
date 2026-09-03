use libobs_wrapper::{
    context::ObsContext,
    data::video::ObsVideoInfoBuilder,
    enums::ObsOrderMovement,
    graphics::Vec2,
    scenes::{ObsSceneItemCrop, SceneItemTrait},
    unsafe_send::NativeObjectId,
    utils::{ObsError, StartupInfo},
};

fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

fn assert_close(actual: f32, expected: f32, label: &str) {
    assert!(
        (actual - expected).abs() <= 0.001,
        "{label}: expected {expected}, got {actual}"
    );
}

fn assert_half_pixel_position(actual: f32, requested: f32, label: &str) {
    // libobs stores ordinary scene positions in canvas-relative coordinates and nudges the
    // reconstructed absolute value to the half-pixel grid. Relative-coordinate floating-point
    // conversion can choose either adjacent half-pixel at an exact boundary, so assert the
    // observable contract: a half-pixel coordinate no farther than 0.5 px from the request.
    let doubled = actual * 2.0;
    assert!(
        (doubled - doubled.round()).abs() <= 0.001,
        "{label}: native position {actual} is not on the half-pixel grid"
    );
    assert!(
        (actual - requested).abs() <= 0.5001,
        "{label}: requested {requested}, native position {actual} differs by more than 0.5 px"
    );
}

fn assert_native_order<T: SceneItemTrait>(items: &[&T], expected_bottom_to_top: &[NativeObjectId]) {
    let mut actual = items
        .iter()
        .filter(|item| !item.is_removed())
        .map(|item| {
            (
                item.order_position().expect("read native order position"),
                item.object_id(),
            )
        })
        .collect::<Vec<_>>();
    actual.sort_by_key(|(position, _)| *position);
    let actual = actual.into_iter().map(|(_, id)| id).collect::<Vec<_>>();
    assert_eq!(actual, expected_bottom_to_top);
}

#[test]
fn scene_item_state_and_order_match_a_deterministic_model() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .is_test(true)
        .try_init();

    let video = ObsVideoInfoBuilder::new()
        .base_width(320)
        .base_height(180)
        .output_width(320)
        .output_height(180)
        .build();
    let mut context = ObsContext::new(StartupInfo::new().set_video_info(video))
        .expect("initialize OBS behavior-test runtime");
    let capabilities = context.capabilities().expect("discover capabilities");
    let color_type = capabilities
        .source_types()
        .iter()
        .find(|source| source.id() == "color_source_v3")
        .expect("validation OBS exposes color_source_v3");
    let source = context
        .create_source(color_type, "behavior-shared-source", None)
        .expect("create shared color source");
    let mut scene = context.scene("behavior-scene", None).expect("create scene");

    let item0 = scene.add(source.clone()).expect("add item 0");
    let item1 = scene.add(source.clone()).expect("add item 1");
    let item2 = scene.add(source.clone()).expect("add item 2");
    let item3 = scene.add(source.clone()).expect("add item 3");
    let items = [&item0, &item1, &item2, &item3];

    let id0 = item0.object_id();
    let id1 = item1.object_id();
    let id2 = item2.object_id();
    let id3 = item3.object_id();
    let mut model = vec![id0, id1, id2, id3];
    assert_native_order(&items, &model);

    item0
        .move_order(ObsOrderMovement::Top)
        .expect("move item0 top");
    model.remove(0);
    model.push(id0);
    assert_native_order(&items, &model);

    item3
        .move_order(ObsOrderMovement::Down)
        .expect("move item3 down one level");
    let pos3 = model
        .iter()
        .position(|id| *id == id3)
        .expect("item3 in model");
    model.swap(pos3, pos3 - 1);
    assert_native_order(&items, &model);

    item1
        .move_order(ObsOrderMovement::Up)
        .expect("move item1 up one level");
    let pos1 = model
        .iter()
        .position(|id| *id == id1)
        .expect("item1 in model");
    model.swap(pos1, pos1 + 1);
    assert_native_order(&items, &model);

    item2
        .set_order_position(0)
        .expect("move item2 to absolute bottom");
    model.retain(|id| *id != id2);
    model.insert(0, id2);
    assert_native_order(&items, &model);

    let before_invalid = model.clone();
    assert!(matches!(
        item1.set_order_position(-1),
        Err(ObsError::InvalidOperation(_))
    ));
    assert_native_order(&items, &before_invalid);

    // The same source can appear multiple times in one scene; each scene-item transform must
    // remain independent while the scene registry still reports all live items for the source.
    item0
        .set_position(Vec2::new(12.5, 24.25))
        .expect("set item0 position");
    item0
        .set_scale(Vec2::new(1.25, 0.75))
        .expect("set item0 scale");
    item0.set_rotation(37.0).expect("set item0 rotation");
    item0
        .set_crop(ObsSceneItemCrop::new(3, 5, 7, 11))
        .expect("set item0 crop");
    assert_eq!(
        item0.position().expect("item0 position"),
        Vec2::new(12.5, 24.5),
        "libobs quantizes ordinary scene positions to half-pixel coordinates"
    );
    assert_eq!(item0.scale().expect("item0 scale"), Vec2::new(1.25, 0.75));
    assert_close(item0.rotation().expect("item0 rotation"), 37.0, "rotation");
    assert_eq!(
        item0.crop().expect("item0 crop"),
        ObsSceneItemCrop::new(3, 5, 7, 11)
    );
    assert_eq!(
        item1.position().expect("item1 default position"),
        Vec2::new(0.0, 0.0)
    );
    assert_eq!(
        item1.crop().expect("item1 default crop"),
        ObsSceneItemCrop::default()
    );

    item1
        .set_crop(ObsSceneItemCrop::new(-8, -3, 4, 6))
        .expect("set crop containing negative edges");
    assert_eq!(
        item1.crop().expect("read clamped crop"),
        ObsSceneItemCrop::new(0, 0, 4, 6),
        "libobs clamps negative crop edges rather than storing invalid geometry"
    );
    item1
        .set_crop(ObsSceneItemCrop::default())
        .expect("restore item1 crop");

    let live = scene
        .items_for_source(&source)
        .expect("query duplicate source items");
    assert_eq!(live.len(), 4);
    assert!(live.iter().any(|item| item.object_id() == id0));
    assert!(live.iter().any(|item| item.object_id() == id3));

    // Exercise hundreds of setter/getter pairs with reproducible non-integer transforms. This
    // catches type conversion, actor serialization and stale-handle regressions without relying on
    // a few hand-picked values.
    let mut state = 0x51CE_1E57_CAFE_BABE_u64;
    for _ in 0..256 {
        let x = (next_random(&mut state) % 32_000) as f32 / 100.0 - 40.0;
        let y = (next_random(&mut state) % 18_000) as f32 / 100.0 - 25.0;
        let sx = 0.1 + (next_random(&mut state) % 400) as f32 / 100.0;
        let sy = 0.1 + (next_random(&mut state) % 400) as f32 / 100.0;
        let rotation = (next_random(&mut state) % 36_000) as f32 / 100.0;
        let crop = ObsSceneItemCrop::new(
            (next_random(&mut state) % 20) as i32,
            (next_random(&mut state) % 20) as i32,
            (next_random(&mut state) % 20) as i32,
            (next_random(&mut state) % 20) as i32,
        );

        item0
            .set_position(Vec2::new(x, y))
            .expect("random position");
        item0.set_scale(Vec2::new(sx, sy)).expect("random scale");
        item0.set_rotation(rotation).expect("random rotation");
        item0.set_crop(crop).expect("random crop");

        let actual_position = item0.position().expect("read random position");
        let actual_scale = item0.scale().expect("read random scale");
        assert_half_pixel_position(*actual_position.x(), x, "random x");
        assert_half_pixel_position(*actual_position.y(), y, "random y");
        assert_close(*actual_scale.x(), sx, "random scale x");
        assert_close(*actual_scale.y(), sy, "random scale y");
        assert_close(
            item0.rotation().expect("read random rotation"),
            rotation,
            "random rotation",
        );
        assert_eq!(item0.crop().expect("read random crop"), crop);
    }

    scene.remove_item(&item3).expect("remove item3");
    assert!(item3.is_removed());
    model.retain(|id| *id != id3);
    assert_native_order(&items, &model);
    let live = scene
        .items_for_source(&source)
        .expect("query source items after removal");
    assert_eq!(live.len(), 3);
    assert!(live.iter().all(|item| item.object_id() != id3));

    scene.clear().expect("clear behavior scene");
    assert!(item0.is_removed());
    assert!(item1.is_removed());
    assert!(item2.is_removed());
    assert!(scene
        .items_for_source(&source)
        .expect("query source items after clear")
        .is_empty());
}
