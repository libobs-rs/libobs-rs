#[path = "common/render_probe.rs"]
mod render_probe;

use std::time::Duration;

use libobs_wrapper::{
    capabilities::SourceTypeInfo,
    context::ObsContext,
    data::{video::ObsVideoInfoBuilder, ObsDataSetters},
    graphics::Vec2,
    scenes::{ObsSceneItemCrop, SceneItemTrait},
    sources::ObsSourceRef,
    utils::StartupInfo,
};
use render_probe::{capture_until, PixelBounds};

const CANVAS_WIDTH: u32 = 200;
const CANVAS_HEIGHT: u32 = 140;
const SOURCE_WIDTH: u32 = 64;
const SOURCE_HEIGHT: u32 = 48;
const ORANGE: [u8; 4] = [255, 128, 0, 255];
const COLOR_TOLERANCE: u8 = 3;

fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

fn rgba_setting([r, g, b, a]: [u8; 4]) -> i64 {
    u32::from_le_bytes([r, g, b, a]) as i64
}

fn create_color_source(
    context: &ObsContext,
    color_type: &SourceTypeInfo,
    name: &str,
    rgba: [u8; 4],
) -> ObsSourceRef {
    let mut settings = color_type
        .default_settings_mut()
        .expect("read color-source defaults");
    settings
        .set_int("width", SOURCE_WIDTH as i64)
        .expect("set source width")
        .set_int("height", SOURCE_HEIGHT as i64)
        .expect("set source height")
        .set_int("color", rgba_setting(rgba))
        .expect("set source color");
    context
        .create_source(color_type, name, Some(settings))
        .expect("create synthetic property-test source")
}

#[test]
fn randomized_axis_aligned_crop_scale_and_position_match_rendered_geometry() {
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
        .expect("initialize deterministic property-test canvas");
    let capabilities = context
        .capabilities()
        .expect("discover source capabilities");
    let color_type = capabilities
        .source_types()
        .iter()
        .find(|source| source.id() == "color_source_v3")
        .expect("validation OBS exposes color_source_v3");

    let mut scene = context
        .scene("visual-property-scene", None)
        .expect("create property-test scene");
    scene
        .set_to_channel(0)
        .expect("show property-test scene on program channel");
    let source = create_color_source(&context, color_type, "visual-property-orange", ORANGE);
    let item = scene.add(source).expect("add property-test source");

    let mut state = 0xA11C_E5CE_5EED_C0DE_u64;
    for iteration in 0..64 {
        let left = (next_random(&mut state) % 17) as u32;
        let right = (next_random(&mut state) % 17) as u32;
        let top = (next_random(&mut state) % 13) as u32;
        let bottom = (next_random(&mut state) % 13) as u32;
        let scale_x = 1 + (next_random(&mut state) % 2) as u32;
        let scale_y = 1 + (next_random(&mut state) % 2) as u32;

        let visible_width = (SOURCE_WIDTH - left - right) * scale_x;
        let visible_height = (SOURCE_HEIGHT - top - bottom) * scale_y;
        assert!(visible_width > 0 && visible_height > 0);
        assert!(visible_width <= CANVAS_WIDTH && visible_height <= CANVAS_HEIGHT);

        let x = (next_random(&mut state) % (CANVAS_WIDTH - visible_width + 1) as u64) as u32;
        let y = (next_random(&mut state) % (CANVAS_HEIGHT - visible_height + 1) as u64) as u32;
        let crop = ObsSceneItemCrop::new(left as i32, top as i32, right as i32, bottom as i32);

        item.set_crop(crop).expect("set randomized crop");
        item.set_scale(Vec2::new(scale_x as f32, scale_y as f32))
            .expect("set randomized integer scale");
        item.set_position(Vec2::new(x as f32, y as f32))
            .expect("set randomized integer position");

        assert_eq!(item.crop().expect("read randomized crop"), crop);
        assert_eq!(
            item.scale().expect("read randomized scale"),
            Vec2::new(scale_x as f32, scale_y as f32)
        );
        assert_eq!(
            item.position().expect("read randomized position"),
            Vec2::new(x as f32, y as f32)
        );

        let expected = PixelBounds {
            left: x,
            top: y,
            right: x + visible_width - 1,
            bottom: y + visible_height - 1,
        };
        let expected_area = (visible_width * visible_height) as usize;

        // Crop changes deliberately invalidate a scene-item texrender for the next video tick.
        // Poll across a few real 30 fps frames: seeing the previous geometry briefly is legal, but
        // the rendered state must converge to the public transform we just applied.
        let frame = capture_until(
            &scene,
            CANVAS_WIDTH,
            CANVAS_HEIGHT,
            12,
            Duration::from_millis(12),
            |frame| {
                frame.color_bounds(ORANGE, COLOR_TOLERANCE) == Some(expected)
                    && frame.count_color(ORANGE, COLOR_TOLERANCE) == expected_area
            },
        )
        .expect("render randomized scene-item transform");
        let actual_bounds = frame.color_bounds(ORANGE, COLOR_TOLERANCE);
        let actual_area = frame.count_color(ORANGE, COLOR_TOLERANCE);
        assert_eq!(
            actual_bounds,
            Some(expected),
            "iteration {iteration}: rendered bounds did not converge to crop/scale/position model"
        );
        assert_eq!(
            actual_area, expected_area,
            "iteration {iteration}: rendered area did not converge to crop/scale model"
        );
    }

    scene
        .remove_from_channel(0)
        .expect("detach property-test scene");
    scene.clear().expect("clear property-test scene");
    drop(item);
    drop(scene);
    context
        .capabilities()
        .expect("runtime remains healthy after randomized visual teardown");
}
