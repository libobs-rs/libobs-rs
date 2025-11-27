use libobs_sources::linux::{PipeWireScreenCaptureSourceBuilder, PipeWireSourceExtTrait};
use libobs_wrapper::context::ObsContext;
use libobs_wrapper::encoders::ObsContextEncoders;
use libobs_wrapper::logger::ObsLogger;
use libobs_wrapper::sources::ObsSourceBuilder;
use libobs_wrapper::utils::{AudioEncoderInfo, ObsPath, OutputInfo, StartupInfo};
use std::fs;

#[derive(Debug)]
pub struct NoLogger {}
impl ObsLogger for NoLogger {
    fn log(&mut self, _level: libobs_wrapper::enums::ObsLogLevel, _msg: String) {}
}

pub fn main() -> anyhow::Result<()> {
    println!("Starting Linux XComposite Window Capture Example...");
    let restore_token_path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("window_restore_token.txt");

    // Start the OBS context
    let startup_info = StartupInfo::default()
        // This is just so the console output from libobs is suppressed, this isn't recommended in production
        .set_logger(Box::new(NoLogger {}));
    let mut context = ObsContext::new(startup_info)?;

    let mut scene = context.scene("main")?;

    let mut window_capture_builder = context
        .source_builder::<PipeWireScreenCaptureSourceBuilder, _>("PipeWire Screen Capture")?;

    if let Ok(restore_token) = fs::read_to_string(&restore_token_path) {
        println!(
            "Using restore token from file({}): {}",
            &restore_token_path.display(),
            restore_token
        );
        window_capture_builder = window_capture_builder.set_restore_token(restore_token);
    }

    let window_capture = window_capture_builder.add_to_scene(&mut scene)?;

    // Register the source
    scene.set_to_channel(0)?;

    // Set up output to ./linux-window-recording.mp4
    let mut output_settings = context.data()?;
    let obs_path = ObsPath::from_relative("linux-window-recording.mp4").build();
    output_settings.set_string("path", obs_path.clone())?;

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
    println!("Recording started! Waiting for you to press Enter to stop...");
    println!("Make sure the target window is visible and not minimized.");

    // wait for enter key press
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    println!("Stopping recording...");
    // Stop recording
    output.stop()?;
    println!("Recording stopped. Output saved to {}", obs_path);
    let restore_token = window_capture.get_restore_token()?;
    println!("Restore Token: {:?}. You can use this when creating a source so the exact same window is captured again", restore_token);

    if let Some(restore_token) = restore_token {
        // Save the restore token to a file
        fs::write(&restore_token_path, restore_token)?;
        println!("Restore token saved to {}", restore_token_path.display());
    }

    Ok(())
}
