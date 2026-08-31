//! Platform source smoke tests.
//!
//! These tests prove that the prepared OBS runtime can load and instantiate the
//! platform source exposed by libobs-simple. They deliberately do not start a
//! capture: hosted runners do not have stable display-capture permissions or
//! GPU hardware.

use std::{
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

use libobs_wrapper::{
    context::ObsContext,
    data::{object::ObsObjectTrait, ObsDataSetters},
    graphics::Vec2,
    scenes::{SceneItemExtSceneTrait, SceneItemTrait},
    sources::ObsSourceRef,
    utils::{ObsPath, StartupInfo, StartupPaths},
};

fn runtime_dir() -> PathBuf {
    std::env::var_os("LIBOBS_TEST_RUNTIME_DIR")
        .map(PathBuf::from)
        .expect("LIBOBS_TEST_RUNTIME_DIR must point to the prepared OBS runtime")
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn context_with_prepared_runtime() -> ObsContext {
    let runtime_dir = runtime_dir();

    #[cfg(target_os = "windows")]
    let paths = StartupPaths::new(
        ObsPath::new(runtime_dir.join("data/libobs").to_string_lossy().as_ref()),
        ObsPath::new(
            runtime_dir
                .join("obs-plugins/64bit")
                .to_string_lossy()
                .as_ref(),
        ),
        ObsPath::new(
            runtime_dir
                .join("data/obs-plugins/%module%")
                .to_string_lossy()
                .as_ref(),
        ),
    );

    #[cfg(target_os = "macos")]
    let paths = StartupPaths::new(
        ObsPath::new(runtime_dir.join("data/libobs").to_string_lossy().as_ref()),
        ObsPath::new(
            runtime_dir
                .join("obs-plugins/%module%.plugin/Contents/MacOS")
                .to_string_lossy()
                .as_ref(),
        ),
        ObsPath::new(
            runtime_dir
                .join("data/obs-plugins/%module%")
                .to_string_lossy()
                .as_ref(),
        ),
    );

    ObsContext::new(StartupInfo::new().set_startup_paths(paths))
        .expect("prepared OBS runtime should initialize")
}

#[cfg(target_os = "linux")]
fn context_with_prepared_runtime() -> ObsContext {
    let runtime_dir = runtime_dir();
    let paths = StartupPaths::new(
        ObsPath::new(
            runtime_dir
                .join("share/obs/libobs")
                .to_string_lossy()
                .as_ref(),
        ),
        ObsPath::new(
            runtime_dir
                .join("lib/obs-plugins/%module%")
                .to_string_lossy()
                .as_ref(),
        ),
        ObsPath::new(
            runtime_dir
                .join("share/obs/obs-plugins/%module%")
                .to_string_lossy()
                .as_ref(),
        ),
    );

    ObsContext::new(StartupInfo::new().set_startup_paths(paths))
        .expect("prepared OBS runtime should initialize")
}

#[test]
fn simple_output_builder_creates_x264_crf_output_from_prepared_runtime() {
    use libobs_simple::output::simple::SimpleOutputBuilder;

    let context = context_with_prepared_runtime();
    let output_path = std::env::temp_dir().join("portable-x264-crf-output.mp4");
    let output = SimpleOutputBuilder::new(
        context,
        "portable-x264-crf-output",
        ObsPath::new(output_path.to_string_lossy().as_ref()),
    )
    .crf(80)
    .build()
    .expect("prepared OBS runtime should construct an x264 CRF output");

    assert_eq!(output.name(), "portable-x264-crf-output");
}

fn color_source(
    context: &ObsContext,
    name: &str,
    color: i64,
    width: i64,
    height: i64,
) -> ObsSourceRef {
    let mut settings = context.data().expect("OBS data allocation should succeed");
    settings
        .set_int("color", color)
        .expect("color source should accept an RGBA color");
    settings
        .set_int("width", width)
        .expect("color source should accept a width");
    settings
        .set_int("height", height)
        .expect("color source should accept a height");

    ObsSourceRef::new(
        "color_source",
        name,
        Some(settings.into()),
        None,
        context.runtime().clone(),
    )
    .expect("prepared OBS runtime should provide the color source module")
}

fn sample_rgb(video: &Path, x: u32, y: u32) -> [u8; 3] {
    use ffmpeg_sidecar::{download::auto_download, paths::ffmpeg_path};

    auto_download().expect("FFmpeg should be available for recording verification");
    let filter = format!("crop=64:64:{x}:{y},scale=1:1,format=rgb24");
    let output = Command::new(ffmpeg_path())
        .args([
            "-v",
            "error",
            "-ss",
            "1",
            "-i",
            video
                .to_str()
                .expect("temporary recording path must be valid UTF-8"),
            "-frames:v",
            "1",
            "-vf",
            &filter,
            "-f",
            "rawvideo",
            "-",
        ])
        .output()
        .expect("FFmpeg should sample the recording");
    assert!(
        output.status.success(),
        "FFmpeg could not sample the recording: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout.len(), 3, "expected one RGB sample pixel");
    [output.stdout[0], output.stdout[1], output.stdout[2]]
}

#[test]
fn composed_color_scene_records_distinct_non_black_regions() {
    use libobs_simple::output::simple::ObsContextSimpleExt;
    use libobs_wrapper::data::output::ObsOutputTrait;

    let mut context = context_with_prepared_runtime();
    let recording = std::env::temp_dir().join(format!(
        "libobs-simple-composition-{}.mp4",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&recording);

    let mut scene = context
        .scene("composition-scene", Some(0))
        .expect("scene should be created and assigned to the program channel");
    let background = color_source(&context, "background", 0xFF0000FF, 1920, 1080);
    scene
        .add_source(background)
        .expect("background source should be added to the scene");
    let foreground = color_source(&context, "foreground", 0xFF00FF00, 640, 360);
    let foreground_item = scene
        .add_source(foreground)
        .expect("foreground source should be added to the scene");
    foreground_item
        .set_source_position(Vec2::new(640.0, 360.0))
        .expect("foreground source should be positioned in the scene");

    let mut output = context
        .simple_output_builder(
            "composition-recording",
            ObsPath::new(recording.to_string_lossy().as_ref()),
        )
        .crf(80)
        .build()
        .expect("scene should be encodable with the prepared OBS runtime");
    output.start().expect("OBS recording should start");
    thread::sleep(Duration::from_secs(2));
    output.stop().expect("OBS recording should stop cleanly");
    thread::sleep(Duration::from_secs(1));

    assert!(recording.is_file(), "OBS should create an MP4 recording");
    let background_rgb = sample_rgb(&recording, 100, 100);
    let foreground_rgb = sample_rgb(&recording, 800, 500);
    assert!(
        background_rgb.iter().any(|component| *component > 20),
        "background sample was unexpectedly black: {background_rgb:?}"
    );
    assert!(
        foreground_rgb.iter().any(|component| *component > 20),
        "foreground sample was unexpectedly black: {foreground_rgb:?}"
    );
    assert_ne!(
        background_rgb, foreground_rgb,
        "scene layers should produce distinct colors in different regions"
    );

    std::fs::remove_file(&recording).expect("temporary recording should be removable");
}

#[cfg(target_os = "macos")]
#[test]
fn screencapturekit_source_accepts_application_capture_configuration() {
    use libobs_simple::sources::{
        macos::{ScreenCaptureSourceBuilder, ScreenCaptureType},
        ObsSourceBuilder,
    };
    use libobs_wrapper::data::ObsObjectBuilder;

    let context = context_with_prepared_runtime();
    let source =
        ScreenCaptureSourceBuilder::new("macos-application-capture", context.runtime().clone())
            .expect("ScreenCaptureKit source builder should be available")
            .set_capture_mode(ScreenCaptureType::Application)
            .set_application("com.apple.Safari".to_string())
            .set_display_uuid("test-display".to_string())
            .set_show_cursor(true)
            .set_hide_obs(true)
            .build()
            .expect("ScreenCaptureKit source should be constructible without capture permission");

    assert_eq!(source.name(), "macos-application-capture");
}

#[cfg(target_os = "linux")]
#[test]
fn x11_screen_source_can_be_constructed_against_xvfb() {
    use libobs_simple::sources::{linux::LinuxGeneralScreenCaptureBuilder, ObsSourceBuilder};
    use libobs_wrapper::data::ObsObjectBuilder;

    let context = context_with_prepared_runtime();
    let source =
        LinuxGeneralScreenCaptureBuilder::new("xvfb-screen-capture", context.runtime().clone())
            .expect("Xvfb should select the X11 capture source")
            .set_screen(0)
            .set_show_cursor(false)
            .build()
            .expect("X11 screen source should be constructible against Xvfb");

    assert_eq!(source.name(), "xvfb-screen-capture");
}

#[cfg(target_os = "windows")]
#[test]
fn monitor_source_can_be_constructed_without_starting_desktop_capture() {
    use libobs_simple::sources::{windows::MonitorCaptureSourceBuilder, ObsSourceBuilder};
    use libobs_wrapper::data::ObsObjectBuilder;

    let context = context_with_prepared_runtime();
    let source =
        MonitorCaptureSourceBuilder::new("windows-monitor-capture", context.runtime().clone())
            .expect("monitor capture source builder should be available")
            .set_capture_cursor(false)
            .build()
            .expect("monitor capture source should be constructible without recording");

    assert_eq!(source.name(), "windows-monitor-capture");
}
