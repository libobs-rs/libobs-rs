//! Platform source smoke tests.
//!
//! These tests prove that the prepared OBS runtime can load and instantiate the
//! platform source exposed by libobs-simple. They deliberately do not start a
//! capture: hosted runners do not have stable display-capture permissions or
//! GPU hardware.

use std::path::PathBuf;

use libobs_wrapper::{
    context::ObsContext,
    data::object::ObsObjectTrait,
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
