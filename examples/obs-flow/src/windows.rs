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
        ObsDataSetters,
        object::ObsObjectTrait,
        output::ObsOutputTrait,
        properties::{ObsProperty, ObsPropertyObject, types::ObsListItemValue},
    },
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
    let source = context
        .source_builder::<MonitorCaptureSourceBuilder, _>("Display name")?
        .add_to_scene(&mut scene)?;

    let properties = ObsSourceRef::get_properties_by_id("monitor_capture", context.runtime())?;
    println!("Properties: {:?}", properties);

    // Can update the output path to record to a different location
    let mut settings = context.data()?;
    settings.set_string("path", ObsPath::from_relative("obs_output.mp4"))?;

    // Update path
    output.update_settings(settings)?;
    // To get the list of all monitors
    // It has a loop hole though, somehow the monitor_id returned in property is same if we have multiple monitor of exactly same model (exactly same monitor), use `libobs-window-helper` lib for fix
    let properties = source.get_properties()?;
    let mut builder: MonitorCaptureSourceBuilder = context.source_builder("Display name 2")?;

    // Read the monitor_id from the property
    let prop = properties.get("monitor_id");
    if let Some(prop) = prop
        && let ObsProperty::List(list) = prop
        && !list.items().is_empty()
        && let ObsListItemValue::String(value) = list.items()[0].value()
    {
        builder = builder.set_monitor_id_raw(value.to_string());
    }

    // method 2 is WGC
    let source = builder
        .set_capture_method(ObsDisplayCaptureMethod::MethodWgc)
        .add_to_scene(&mut scene)?;

    println!("Source added to scene!");
    let position = scene.get_source_position(&source)?;
    println!("Position: {:?}", position);

    let scale = scene.get_source_scale(&source)?;
    println!("Scale: {:?}", scale);

    scene.fit_source_to_screen(&source)?;

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
