use libobs_wrapper::{
    capabilities::SourceKind,
    context::ObsContext,
    data::{
        object::ObsObjectTrait,
        output::{ObsOutputComposition, ObsOutputTrait},
    },
    enums::ObsOrderMovement,
    graphics::Vec2,
    scenes::SceneItemTrait,
    sources::ObsSourceRef,
    utils::StartupInfo,
};

fn next_random(state: &mut u64) -> u64 {
    // Fixed LCG parameters make every failure exactly reproducible from the seed.
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

#[test]
fn randomized_public_operations_survive_heavy_single_runtime_churn() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .is_test(true)
        .try_init();

    let mut context = ObsContext::new(StartupInfo::default()).expect("initialize OBS");
    let capabilities = context.capabilities().expect("discover capabilities");
    let color_type = capabilities
        .source_types()
        .iter()
        .find(|source| source.id() == "color_source_v3")
        .expect("validation OBS exposes color source");
    assert_eq!(color_type.kind(), SourceKind::Input);
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
    let video_encoder = context
        .create_video_encoder(
            video_type,
            "stress-video",
            Some(video_type.default_settings_mut().expect("video defaults")),
        )
        .expect("create stress video encoder");
    let audio_encoder = context
        .create_audio_encoder(
            audio_type,
            "stress-audio",
            Some(audio_type.default_settings_mut().expect("audio defaults")),
            0,
        )
        .expect("create stress audio encoder");
    let pipeline = context
        .output_pipeline(null_output_type, "stress-null-output", None)
        .video_encoder(video_encoder.clone())
        .audio_encoder(0, audio_encoder.clone())
        .build()
        .expect("build encoded null-output pipeline");
    let output = pipeline.output().clone();

    // Shared output handles race reads and complete desired-state rewrites. The per-output
    // lifecycle/configuration lock must keep every snapshot coherent.
    let workers = (0..4_u64)
        .map(|worker| {
            let output = output.clone();
            let video_encoder = video_encoder.clone();
            let audio_encoder = audio_encoder.clone();
            std::thread::spawn(move || {
                let mut state = 0xA11C_E5E5_u64 ^ (worker << 32);
                for _ in 0..128 {
                    match next_random(&mut state) % 3 {
                        0 => output
                            .apply_composition(
                                ObsOutputComposition::new()
                                    .with_video_encoder(video_encoder.clone())
                                    .with_audio_encoder(0, audio_encoder.clone()),
                            )
                            .expect("reapply encoded null-output composition"),
                        1 => output
                            .apply_composition(ObsOutputComposition::new())
                            .expect("detach null-output composition"),
                        _ => {
                            let composition = output
                                .current_composition()
                                .expect("read composition concurrently");
                            let has_video = composition.video_encoder().is_some();
                            let has_audio = composition.audio_encoders().contains_key(&0);
                            assert_eq!(
                                has_video, has_audio,
                                "composition snapshots must never mix old/new attachment states"
                            );
                            assert!(composition.service().is_none());
                        }
                    }
                }
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().expect("output stress worker did not panic");
    }
    output
        .apply_composition(
            ObsOutputComposition::new()
                .with_video_encoder(video_encoder.clone())
                .with_audio_encoder(0, audio_encoder.clone()),
        )
        .expect("restore complete composition after concurrency stress");

    // Keep one native runtime/graphics context alive while repeatedly creating and tearing
    // down large scene graphs. This separates handle/refcount stress from platform restart
    // behavior and gives the ownership model thousands of deterministic operations.
    for round in 0..16_u64 {
        let mut scene = context
            .scene(format!("stress-scene-{round}"), None)
            .expect("create stress scene");
        let mut sources = Vec::<ObsSourceRef>::new();
        let mut items = Vec::new();
        let mut state = 0x5EED_1234_9876_0000_u64 ^ round;

        for step in 0..96_u64 {
            let source = context
                .create_source(color_type, format!("stress-source-{round}-{step}"), None)
                .expect("create randomized source");
            let item = scene
                .add(source.clone())
                .expect("add randomized scene item");
            sources.push(source);
            items.push(item);
        }

        for _ in 0..256 {
            let index = next_random(&mut state) as usize % items.len();
            let item = &items[index];
            if item.is_removed() {
                continue;
            }
            match next_random(&mut state) % 5 {
                0 => {
                    let x = (next_random(&mut state) % 1_920) as f32;
                    let y = (next_random(&mut state) % 1_080) as f32;
                    item.set_position(Vec2::new(x, y)).expect("set position");
                }
                1 => {
                    let scale = 0.25 + (next_random(&mut state) % 300) as f32 / 100.0;
                    item.set_scale(Vec2::new(scale, scale)).expect("set scale");
                }
                2 => item
                    .set_rotation((next_random(&mut state) % 360) as f32)
                    .expect("set rotation"),
                3 => item
                    .set_visible(next_random(&mut state) & 1 == 0)
                    .expect("set visibility"),
                _ => item
                    .move_order(ObsOrderMovement::Top)
                    .expect("move item order"),
            }
        }

        // Clone/drop bursts verify opaque native identity while scene removal is in flight.
        for source in sources.iter().take(16) {
            let object_id = source.object_id();
            let clones = (0..8).map(|_| source.clone()).collect::<Vec<_>>();
            assert!(clones.iter().all(|clone| clone.object_id() == object_id));
            drop(clones);
        }

        scene.clear().expect("clear stress scene");
        assert!(items.iter().all(SceneItemTrait::is_removed));
        items.clear();
        sources.clear();
        drop(scene);
    }

    let final_composition = output
        .current_composition()
        .expect("final composition snapshot");
    assert!(final_composition.video_encoder().is_some());
    assert!(final_composition.audio_encoders().contains_key(&0));
    assert!(final_composition.service().is_none());
}
