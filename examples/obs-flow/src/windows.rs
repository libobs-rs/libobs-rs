// Example demonstrating useful flows using libobs-wrapper

use std::{thread::sleep, time::Duration};

use libobs_sources::{
    ObsSourceBuilder,
    windows::{MonitorCaptureSourceBuilder, ObsDisplayCaptureMethod},
};
use libobs_wrapper::{
    Vec2,
    context::ObsContext,
    data::properties::{ObsProperty, ObsPropertyObject, types::ObsListItemValue},
    encoders::{ObsAudioEncoderType, ObsVideoEncoderType},
    sources::ObsSourceRef,
    utils::{AudioEncoderInfo, ObsPath, OutputInfo, StartupInfo, VideoEncoderInfo},
};

pub fn main() -> anyhow::Result<()> {
    env_logger::init();

    let startup_info = StartupInfo::new();
    let mut context = ObsContext::new(startup_info)?;

    // Create a new main scene
    let mut scene = context.scene("MAIN")?;
    // Set the scene as main channel for video and audio output
    scene.set_to_channel(0)?;
    scene.set_to_channel(1)?;

    // Add a ffmpeg_muxer output
    let mut output = context.output(OutputInfo::new("ffmpeg_muxer", "MAIN", None, None))?;

    // Read all the properties of source type or encoders
    let source = context
        .source_builder::<MonitorCaptureSourceBuilder, _>("Display name")?
        .add_to_scene(&mut scene)?;

    let properties = ObsSourceRef::get_properties_by_id("monitor_capture", context.runtime())?;
    println!("Properties: {:?}", properties);

    // Property name as key and its value can be passed as settings to the encoder while creating or updating the encoder
    output.create_and_set_video_encoder(VideoEncoderInfo::new(
        ObsVideoEncoderType::OBS_X264,
        "video_encoder",
        None,
        None,
    ))?;
    output.create_and_set_audio_encoder(
        //TODO use FFMPEG_AAC after fixing the enum mapping
        AudioEncoderInfo::new(ObsAudioEncoderType::FFMPEG_AAC, "audio_encoder", None, None),
        0,
    )?;

    // In case we already have the encoder and we want to use the same encoder for multiple outputs, use:
    // output.set_video_encoder(ObsVideoEncoder)?;
    // output.set_audio_encoder(ObsAudioEncoder)?;

    // Can update the output path to record to a different location
    let mut settings = context.data()?;
    settings.set_string("path", ObsPath::from_relative("obs_output.mp4"))?;

    // Update path
    context.update_output("MAIN", settings)?;
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

    let position = scene.get_source_position(&source)?;
    println!("Position: {:?}", position);

    let scale = scene.get_source_scale(&source)?;
    println!("Scale: {:?}", scale);

    scene.set_source_position(&source, Vec2::new(5.0, 5.0))?;
    scene.set_source_scale(&source, Vec2::new(0.5, 0.5))?;

    output.start()?;

    sleep(Duration::from_secs(5));

    output.pause(true)?;

    sleep(Duration::from_secs(4));

    output.pause(false)?;

    sleep(Duration::from_secs(5));

    // Stop the recording
    output.stop()?;

    // Remove the source from the scene
    // scene.remove_source(&source)?;

    Ok(())
}
