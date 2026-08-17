use libobs_simple::output::simple::ObsContextSimpleExt;
use libobs_wrapper::{
    context::ObsContext,
    data::output::ObsOutputTrait,
    encoders::ObsEncoderTrait,
    utils::{ObsPath, StartupInfo},
};

#[test]
fn default_simple_recording_builds_a_compatible_h264_aac_graph() {
    let context = ObsContext::new(StartupInfo::default()).expect("initialize OBS");
    let output = context
        .simple_output_builder(
            "simple-recording",
            ObsPath::new("/tmp/libobs-rs-simple-recording.mp4"),
        )
        .build()
        .expect("build capability-driven recording graph");

    assert!(!output
        .is_active()
        .expect("recording remains inactive after build"));
    let composition = output.current_composition().expect("inspect composition");
    assert_eq!(
        composition
            .video_encoder()
            .expect("video encoder")
            .codec()
            .expect("video codec"),
        Some("h264".into())
    );
    assert_eq!(
        composition
            .audio_encoders()
            .get(&0)
            .expect("audio encoder")
            .codec()
            .expect("audio codec"),
        Some("aac".into())
    );
}
