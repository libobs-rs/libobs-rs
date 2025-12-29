use libobs_simple::{
    output::replay::ObsContextReplayExt, sources::ObsSourceBuilder,
    sources::windows::MonitorCaptureSourceBuilder,
};
use libobs_wrapper::{
    data::output::ObsOutputTrait,
    utils::{ObsPath, StartupInfo},
};

pub fn main() -> anyhow::Result<()> {
    env_logger::init();
    let mut ctx = StartupInfo::new().start()?;

    let replay_output = ctx
        .replay_buffer_builder("Test Replay Output", ObsPath::from_relative("."))
        // You can customize encoders and other settings here
        .max_time_sec(10)
        .build()?;

    let mut scene = ctx.scene("Test Scene", Some(0))?;
    let monitor_source = ctx
        .source_builder::<MonitorCaptureSourceBuilder, _>("Test Monitor Capture")?
        .set_monitor(&MonitorCaptureSourceBuilder::get_monitors()?[0])
        .add_to_scene(&mut scene)?;

    scene.fit_source_to_screen(&monitor_source)?;

    scene.remove_all_sources()?;
    ctx.scenes_mut().write().unwrap().remove(0);

    drop(scene);

    replay_output.start()?;

    println!("Replay buffer started. Press Enter to save a 10s replay...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    println!("Saving replay...");
    let out = replay_output.save_buffer()?;
    println!("====================================");
    println!("Replay saved to {}!", out.display());
    println!("====================================");
    Ok(())
}
