#[path = "common/render_probe.rs"]
mod render_probe;

use libobs_wrapper::{
    capabilities::SourceTypeInfo,
    context::ObsContext,
    data::{video::ObsVideoInfoBuilder, ObsDataSetters},
    graphics::Vec2,
    scenes::{ObsSceneItemCrop, SceneItemTrait},
    sources::ObsSourceRef,
    utils::StartupInfo,
};
use render_probe::capture_program;

const WIDTH: u32 = 160;
const HEIGHT: u32 = 120;
const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const TOLERANCE: u8 = 3;

fn rgba_setting([r, g, b, a]: [u8; 4]) -> i64 {
    u32::from_le_bytes([r, g, b, a]) as i64
}

fn color_source(
    context: &ObsContext,
    ty: &SourceTypeInfo,
    name: &str,
    width: u32,
    height: u32,
    rgba: [u8; 4],
) -> ObsSourceRef {
    let mut settings = ty.default_settings_mut().expect("source defaults");
    settings
        .set_int("width", i64::from(width))
        .expect("width")
        .set_int("height", i64::from(height))
        .expect("height")
        .set_int("color", rgba_setting(rgba))
        .expect("color");
    context
        .create_source(ty, name, Some(settings))
        .expect("create color source")
}

#[test]
fn scene_snapshots_groups_and_native_order_round_trip_through_libobs() {
    let video = ObsVideoInfoBuilder::new()
        .base_width(WIDTH)
        .base_height(HEIGHT)
        .output_width(WIDTH)
        .output_height(HEIGHT)
        .build();
    let mut context =
        ObsContext::new(StartupInfo::new().set_video_info(video)).expect("initialize OBS");
    let capabilities = context.capabilities().expect("discover capabilities");
    let color_type = capabilities
        .source_types()
        .iter()
        .find(|source| source.id() == "color_source_v3")
        .expect("color source available");

    let mut scene = context.scene("group-scene", None).expect("scene");
    scene.set_to_channel(0).expect("program scene");
    let red = color_source(&context, color_type, "group-red", 30, 20, RED);
    let green = color_source(&context, color_type, "group-green", 25, 25, GREEN);
    let red_item = scene.add(red).expect("red item");
    let green_item = scene.add(green).expect("green item");
    red_item
        .set_position(Vec2::new(10.0, 15.0))
        .expect("red position");
    green_item
        .set_position(Vec2::new(55.0, 30.0))
        .expect("green position");

    let original = red_item.state_snapshot().expect("snapshot red item");
    red_item
        .set_position(Vec2::new(90.0, 75.0))
        .expect("mutate position");
    red_item.set_rotation(90.0).expect("mutate rotation");
    red_item
        .set_crop(ObsSceneItemCrop::new(2, 3, 4, 5))
        .expect("mutate crop");
    red_item.set_visible(false).expect("mutate visibility");
    red_item.set_locked(true).expect("mutate lock");
    red_item
        .apply_state_snapshot(&original)
        .expect("restore complete item state");
    assert_eq!(
        red_item.state_snapshot().expect("restored snapshot"),
        original
    );

    let top_level = scene.items_in_order().expect("native top-level order");
    assert_eq!(top_level.len(), 2);
    assert_eq!(top_level[0].object_id(), red_item.object_id());
    assert_eq!(top_level[1].object_id(), green_item.object_id());

    let group = scene.create_group("foreground").expect("create group");
    let nested = scene.create_group("nested-rejected").expect("second group");
    assert!(
        group.add_item(&nested).is_err(),
        "managed groups reject nesting"
    );
    scene.remove_item(&nested).expect("remove unused group");

    group.add_item(&red_item).expect("group red");
    group.add_item(&green_item).expect("group green");
    assert!(
        group.remove_item(&group).is_err(),
        "removing a non-child through a group is rejected before libobs can reparent it"
    );
    let children = group.items_in_order().expect("group native order");
    assert_eq!(children.len(), 2);
    assert!(children
        .iter()
        .any(|item| item.object_id() == red_item.object_id()));
    assert!(children
        .iter()
        .any(|item| item.object_id() == green_item.object_id()));

    let before = capture_program(&scene, WIDTH, HEIGHT).expect("render grouped scene");
    let red_before = before.color_bounds(RED, TOLERANCE).expect("red visible");
    let group_position = group.position().expect("group position");
    group
        .set_position(Vec2::new(
            *group_position.x() + 12.0,
            *group_position.y() + 8.0,
        ))
        .expect("move group");
    let after = capture_program(&scene, WIDTH, HEIGHT).expect("render moved group");
    let red_after = after
        .color_bounds(RED, TOLERANCE)
        .expect("moved red visible");
    assert_eq!(red_after.left as i32 - red_before.left as i32, 12);
    assert_eq!(red_after.top as i32 - red_before.top as i32, 8);

    group
        .remove_item(&green_item)
        .expect("remove green from group");
    assert_eq!(group.items_in_order().expect("group after remove").len(), 1);
    let top_level = scene
        .items_in_order()
        .expect("top level after ungroup child");
    assert!(top_level
        .iter()
        .any(|item| item.object_id() == group.object_id()));
    assert!(top_level
        .iter()
        .any(|item| item.object_id() == green_item.object_id()));

    let replacements = group.ungroup().expect("ungroup foreground");
    assert!(group.is_removed());
    assert_eq!(replacements.len(), 1, "only red remained inside the group");
    assert_eq!(replacements[0].previous_object_id(), red_item.object_id());
    let replacement_red_id = replacements[0].item().object_id();
    assert_ne!(replacement_red_id, red_item.object_id());
    assert!(
        red_item.is_removed(),
        "libobs replaces grouped child items on ungroup"
    );
    assert!(
        !green_item.is_removed(),
        "green was already reparented without replacement"
    );

    let top_level = scene.items_in_order().expect("native order after ungroup");
    assert_eq!(top_level.len(), 2);
    assert!(top_level
        .iter()
        .all(|item| item.object_id() != group.object_id()));
    assert!(top_level
        .iter()
        .any(|item| item.object_id() == replacement_red_id));
    assert!(top_level
        .iter()
        .any(|item| item.object_id() == green_item.object_id()));

    let cleanup_group = scene.create_group("cleanup-group").expect("cleanup group");
    cleanup_group
        .add_item(&green_item)
        .expect("group green for deletion");
    scene
        .remove_item(&cleanup_group)
        .expect("remove group with child");
    assert!(cleanup_group.is_removed());
    assert!(
        green_item.is_removed(),
        "removing a group invalidates its child handles"
    );
    let remaining = scene.items_in_order().expect("order after deleting group");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].object_id(), replacement_red_id);

    scene.remove_from_channel(0).expect("detach scene");
    scene.clear().expect("clear scene");
}
