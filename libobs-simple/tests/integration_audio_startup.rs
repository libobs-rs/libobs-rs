#[path = "support/prepared_runtime.rs"]
mod prepared_runtime;

use std::path::{Path, PathBuf};

use libobs_simple::wrapper::{
    data::audio::ObsAudioInfo,
    enums::{ObsSamplesPerSecond, ObsSpeakerLayout},
    utils::{ObsPath, StartupInfo, StartupPaths},
};

fn test_runtime() -> Option<PathBuf> {
    std::env::var_os("LIBOBS_TEST_RUNTIME_DIR").map(PathBuf::from)
}

fn startup_paths(root: &Path) -> StartupPaths {
    prepared_runtime::startup_paths(root)
}

#[test]
fn configured_audio_info_is_the_native_active_configuration() {
    let Some(root) = test_runtime() else {
        eprintln!("skipping: LIBOBS_TEST_RUNTIME_DIR is not configured");
        return;
    };
    let config_dir = std::env::temp_dir().join(format!(
        "libobs-rs-audio-startup-config-{}",
        std::process::id()
    ));
    let requested = ObsAudioInfo::new(
        ObsSamplesPerSecond::F48000,
        ObsSpeakerLayout::Stereo,
        50,
        true,
    );
    let context = StartupInfo::new()
        .set_startup_paths(startup_paths(&root))
        .set_module_config_path(ObsPath::new(config_dir.to_string_lossy().as_ref()))
        .set_audio_info(requested.clone())
        .start()
        .expect("prepared OBS runtime should accept the requested audio configuration");

    let active = context
        .audio_info()
        .expect("libobs should expose its active audio configuration");
    assert_eq!(active.samples_per_second(), ObsSamplesPerSecond::F48000);
    assert_eq!(active.speakers(), ObsSpeakerLayout::Stereo);
    assert!(active.fixed_buffering());
    assert!(
        active.max_buffering_ms() >= requested.max_buffering_ms(),
        "libobs may normalize fixed buffering upward, but must not silently undercut the requested headroom: {active:?}"
    );
}
