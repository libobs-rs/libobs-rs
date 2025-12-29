use env_logger::Env;
use libobs_wrapper::{
    context::ObsContext,
    data::{output::ObsOutputTrait, ObsDataSetters},
    encoders::{ObsContextEncoders, ObsVideoEncoderType},
    utils::{AudioEncoderInfo, OutputInfo, StartupInfo},
};

/// Stage 3: Initialize OBS and create output with video and audio encoders
#[test]
pub fn test_output_with_encoders() {
    let _ = env_logger::Builder::from_env(Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let startup_info = StartupInfo::default();
    let mut context = ObsContext::new(startup_info).unwrap();

    // Set up output settings
    let mut output_settings = context.data().unwrap();
    let rec_file = "leak_test_output_with_encoders.mp4";
    output_settings.set_string("path", rec_file).unwrap();

    // Create output
    let output_name = "output";
    let output_info = OutputInfo::new("ffmpeg_muxer", output_name, Some(output_settings), None);
    let mut output = context.output(output_info).unwrap();

    // Set up video encoder
    let mut video_settings = context.data().unwrap();
    video_settings
        .bulk_update()
        .set_int("bf", 0)
        .set_bool("psycho_aq", true)
        .set_bool("lookahead", true)
        .set_string("profile", "high")
        .set_string("preset", "fast")
        .set_string("rate_control", "cbr")
        .set_int("bitrate", 10000)
        .apply()
        .unwrap();

    let encoders = context.available_video_encoders().unwrap();
    let mut encoder = encoders
        .into_iter()
        .find(|e| {
            let t = e.get_encoder_id();
            t == &ObsVideoEncoderType::H264_TEXTURE_AMF
                || t == &ObsVideoEncoderType::AV1_TEXTURE_AMF
                || t == &ObsVideoEncoderType::OBS_X264
        })
        .unwrap();

    encoder.set_settings(video_settings);
    encoder.set_to_output(&mut output, "video_encoder").unwrap();

    // Set up audio encoder
    let mut audio_settings = context.data().unwrap();
    audio_settings.set_int("bitrate", 160).unwrap();

    let audio_info =
        AudioEncoderInfo::new("ffmpeg_aac", "audio_encoder", Some(audio_settings), None);
    output.create_and_set_audio_encoder(audio_info, 0).unwrap();

    // Output and context will be dropped here, testing for memory leaks
}
