use std::{process::Command, time::Duration};

use libobs_wrapper::{
    context::ObsContext,
    scenes::SceneItemTrait,
    utils::{ObsError, StartupInfo},
};

const CHILD_ENV: &str = "LIBOBS_RS_RESTART_CHILD_CYCLE";

fn initialize_after_deferred_cleanup() -> ObsContext {
    for _ in 0..500 {
        match ObsContext::new(StartupInfo::default()) {
            Ok(context) => return context,
            Err(ObsError::ThreadFailure) => {
                // Final native releases are fire-and-forget actor cleanup. A just-dropped
                // context can still be draining for a few milliseconds even when no public
                // handle remains. Bound the retry so a stuck/leaking runtime is still a failure.
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(error) => panic!("initialize OBS lifecycle cycle: {error:?}"),
        }
    }
    panic!("previous OBS runtime did not finish deferred cleanup within one second");
}

fn exercise_runtime_cycle(cycle: u64) {
    let mut context = initialize_after_deferred_cleanup();
    let capabilities = context.capabilities().expect("discover capabilities");
    let color_type = capabilities
        .source_types()
        .iter()
        .find(|source| source.id() == "color_source_v3")
        .expect("validation OBS exposes color source");
    let null_output_type = capabilities
        .outputs()
        .iter()
        .find(|output| output.id() == "null_output")
        .expect("validation OBS exposes null output");
    let video_type = capabilities
        .select_video_encoder()
        .codec("h264")
        .matches()
        .into_iter()
        .find(|encoder| encoder.id() == "obs_x264")
        .expect("validation OBS exposes x264");
    let audio_type = capabilities
        .select_audio_encoder()
        .codec("aac")
        .matches()
        .into_iter()
        .find(|encoder| encoder.id() == "ffmpeg_aac")
        .expect("validation OBS exposes ffmpeg AAC");

    let source = context
        .create_source(color_type, format!("restart-source-{cycle}"), None)
        .expect("create restart source");
    let mut scene = context
        .scene(format!("restart-scene-{cycle}"), None)
        .expect("create restart scene");
    let item = scene.add(source.clone()).expect("add restart scene item");
    scene.remove_item(&item).expect("remove restart scene item");
    assert!(item.is_removed());

    let video = context
        .create_video_encoder(video_type, format!("restart-video-{cycle}"), None)
        .expect("create restart video encoder");
    let audio = context
        .create_audio_encoder(audio_type, format!("restart-audio-{cycle}"), None, 0)
        .expect("create restart audio encoder");
    let pipeline = context
        .output_pipeline(null_output_type, format!("restart-output-{cycle}"), None)
        .video_encoder(video.clone())
        .audio_encoder(0, audio.clone())
        .build()
        .expect("build restart output pipeline");
    assert!(!pipeline.is_active().expect("query restart output"));

    drop(item);
    drop(scene);
    drop(source);
    drop(pipeline);
    drop(video);
    drop(audio);
    drop(capabilities);
    drop(context);
}

#[test]
fn repeated_same_process_startup_shutdown_smoke() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .is_test(true)
        .try_init();

    // This specifically verifies that libobs-rs can tear down and recreate OBS global state in
    // one address space. Keep the count deliberately small: Mesa/llvmpipe's EGL implementation
    // becomes nondeterministic after many same-process X11 graphics reinitializations, which is a
    // graphics-driver limitation rather than a wrapper lifetime property.
    for cycle in 0..4_u64 {
        exercise_runtime_cycle(cycle);
    }
}

#[test]
#[ignore = "extended 24-cycle OBS restart stress; run in the dedicated lifecycle stress gate"]
fn process_isolated_runtime_lifecycle_survives_24_complete_cycles() {
    let executable = std::env::current_exe().expect("resolve lifecycle integration test binary");

    for cycle in 0..24_u64 {
        let output = Command::new(&executable)
            .arg("--exact")
            .arg("single_runtime_lifecycle_child")
            .arg("--ignored")
            .arg("--test-threads=1")
            .env(CHILD_ENV, cycle.to_string())
            .output()
            .expect("spawn isolated OBS lifecycle child");

        assert!(
            output.status.success(),
            "isolated OBS lifecycle cycle {cycle} failed with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
#[ignore = "helper executed by process_isolated_runtime_lifecycle_survives_24_complete_cycles"]
fn single_runtime_lifecycle_child() {
    let cycle = std::env::var(CHILD_ENV)
        .expect("isolated lifecycle child cycle environment variable")
        .parse::<u64>()
        .expect("parse isolated lifecycle child cycle");
    exercise_runtime_cycle(cycle);
}
