#[path = "common/render_probe.rs"]
mod render_probe;

use libobs_wrapper::{
    capabilities::SourceTypeInfo,
    context::ObsContext,
    data::{video::ObsVideoInfoBuilder, ObsDataSetters},
    enums::{ObsBoundsType, ObsOrderMovement},
    graphics::Vec2,
    scenes::{ObsSceneItemCrop, ObsTransformInfoBuilder, SceneItemTrait},
    sources::ObsSourceRef,
    utils::StartupInfo,
};
use render_probe::{capture_program, PixelBounds};

const CANVAS_WIDTH: u32 = 160;
const CANVAS_HEIGHT: u32 = 120;
const WHITE: [u8; 4] = [255, 255, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];
const CYAN: [u8; 4] = [0, 255, 255, 255];
const MAGENTA: [u8; 4] = [255, 0, 255, 255];
const COLOR_TOLERANCE: u8 = 3;

fn rgba_setting([r, g, b, a]: [u8; 4]) -> i64 {
    u32::from_le_bytes([r, g, b, a]) as i64
}

fn create_color_source(
    context: &ObsContext,
    color_type: &SourceTypeInfo,
    name: &str,
    width: u32,
    height: u32,
    rgba: [u8; 4],
) -> ObsSourceRef {
    let mut settings = color_type
        .default_settings_mut()
        .expect("read color-source defaults");
    settings
        .set_int("width", width as i64)
        .expect("set color-source width")
        .set_int("height", height as i64)
        .expect("set color-source height")
        .set_int("color", rgba_setting(rgba))
        .expect("set color-source color");
    context
        .create_source(color_type, name, Some(settings))
        .expect("create synthetic color source")
}

fn assert_bounds(actual: Option<PixelBounds>, expected: PixelBounds, label: &str) {
    assert_eq!(actual, Some(expected), "unexpected {label} rendered bounds");
}

#[test]
fn scene_composition_matches_rendered_pixels_for_position_order_crop_scale_and_fit() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .is_test(true)
        .try_init();

    let video = ObsVideoInfoBuilder::new()
        .base_width(CANVAS_WIDTH)
        .base_height(CANVAS_HEIGHT)
        .output_width(CANVAS_WIDTH)
        .output_height(CANVAS_HEIGHT)
        .build();
    let mut context = ObsContext::new(StartupInfo::new().set_video_info(video))
        .expect("initialize OBS with deterministic visual-test canvas");
    let capabilities = context
        .capabilities()
        .expect("discover source capabilities");
    let color_type = capabilities
        .source_types()
        .iter()
        .find(|source| source.id() == "color_source_v3")
        .expect("validation OBS exposes color_source_v3");

    let mut scene = context.scene("visual-scene", None).expect("create scene");
    scene
        .set_to_channel(0)
        .expect("show scene on program channel");

    let background = create_color_source(
        &context,
        color_type,
        "visual-background",
        CANVAS_WIDTH,
        CANVAS_HEIGHT,
        WHITE,
    );
    let background_item = scene.add(background).expect("add background");

    let red = create_color_source(&context, color_type, "visual-red", 40, 30, RED);
    let red_item = scene.add(red).expect("add red source");
    red_item
        .set_position(Vec2::new(10.0, 15.0))
        .expect("position red source");

    let green = create_color_source(&context, color_type, "visual-green", 50, 40, GREEN);
    let green_item = scene.add(green).expect("add green source");
    green_item
        .set_position(Vec2::new(25.0, 25.0))
        .expect("position green source");

    assert_eq!(
        background_item.order_position().expect("background order"),
        0
    );
    assert_eq!(red_item.order_position().expect("red order"), 1);
    assert_eq!(green_item.order_position().expect("green order"), 2);
    assert_eq!(
        red_item.position().expect("red native position"),
        Vec2::new(10.0, 15.0)
    );
    assert_eq!(
        green_item.position().expect("green native position"),
        Vec2::new(25.0, 25.0)
    );

    let frame = capture_program(&scene, CANVAS_WIDTH, CANVAS_HEIGHT)
        .expect("render positioned/z-ordered synthetic scene");
    frame.assert_pixel_close(2, 2, WHITE, COLOR_TOLERANCE);
    frame.assert_pixel_close(12, 17, RED, COLOR_TOLERANCE);
    frame.assert_pixel_close(30, 30, GREEN, COLOR_TOLERANCE);
    frame.assert_pixel_close(70, 30, GREEN, COLOR_TOLERANCE);
    assert_bounds(
        frame.color_bounds(GREEN, COLOR_TOLERANCE),
        PixelBounds {
            left: 25,
            top: 25,
            right: 74,
            bottom: 64,
        },
        "green",
    );
    assert_eq!(
        frame.count_color(GREEN, COLOR_TOLERANCE),
        50 * 40,
        "topmost green rectangle should be fully visible"
    );
    assert_eq!(
        frame.count_color(RED, COLOR_TOLERANCE),
        40 * 30 - 25 * 20,
        "green must occlude exactly the geometric overlap with red"
    );
    assert_eq!(
        frame.count_color(WHITE, COLOR_TOLERANCE),
        (CANVAS_WIDTH * CANVAS_HEIGHT - (40 * 30 + 50 * 40 - 25 * 20)) as usize,
        "background coverage must equal canvas minus the union of foreground rectangles"
    );

    green_item
        .move_order(ObsOrderMovement::Bottom)
        .expect("move green below background/red");
    assert_eq!(green_item.order_position().expect("green bottom order"), 0);
    let frame = capture_program(&scene, CANVAS_WIDTH, CANVAS_HEIGHT)
        .expect("render reordered synthetic scene");
    frame.assert_pixel_close(30, 30, RED, COLOR_TOLERANCE);
    green_item
        .move_order(ObsOrderMovement::Top)
        .expect("move green back to top");
    frame.assert_pixel_close(2, 2, WHITE, COLOR_TOLERANCE);

    red_item.set_visible(false).expect("hide red source");
    green_item.set_visible(false).expect("hide green source");

    let blue = create_color_source(&context, color_type, "visual-blue", 80, 60, BLUE);
    let blue_item = scene.add(blue).expect("add blue source");
    blue_item
        .set_position(Vec2::new(20.0, 20.0))
        .expect("position blue source");
    blue_item
        .set_crop(ObsSceneItemCrop::new(20, 10, 20, 10))
        .expect("crop blue source");
    blue_item
        .set_scale(Vec2::new(1.5, 0.5))
        .expect("scale cropped blue source");
    assert_eq!(
        blue_item.crop().expect("read native blue crop"),
        ObsSceneItemCrop::new(20, 10, 20, 10)
    );
    assert_eq!(
        blue_item.scale().expect("read native blue scale"),
        Vec2::new(1.5, 0.5)
    );

    let frame = capture_program(&scene, CANVAS_WIDTH, CANVAS_HEIGHT)
        .expect("render cropped/scaled synthetic source");
    assert_bounds(
        frame.color_bounds(BLUE, COLOR_TOLERANCE),
        PixelBounds {
            left: 20,
            top: 20,
            right: 79,
            bottom: 39,
        },
        "cropped/scaled blue",
    );
    assert_eq!(
        frame.count_color(BLUE, COLOR_TOLERANCE),
        60 * 20,
        "axis-aligned crop/scale should produce the exact expected pixel area"
    );

    blue_item.set_visible(false).expect("hide blue source");
    let hidden = capture_program(&scene, CANVAS_WIDTH, CANVAS_HEIGHT)
        .expect("render scene after hiding blue source");
    assert_eq!(
        hidden.count_color(BLUE, COLOR_TOLERANCE),
        0,
        "hidden scene items must contribute no rendered pixels"
    );

    let yellow = create_color_source(&context, color_type, "visual-rotation", 40, 20, YELLOW);
    let yellow_item = scene.add(yellow).expect("add rotation source");
    yellow_item
        .set_position(Vec2::new(110.0, 20.0))
        .expect("position rotation source");
    yellow_item
        .set_rotation(90.0)
        .expect("rotate source by a right angle");
    let rotated = capture_program(&scene, CANVAS_WIDTH, CANVAS_HEIGHT)
        .expect("render right-angle rotated source");
    assert_bounds(
        rotated.color_bounds(YELLOW, COLOR_TOLERANCE),
        PixelBounds {
            left: 90,
            top: 20,
            right: 109,
            bottom: 59,
        },
        "right-angle rotated yellow",
    );
    assert_eq!(
        rotated.count_color(YELLOW, COLOR_TOLERANCE),
        40 * 20,
        "a 90-degree rotation must preserve solid-source pixel area"
    );
    yellow_item.set_visible(false).expect("hide rotated source");

    let cyan = create_color_source(&context, color_type, "visual-bounds-crop", 80, 40, CYAN);
    let cyan_item = scene.add(cyan).expect("add bounds-crop source");
    let bounds_transform = ObsTransformInfoBuilder::new()
        .set_pos(Vec2::new(60.0, 60.0))
        .set_bounds(Vec2::new(40.0, 40.0))
        .set_bounds_type(ObsBoundsType::ScaleOuter)
        .set_crop_to_bounds(true)
        .build_with_fallback(&cyan_item)
        .expect("build bounds-crop transform");
    cyan_item
        .set_transform_info(&bounds_transform)
        .expect("apply bounds-crop transform");
    let bounded = capture_program(&scene, CANVAS_WIDTH, CANVAS_HEIGHT)
        .expect("render scale-outer crop-to-bounds source");
    assert_bounds(
        bounded.color_bounds(CYAN, COLOR_TOLERANCE),
        PixelBounds {
            left: 60,
            top: 60,
            right: 99,
            bottom: 99,
        },
        "scale-outer crop-to-bounds cyan",
    );
    assert_eq!(
        bounded.count_color(CYAN, COLOR_TOLERANCE),
        40 * 40,
        "crop-to-bounds must clip the scale-outer source to its exact bound box"
    );
    cyan_item
        .set_visible(false)
        .expect("hide bounds-crop source");

    let magenta = create_color_source(&context, color_type, "visual-fit", 80, 40, MAGENTA);
    let magenta_item = scene.add(magenta).expect("add fit-to-screen source");
    magenta_item.set_locked(true).expect("lock fit source");
    magenta_item
        .set_position(Vec2::new(13.0, 17.0))
        .expect("position locked fit source");
    assert!(!magenta_item
        .fit_source_to_screen()
        .expect("fit locked source should be a no-op"));
    assert_eq!(
        magenta_item
            .position()
            .expect("locked position remains unchanged"),
        Vec2::new(13.0, 17.0)
    );

    magenta_item.set_locked(false).expect("unlock fit source");
    assert!(magenta_item
        .fit_source_to_screen()
        .expect("fit unlocked source"));
    let fitted =
        capture_program(&scene, CANVAS_WIDTH, CANVAS_HEIGHT).expect("render fit-to-screen source");
    let fitted_bounds = fitted
        .color_bounds(MAGENTA, COLOR_TOLERANCE)
        .expect("fitted magenta source must be visible");
    assert_eq!(fitted_bounds.width(), CANVAS_WIDTH);
    assert_eq!(fitted_bounds.height(), 80);
    assert_eq!(fitted_bounds.left, 0);
    assert_eq!(fitted_bounds.right, CANVAS_WIDTH - 1);
    assert_eq!(fitted_bounds.top, 20);
    assert_eq!(fitted_bounds.bottom, 99);

    scene.remove_from_channel(0).expect("detach visual scene");
    scene.clear().expect("remove all visual-test scene items");
    drop((
        background_item,
        red_item,
        green_item,
        blue_item,
        yellow_item,
        cyan_item,
        magenta_item,
    ));
    drop(scene);

    // Run one final actor command after dropping every managed scene/item handle. Besides proving
    // the runtime is still healthy, this lets deferred native cleanup drain before ObsContext
    // begins process-global shutdown.
    context
        .capabilities()
        .expect("OBS runtime remains healthy after visual graph teardown");
}
