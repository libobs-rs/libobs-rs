use env_logger::Env;
use libobs_wrapper::{
    context::ObsContext,
    data::output::ObsOutputRef,
    encoders::{ObsContextEncoders, ObsVideoEncoderType},
    utils::{AudioEncoderInfo, ObsString, OutputInfo, StartupInfo},
};

/// The string returned is the name of the obs output
#[allow(dead_code)]
pub fn initialize_obs<T: Into<ObsString> + Send + Sync>(rec_file: T) -> (ObsContext, ObsOutputRef) {
    let _ = env_logger::Builder::from_env(Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    #[allow(unused_mut)]
    let mut context = ObsContext::new(StartupInfo::default()).unwrap();

    // Set up output to ./recording.mp4
    let mut output_settings = context.data().unwrap();
    output_settings.set_string("path", rec_file).unwrap();

    let output_name = "output";
    let output_info = OutputInfo::new("ffmpeg_muxer", output_name, Some(output_settings), None);

    let mut output = context.output(output_info).unwrap();

    // Register the video encoder
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
        .update()
        .unwrap();

    let encoders = context.available_video_encoders().unwrap();

    println!(
        "Available encoders: {:?}",
        encoders
            .iter()
            .map(|e| e.get_encoder_id())
            .collect::<Vec<_>>()
    );
    let mut encoder = encoders
        .into_iter()
        .find(|e| {
            let t = e.get_encoder_id();
            t == &ObsVideoEncoderType::H264_TEXTURE_AMF
                || t == &ObsVideoEncoderType::AV1_TEXTURE_AMF
                || t == &ObsVideoEncoderType::OBS_NVENC_H264_TEX
                || t == &ObsVideoEncoderType::OBS_X264
        })
        .unwrap();

    println!("Using encoder {:?}", encoder.get_encoder_id());
    encoder.set_settings(video_settings);
    encoder.set_to_output(&mut output, "video_encoder").unwrap();

    // Register the audio encoder
    let mut audio_settings = context.data().unwrap();
    audio_settings.set_int("bitrate", 160).unwrap();

    let audio_info =
        AudioEncoderInfo::new("ffmpeg_aac", "audio_encoder", Some(audio_settings), None);

    output.create_and_set_audio_encoder(audio_info, 0).unwrap();

    (context, output)
}
