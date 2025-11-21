use std::thread;
use std::time::Duration;

use libobs_sources::linux::PipeWireCaptureSourceBuilder;
use libobs_wrapper::context::ObsContext;
use libobs_wrapper::encoders::ObsContextEncoders;
use libobs_wrapper::sources::ObsSourceBuilder;
use libobs_wrapper::utils::{AudioEncoderInfo, ObsPath, OutputInfo, StartupInfo};

pub fn main() -> anyhow::Result<()> {
    println!("Starting Linux XComposite Window Capture Example...");

    // Start the OBS context
    let startup_info = StartupInfo::default();
    let mut context = ObsContext::new(startup_info)?;

    let mut scene = context.scene("main")?;

    let _window_capture = context
        .source_builder::<PipeWireCaptureSourceBuilder, _>("PipeWire Capture")?
        .set_show_cursor(false) // Usually don't show cursor for window capture
        .add_to_scene(&mut scene)?;

    // Register the source
    scene.set_to_channel(0)?;

    // Set up output to ./linux-window-recording.mp4
    let mut output_settings = context.data()?;
    output_settings.set_string(
        "path",
        ObsPath::from_relative("linux-window-recording.mp4").build(),
    )?;

    let output_info = OutputInfo::new("ffmpeg_muxer", "output", Some(output_settings), None);
    let mut output = context.output(output_info)?;

    // Register the video encoder
    let mut video_settings = context.data()?;
    video_settings
        .bulk_update()
        .set_int("bf", 2)
        .set_bool("psycho_aq", true)
        .set_bool("lookahead", true)
        .set_string("profile", "high")
        .set_string("preset", "hq")
        .set_string("rate_control", "cbr")
        .set_int("bitrate", 8000) // Lower bitrate for window capture
        .update()?;

    let mut video_encoder = context.best_video_encoder()?;
    video_encoder.set_settings(video_settings);
    video_encoder.set_to_output(&mut output, "video_encoder")?;

    // Register the audio encoder
    let mut audio_settings = context.data()?;
    audio_settings.set_int("bitrate", 160)?;
    let audio_info =
        AudioEncoderInfo::new("ffmpeg_aac", "audio_encoder", Some(audio_settings), None);
    output.create_and_set_audio_encoder(audio_info, 0)?;

    // Start recording
    output.start()?;
    println!("Recording started! Recording for 10 seconds...");
    println!("Make sure the target window is visible and not minimized.");

    // Record for 10 seconds
    thread::sleep(Duration::from_secs(10));

    // Stop recording
    output.stop()?;
    println!("Recording stopped. Output saved to linux-window-recording.mp4");

    Ok(())
}
