use libobs_simple::output::simple::ObsContextSimpleExt;
use libobs_simple::sources::linux::pipewire::PipeWireScreenCaptureSourceBuilder;
use libobs_simple::wrapper::{
    context::ObsContext,
    data::output::ObsOutputTrait,
    enums::ObsLogLevel,
    logger::ObsLogger,
    sources::ObsSourceBuilder,
    utils::{ObsPath, StartupInfo},
};
use std::fs;

#[derive(Debug)]
pub struct NoLogger {}
impl ObsLogger for NoLogger {
    fn log(&mut self, _level: ObsLogLevel, _msg: String) {}
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

    let mut scene = context.scene("main", Some(0))?;

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

    let obs_path = ObsPath::from_relative("linux-window-recording.mp4");
    let mut output = context
        .simple_output_builder("window-capture", obs_path.clone())
        .build()?;

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
    println!("Recording stopped. Output saved to {:?}", obs_path);
    let restore_token = window_capture.inner_source().get_restore_token()?;
    println!("Restore Token: {:?}. You can use this when creating a source so the exact same window is captured again", restore_token);

    if let Some(restore_token) = restore_token {
        // Save the restore token to a file
        fs::write(&restore_token_path, restore_token)?;
        println!("Restore token saved to {}", restore_token_path.display());
    }

    Ok(())
}
