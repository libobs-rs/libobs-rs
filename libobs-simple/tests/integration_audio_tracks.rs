use std::{path::PathBuf, process::Command, thread, time::Duration};

use libobs_simple::{
    output::simple::{ObsContextSimpleExt, OutputFormat, X264Preset},
    wrapper::{
        data::{
            audio::{AudioEncoderSlot, AudioMixIndex, AudioMixMask},
            output::ObsOutputTrait,
            ObsDataSetters,
        },
        sources::ObsSourceAudioExt,
        utils::{ObsPath, StartupInfo, StartupPaths},
    },
};

fn runtime_dir() -> Option<PathBuf> {
    std::env::var_os("LIBOBS_TEST_RUNTIME_DIR").map(PathBuf::from)
}

fn start_context() -> Option<libobs_simple::wrapper::context::ObsContext> {
    let root = runtime_dir()?;
    let multiarch = match std::env::consts::ARCH {
        "x86_64" => "x86_64-linux-gnu",
        "aarch64" => "aarch64-linux-gnu",
        "arm" => "arm-linux-gnueabihf",
        _ => "",
    };
    let plugin_root = root.join("lib").join(multiarch).join("obs-plugins");
    let paths = StartupPaths::new(
        ObsPath::new(root.join("share/obs/libobs").to_string_lossy().as_ref()),
        ObsPath::new(plugin_root.join("%module%").to_string_lossy().as_ref()),
        ObsPath::new(
            root.join("share/obs/obs-plugins/%module%")
                .to_string_lossy()
                .as_ref(),
        ),
    );
    let config = std::env::temp_dir().join(format!(
        "libobs-rs-audio-tracks-config-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&config).expect("module config directory");
    Some(
        StartupInfo::new()
            .set_startup_paths(paths)
            .set_module_config_path(ObsPath::new(config.to_string_lossy().as_ref()))
            .start()
            .expect("prepared OBS runtime should initialize"),
    )
}

fn generate_tone(path: &std::path::Path, frequency: u32) {
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency={frequency}:sample_rate=44100:duration=8"),
            "-ac",
            "2",
        ])
        .arg(path)
        .status()
        .expect("ffmpeg should be installed");
    assert!(status.success(), "tone generation should succeed");
}

#[test]
fn native_recording_contains_combined_and_separate_audio_tracks() {
    let Some(mut context) = start_context() else {
        eprintln!("skipping: LIBOBS_TEST_RUNTIME_DIR is not configured");
        return;
    };

    let root = std::env::temp_dir().join(format!(
        "libobs-rs-audio-tracks-native-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let desktop_tone = root.join("desktop-440.wav");
    let mic_tone = root.join("mic-880.wav");
    let recording = root.join("tracks.mkv");
    generate_tone(&desktop_tone, 440);
    generate_tone(&mic_tone, 880);

    let media_type = context
        .source_type("ffmpeg_source")
        .unwrap()
        .expect("obs-ffmpeg media source should be installed");

    let mut desktop_settings = media_type.default_settings_mut().unwrap();
    desktop_settings
        .set_bool("is_local_file", true)
        .unwrap()
        .set_string("local_file", desktop_tone.to_string_lossy().as_ref())
        .unwrap()
        .set_bool("looping", true)
        .unwrap();
    let desktop = context
        .create_source(&media_type, "Synthetic desktop", Some(desktop_settings))
        .unwrap();

    let mut mic_settings = media_type.default_settings_mut().unwrap();
    mic_settings
        .set_bool("is_local_file", true)
        .unwrap()
        .set_string("local_file", mic_tone.to_string_lossy().as_ref())
        .unwrap()
        .set_bool("looping", true)
        .unwrap();
    let mic = context
        .create_source(&media_type, "Synthetic microphone", Some(mic_settings))
        .unwrap();

    let combined = AudioMixIndex::new(0).unwrap();
    let desktop_only = AudioMixIndex::new(1).unwrap();
    let mic_only = AudioMixIndex::new(2).unwrap();
    desktop
        .set_audio_mixes(
            AudioMixMask::empty()
                .with_mix(combined)
                .with_mix(desktop_only),
        )
        .unwrap();
    mic.set_audio_mixes(AudioMixMask::empty().with_mix(combined).with_mix(mic_only))
        .unwrap();

    let mut scene = context.scene("Synthetic audio routing", Some(0)).unwrap();
    scene.add(desktop).unwrap();
    scene.add(mic).unwrap();

    let output = context
        .simple_output_builder(
            "three-track-audio",
            ObsPath::new(recording.to_string_lossy().as_ref()),
        )
        .format(OutputFormat::MatroskaVideo)
        .x264_encoder(X264Preset::VeryFast)
        .video_bitrate(1_000)
        .audio_track(desktop_only, AudioEncoderSlot::new(1).unwrap())
        .audio_track(mic_only, AudioEncoderSlot::new(2).unwrap())
        .build()
        .expect("multi-track graph should build");

    output.start().expect("recording should start");
    thread::sleep(Duration::from_secs(4));
    output
        .stop_with_timeout(Duration::from_secs(15))
        .expect("recording should stop");
    assert!(recording.is_file());

    let probe = Command::new("ffprobe")
        .args(["-v", "error", "-show_streams", "-of", "json"])
        .arg(&recording)
        .output()
        .expect("ffprobe should run");
    assert!(
        probe.status.success(),
        "ffprobe failed: {}",
        String::from_utf8_lossy(&probe.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
    let audio = json["streams"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|stream| stream["codec_type"] == "audio")
        .collect::<Vec<_>>();
    assert_eq!(
        audio.len(),
        3,
        "combined + desktop + microphone tracks should be present: {json}"
    );

    // Every dedicated track should contain real signal, not merely an empty encoder stream.
    for audio_index in 0..3 {
        let decoded = Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(&recording)
            .args([
                "-map",
                &format!("0:a:{audio_index}"),
                "-t",
                "1",
                "-f",
                "s16le",
                "-acodec",
                "pcm_s16le",
                "-",
            ])
            .output()
            .expect("ffmpeg should decode audio track");
        assert!(decoded.status.success());
        assert!(
            decoded.stdout.iter().any(|sample| *sample != 0),
            "audio track {audio_index} should not be silent/empty"
        );
    }

    let _ = std::fs::remove_dir_all(root);
}
