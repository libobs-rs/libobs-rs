use libobs_wrapper::{
    capabilities::{EncoderKind, OutputCapabilities},
    context::ObsContext,
    data::{object::ObsObjectTrait, output::ObsOutputTrait, ObsDataSetters},
    utils::{ObsError, StartupInfo},
};

#[test]
fn capabilities_select_and_validate_a_streaming_pipeline() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let context = ObsContext::new(StartupInfo::default()).expect("initialize OBS");
    let capabilities = context.capabilities().expect("discover OBS capabilities");

    let h264_candidates = capabilities.select_video_encoder().codec("h264").matches();
    assert!(!h264_candidates.is_empty(), "validation OBS exposes H.264");
    assert!(h264_candidates
        .iter()
        .all(|encoder| encoder.kind() == EncoderKind::Video));
    assert!(h264_candidates
        .iter()
        .all(|encoder| !encoder.is_deprecated() && !encoder.is_internal()));

    let preferred_h264 = capabilities
        .select_video_encoder()
        .codec("h264")
        .prefer_hardware()
        .best_available()
        .expect("an H.264 encoder is available");
    assert_eq!(preferred_h264.codec(), Some("h264"));

    let x264_type = h264_candidates
        .into_iter()
        .find(|encoder| encoder.id() == "obs_x264")
        .expect("validation OBS exposes obs_x264");
    let aac_type = capabilities
        .select_audio_encoder()
        .codec("aac")
        .matches()
        .into_iter()
        .find(|encoder| encoder.id() == "ffmpeg_aac")
        .expect("validation OBS exposes ffmpeg_aac");

    let rtmp_type = capabilities
        .select_output()
        .protocol("RTMP")
        .video_codec("h264")
        .audio_codec("aac")
        .require_capabilities(
            OutputCapabilities::ENCODED
                | OutputCapabilities::VIDEO
                | OutputCapabilities::AUDIO
                | OutputCapabilities::SERVICE,
        )
        .matches()
        .into_iter()
        .find(|output| output.id() == "rtmp_output")
        .expect("validation OBS exposes an RTMP H.264/AAC output");

    assert!(rtmp_type.supports_protocol("rtmp"));
    assert!(rtmp_type.supports_video_codec("H264"));
    assert!(rtmp_type.supports_audio_codec("AAC"));

    let missing_name = "pipeline-missing-components";
    let missing = context
        .output_pipeline(rtmp_type, missing_name, None)
        .validate()
        .expect_err("RTMP pipeline requires media encoders and a service");
    assert!(matches!(
        missing,
        ObsError::OutputPipelineMissingComponent { .. }
    ));
    assert!(context
        .get_output(missing_name)
        .expect("query output registry")
        .is_none());

    let video_encoder = context
        .create_video_encoder(
            x264_type,
            "pipeline-x264",
            Some(x264_type.default_settings_mut().expect("x264 defaults")),
        )
        .expect("create H.264 encoder");
    let audio_encoder = context
        .create_audio_encoder(
            aac_type,
            "pipeline-aac",
            Some(aac_type.default_settings_mut().expect("AAC defaults")),
            0,
        )
        .expect("create AAC encoder");

    let service_type = context
        .service_type("rtmp_custom")
        .expect("discover service types")
        .expect("validation OBS exposes custom RTMP service");
    let mut service_settings = service_type
        .default_settings_mut()
        .expect("service defaults");
    service_settings
        .set_string("server", "rtmp://127.0.0.1/live")
        .expect("set test server")
        .set_string("key", "libobs-rs-pipeline-test")
        .expect("set test stream key");
    let service = context
        .create_service(&service_type, "pipeline-service", Some(service_settings))
        .expect("create RTMP service");

    let invalid_mixer = context
        .output_pipeline(rtmp_type, "pipeline-bad-mixer", None)
        .video_encoder(video_encoder.clone())
        .audio_encoder(libobs::MAX_AUDIO_MIXES as usize, audio_encoder.clone())
        .service(service.clone())
        .validate()
        .expect_err("invalid mixer must fail before output creation");
    assert!(matches!(
        invalid_mixer,
        ObsError::AudioMixerIndexOutOfBounds { .. }
    ));
    assert!(context
        .get_output("pipeline-bad-mixer")
        .expect("query output registry")
        .is_none());

    let null_output_type = capabilities
        .outputs()
        .iter()
        .find(|output| output.id() == "null_output")
        .expect("validation OBS exposes null output");
    let null_pipeline = context
        .output_pipeline(null_output_type, "pipeline-null-output", None)
        .video_encoder(video_encoder.clone())
        .audio_encoder(0, audio_encoder.clone())
        .build()
        .expect("build encoded null output");
    assert!(matches!(
        null_pipeline.output().set_service(service.clone()),
        Err(ObsError::OutputPipelineUnexpectedComponent { .. })
    ));
    assert!(null_pipeline
        .output()
        .attached_service()
        .expect("inspect rejected service attachment")
        .is_none());

    let pipeline = context
        .output_pipeline(rtmp_type, "validated-rtmp-pipeline", None)
        .video_encoder(video_encoder.clone())
        .audio_encoder(0, audio_encoder.clone())
        .service(service.clone())
        .build()
        .expect("build validated RTMP pipeline");

    assert!(!pipeline.is_active().expect("query pipeline state"));
    let composition = pipeline
        .output()
        .current_composition()
        .expect("inspect pipeline composition");
    assert_eq!(
        composition
            .video_encoder()
            .expect("video encoder attached")
            .object_id(),
        video_encoder.object_id()
    );
    assert_eq!(
        composition
            .audio_encoders()
            .get(&0)
            .expect("audio encoder attached")
            .object_id(),
        audio_encoder.object_id()
    );
    assert_eq!(
        composition.service().expect("service attached").object_id(),
        service.object_id()
    );
}
