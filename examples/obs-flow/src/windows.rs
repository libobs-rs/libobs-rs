// Example demonstrating useful flows using libobs-wrapper

use std::{thread::sleep, time::Duration};

use libobs_simple::{
    output::{replay::ObsContextReplayExt, simple::ObsContextSimpleExt},
    sources::{
        ObsSourceBuilder,
        windows::{MonitorCaptureSourceBuilder, ObsDisplayCaptureMethod},
    },
};
use libobs_wrapper::{
    context::ObsContext,
    data::{
        ObsDataSetters, object::ObsObjectTrait, output::ObsOutputTrait,
        properties::ObsPropertyObject,
    },
    scenes::{SceneItemExtSceneTrait, SceneItemTrait},
    sources::ObsSourceRef,
    utils::{ObsPath, StartupInfo},
};

pub fn main() -> anyhow::Result<()> {
    env_logger::init();

    let startup_info = StartupInfo::new();
    let mut context = ObsContext::new(startup_info)?;

    // Create a new main scene
    let mut scene = context.scene("MAIN", Some(0))?;

    // Add a output
    let mut output = context
        .simple_output_builder("obs-flow-output", ObsPath::new("obs-flow-example.mp4"))
        .build()?;

    let mut replay_output = context
        .replay_buffer_builder("obs-flow-replay-buffer", ObsPath::from_relative("."))
        .build()?;

    // Read all the properties of source type or encoders
    {
        // You can also just create a source and remove it instantly again
        let scene_item = context
            .source_builder::<MonitorCaptureSourceBuilder, _>("Display name")?
            .add_to_scene(&mut scene)?;

        scene.remove_scene_item(scene_item)?;
    }

    // dropping (and removing) source again for demo purposes

    let properties =
        ObsSourceRef::get_properties_by_source_id("monitor_capture", context.runtime())?;
    println!("Properties: {:?}", properties);

    // Can update the output path to record to a different location
    let mut settings = context.data()?;
    settings.set_string("path", ObsPath::from_relative("obs_output.mp4"))?;

    // Update path
    output.update_settings(settings)?;

    // method 2 is WGC
    let scene_item = context
        .source_builder::<MonitorCaptureSourceBuilder, _>("Test Monitor Capture 2")?
        .set_monitor(&MonitorCaptureSourceBuilder::get_monitors()?[0])
        .set_capture_method(ObsDisplayCaptureMethod::MethodWgc)
        .add_to_scene(&mut scene)?;

    println!("Source added to scene!");
    let position = scene_item.get_source_position()?;
    println!("Position: {:?}", position);

    let scale = scene_item.get_source_scale()?;
    println!("Scale: {:?}", scale);

    scene_item.fit_source_to_screen()?;

    output.start()?;
    replay_output.start()?;

    sleep(Duration::from_secs(5));

    output.pause()?;

    sleep(Duration::from_secs(4));

    output.unpause()?;
    let path = replay_output.save_buffer()?;
    println!("Replay saved to: {}", path.display());

    sleep(Duration::from_secs(5));

    // Stop the recording
    output.stop()?;
    replay_output.stop()?;

    // Remove the source from the scene
    // scene.remove_source(&source)?;

    println!("Done recording!");
    Ok(())
}
