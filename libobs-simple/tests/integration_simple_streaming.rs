use libobs_simple::output::streaming::ObsContextStreamingExt;
use libobs_wrapper::{
    context::ObsContext,
    data::{object::ObsObjectTrait, output::ObsOutputTrait},
    encoders::ObsEncoderTrait,
    utils::StartupInfo,
};

#[test]
fn simple_rtmp_builder_creates_a_complete_compatible_graph_without_starting_network_io() {
    let context = ObsContext::new(StartupInfo::default()).expect("initialize OBS");
    let output = context
        .simple_rtmp_stream("simple-rtmp", "rtmp://127.0.0.1/live", "libobs-rs-test-key")
        .video_bitrate(4_500)
        .audio_bitrate(128)
        .build()
        .expect("build capability-driven RTMP graph");

    assert!(!output
        .is_active()
        .expect("output remains inactive after build"));
    let composition = output.current_composition().expect("inspect composition");
    let video = composition.video_encoder().expect("video encoder attached");
    let audio = composition
        .audio_encoders()
        .get(&0)
        .expect("mixer zero audio encoder attached");
    let service = composition.service().expect("RTMP service attached");

    assert_eq!(video.codec().expect("video codec"), Some("h264".into()));
    assert_eq!(audio.codec().expect("audio codec"), Some("aac".into()));
    assert_eq!(
        service.protocol().expect("service protocol"),
        Some("RTMP".into())
    );
    assert_eq!(output.name().to_string(), "simple-rtmp");
}
