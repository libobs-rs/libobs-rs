use std::time::Duration;

use libobs_wrapper::{
    context::ObsContext,
    scenes::SceneItemTrait,
    utils::{ObsError, StartupInfo},
};

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

#[test]
fn repeated_startup_shutdown_with_managed_objects_drains_cleanly() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .is_test(true)
        .try_init();

    for cycle in 0..24_u64 {
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
}
