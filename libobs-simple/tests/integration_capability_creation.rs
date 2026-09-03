use std::collections::HashSet;

use libobs_wrapper::{
    capabilities::{EncoderKind, SourceKind},
    context::ObsContext,
    data::{
        object::ObsObjectTrait,
        output::{ObsOutputComposition, ObsOutputTrait},
        ObsDataGetters, ObsDataSetters,
    },
    enums::ObsOrderMovement,
    graphics::Vec2,
    scenes::SceneItemTrait,
    sources::ObsSourceTrait,
    utils::{ObsError, StartupInfo},
};

#[test]
fn discovered_types_drive_typed_creation_and_lifecycle() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let mut context = ObsContext::new(StartupInfo::default()).expect("initialize OBS");

    let color_type = context
        .source_type("color_source_v3")
        .expect("discover source types")
        .expect("color_source_v3 is provided by image-source");
    assert_eq!(color_type.kind(), SourceKind::Input);

    let mut source_settings = color_type
        .default_settings_mut()
        .expect("copy source defaults");
    source_settings
        .set_int("width", 640)
        .expect("set source width")
        .set_int("height", 360)
        .expect("set source height");
    let source = context
        .create_source(&color_type, "generic-color", Some(source_settings))
        .expect("create discovered source");
    assert_eq!(source.id(), color_type.id());
    assert_eq!(source.name(), "generic-color");
    assert_eq!(source.settings().unwrap().get_int("width"), Ok(Some(640)));
    assert_eq!(source.settings().unwrap().get_int("height"), Ok(Some(360)));

    let filter_type = context
        .source_type("color_filter_v2")
        .expect("discover filter types")
        .expect("color_filter_v2 is provided by obs-filters");
    assert_eq!(filter_type.kind(), SourceKind::Filter);
    assert!(matches!(
        context.create_source(&filter_type, "wrong-kind", None),
        Err(ObsError::CapabilityKindMismatch { .. })
    ));
    let filter = context
        .create_filter(&filter_type, "generic-filter", None)
        .expect("create discovered filter");
    source.apply_filter(&filter).expect("attach typed filter");
    assert_eq!(source.get_active_filters().unwrap().len(), 1);

    let null_output_type = context
        .output_type("null_output")
        .expect("discover output types")
        .expect("null_output is provided by obs-outputs");
    let output = context
        .create_output(&null_output_type, "generic-null-output", None)
        .expect("create discovered output");
    let registered_output = context
        .get_output("generic-null-output")
        .expect("query output registry")
        .expect("generic output is registered");
    assert_eq!(registered_output.object_id(), output.object_id());

    let video_type = context
        .encoder_type("obs_x264")
        .expect("discover encoder types")
        .expect("obs_x264 is available in the validation OBS build");
    assert_eq!(video_type.kind(), EncoderKind::Video);
    let video_encoder = context
        .create_video_encoder(
            &video_type,
            "generic-video-encoder",
            Some(video_type.default_settings_mut().expect("video defaults")),
        )
        .expect("create discovered video encoder");
    assert_eq!(video_encoder.id(), video_type.id());
    assert!(matches!(
        context.create_audio_encoder(&video_type, "wrong-kind", None, 0),
        Err(ObsError::CapabilityKindMismatch { .. })
    ));

    let audio_type = context
        .encoder_type("ffmpeg_aac")
        .expect("discover encoder types")
        .expect("ffmpeg_aac is available in the validation OBS build");
    assert_eq!(audio_type.kind(), EncoderKind::Audio);
    let audio_encoder = context
        .create_audio_encoder(
            &audio_type,
            "generic-audio-encoder",
            Some(audio_type.default_settings_mut().expect("audio defaults")),
            0,
        )
        .expect("create discovered audio encoder");
    assert_eq!(audio_encoder.id(), audio_type.id());

    let service_type = context
        .service_type("rtmp_custom")
        .expect("discover service types")
        .expect("rtmp_custom is provided by rtmp-services");
    let mut service_settings = service_type
        .default_settings_mut()
        .expect("copy service defaults");
    service_settings
        .set_string("server", "rtmp://127.0.0.1/live")
        .expect("set service URL")
        .set_string("key", "libobs-rs-test")
        .expect("set stream key");
    let service = context
        .create_service(&service_type, "generic-service", Some(service_settings))
        .expect("create discovered service");

    let rtmp_output_type = context
        .output_type("rtmp_output")
        .expect("discover RTMP output")
        .expect("rtmp_output is provided by obs-outputs");
    let rtmp_output = context
        .create_output(&rtmp_output_type, "generic-rtmp-output", None)
        .expect("create discovered RTMP output");
    rtmp_output
        .apply_composition(
            ObsOutputComposition::new()
                .with_video_encoder(video_encoder.clone())
                .with_audio_encoder(0, audio_encoder.clone())
                .with_service(service.clone()),
        )
        .expect("apply runtime-affine output composition");
    assert_eq!(
        rtmp_output
            .attached_service()
            .unwrap()
            .expect("service remains attached")
            .object_id(),
        service.object_id()
    );
    assert_eq!(
        rtmp_output
            .attached_video_encoder()
            .unwrap()
            .expect("video encoder remains attached")
            .object_id(),
        video_encoder.object_id()
    );
    assert_eq!(
        rtmp_output
            .attached_audio_encoder(0)
            .unwrap()
            .expect("audio encoder remains attached")
            .object_id(),
        audio_encoder.object_id()
    );

    // Shared output handles deliberately support concurrent configuration. The per-output
    // lifecycle lock serializes the complete desired-state transition so start/stop and
    // encoder/service snapshots cannot observe half-applied wiring.
    let workers = (0..8)
        .map(|_| {
            let output = rtmp_output.clone();
            let video_encoder = video_encoder.clone();
            let audio_encoder = audio_encoder.clone();
            let service = service.clone();
            std::thread::spawn(move || {
                for _ in 0..4 {
                    output
                        .apply_composition(
                            ObsOutputComposition::new()
                                .with_video_encoder(video_encoder.clone())
                                .with_audio_encoder(0, audio_encoder.clone())
                                .with_service(service.clone()),
                        )
                        .expect("apply composition from shared output clone");
                    output
                        .apply_composition(ObsOutputComposition::new())
                        .expect("clear composition from shared output clone");
                }
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().expect("composition worker did not panic");
    }
    rtmp_output
        .apply_composition(
            ObsOutputComposition::new()
                .with_video_encoder(video_encoder.clone())
                .with_audio_encoder(0, audio_encoder.clone())
                .with_service(service.clone()),
        )
        .expect("restore composition after concurrency stress");

    rtmp_output.clear_service().expect("detach service");
    rtmp_output
        .clear_audio_encoder(0)
        .expect("detach audio encoder");
    rtmp_output
        .clear_video_encoder()
        .expect("detach video encoder");
    assert!(rtmp_output.attached_service().unwrap().is_none());
    assert!(rtmp_output.attached_video_encoder().unwrap().is_none());
    assert!(rtmp_output.attached_audio_encoders().unwrap().is_empty());

    let mut scene = context.scene("generic-scene", None).expect("create scene");
    let item = scene
        .add_discovered_source(&color_type, "scene-color", None)
        .expect("add source from discovered descriptor");
    item.set_position(Vec2::new(12.0, 34.0))
        .expect("set item position");
    item.set_scale(Vec2::new(0.5, 0.75))
        .expect("set item scale");
    item.set_rotation(17.5).expect("set item rotation");
    item.set_visible(false).expect("hide scene item");
    item.set_locked(true).expect("lock scene item");
    assert_eq!(item.position().unwrap(), Vec2::new(12.0, 34.0));
    assert_eq!(item.scale().unwrap(), Vec2::new(0.5, 0.75));
    assert_eq!(item.rotation().unwrap(), 17.5);
    assert!(!item.is_visible().unwrap());
    assert!(item.is_locked().unwrap());
    assert!(item.order_position().unwrap() >= 0);
    item.move_order(ObsOrderMovement::Top)
        .expect("move scene item");
    assert_eq!(
        scene.items_for_source(item.inner_source()).unwrap().len(),
        1
    );
    let retained_item = item.clone();
    scene
        .remove_item(&item)
        .expect("remove scene item immediately");
    assert!(retained_item.is_removed());
    assert!(scene
        .items_for_source(retained_item.inner_source())
        .unwrap()
        .is_empty());

    // Stress the public handle seam: every created source has unique runtime-scoped
    // identity, clones preserve identity, and dropping clone sets leaves OBS usable.
    let mut identities = HashSet::new();
    for index in 0..128 {
        let source = context
            .create_source(&color_type, format!("stress-source-{index}"), None)
            .expect("create stress source");
        let id = source.object_id();
        assert!(identities.insert(id));
        let clones = (0..8).map(|_| source.clone()).collect::<Vec<_>>();
        assert!(clones.iter().all(|clone| clone.object_id() == id));
        drop(source);
        drop(clones);
    }

    let post_stress = context
        .create_source(&color_type, "post-stress-source", None)
        .expect("actor remains healthy after clone/drop stress");
    assert_eq!(post_stress.id(), color_type.id());
}
