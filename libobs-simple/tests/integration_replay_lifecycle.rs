#[path = "support/prepared_runtime.rs"]
mod prepared_runtime;

use std::{path::PathBuf, process::Command, thread, time::Duration};

use libobs_simple::{
    output::{replay::ObsContextReplayExt, simple::X264Preset},
    wrapper::{
        data::{output::ObsOutputTrait, ObsDataSetters},
        utils::{ObsPath, StartupInfo},
    },
};

fn runtime_dir() -> Option<PathBuf> {
    std::env::var_os("LIBOBS_TEST_RUNTIME_DIR").map(PathBuf::from)
}

fn start_context() -> Option<libobs_simple::wrapper::context::ObsContext> {
    let root = runtime_dir()?;
    let paths = prepared_runtime::startup_paths(&root);
    let config =
        std::env::temp_dir().join(format!("libobs-rs-replay-config-{}", std::process::id()));
    std::fs::create_dir_all(&config).expect("test module config directory");
    Some(
        StartupInfo::new()
            .set_startup_paths(paths)
            .set_module_config_path(ObsPath::new(config.to_string_lossy().as_ref()))
            .start()
            .expect("prepared OBS runtime should initialize"),
    )
}

#[test]
fn native_replay_save_returns_the_completed_existing_media_file() {
    let Some(mut context) = start_context() else {
        eprintln!("skipping: LIBOBS_TEST_RUNTIME_DIR is not configured");
        return;
    };

    let replay_dir =
        std::env::temp_dir().join(format!("libobs-rs-replay-native-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&replay_dir);
    std::fs::create_dir_all(&replay_dir).unwrap();

    let color_type = context
        .source_type("color_source_v3")
        .unwrap()
        .expect("color source plugin");
    let mut color_settings = color_type.default_settings_mut().unwrap();
    color_settings
        .set_int("width", 640)
        .unwrap()
        .set_int("height", 360)
        .unwrap()
        .set_int("color", 0xFF22AA44_u32 as i64)
        .unwrap();
    let color = context
        .create_source(&color_type, "Replay native color", Some(color_settings))
        .unwrap();
    let mut scene = context.scene("Replay native scene", Some(0)).unwrap();
    scene.add(color).unwrap();

    let replay = context
        .replay_buffer_builder(
            "native-replay-buffer",
            ObsPath::new(replay_dir.to_string_lossy().as_ref()),
        )
        .max_time_sec(3)
        .max_size_mb(64)
        .format("libobs-rs-replay-%CCYY-%MM-%DD-%hh-%mm-%ss")
        .extension("mkv")
        .x264_encoder(X264Preset::VeryFast)
        .video_bitrate(1_500)
        .build()
        .expect("replay graph should build");

    replay.start().expect("replay buffer should start");
    thread::sleep(Duration::from_secs(4));
    let saved = replay
        .save_buffer_with_timeout(Duration::from_secs(15))
        .expect("save completion should be correlated with this request");
    assert!(
        saved.is_file(),
        "returned replay path must exist: {saved:?}"
    );
    replay.stop_with_timeout(Duration::from_secs(15)).unwrap();

    let probe = Command::new("ffprobe")
        .args(["-v", "error", "-show_streams", "-of", "json"])
        .arg(saved.as_ref())
        .output()
        .expect("ffprobe should be installed");
    assert!(
        probe.status.success(),
        "saved replay must be valid media: {}",
        String::from_utf8_lossy(&probe.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
    assert!(
        json["streams"]
            .as_array()
            .is_some_and(|streams| streams.iter().any(|stream| stream["codec_type"] == "video")),
        "saved replay should contain a video stream: {json}"
    );

    std::fs::remove_file(saved.as_ref()).unwrap();
    let _ = std::fs::remove_dir_all(replay_dir);
}
