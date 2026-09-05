use std::{thread, time::Duration};

use anyhow::{Context, Result};
use libobs_simple::output::simple::{ObsContextSimpleExt, X264Preset};
use libobs_wrapper::{
    data::{
        ObsDataSetters,
        output::{ObsOutputStatisticsExt, ObsOutputTrait, OutputBitrateSampler},
    },
    utils::{ObsPath, StartupInfo},
};

fn main() -> Result<()> {
    let mut obs = StartupInfo::new().start()?;
    let color_type = obs
        .source_type("color_source_v3")?
        .context("the loaded OBS runtime does not provide color_source_v3")?;
    let mut settings = color_type.default_settings_mut()?;
    settings
        .set_int("width", 1280)?
        .set_int("height", 720)?
        .set_int("color", 0xFF3366CC_u32 as i64)?;
    let color = obs.create_source(&color_type, "health-demo-color", Some(settings))?;
    let mut scene = obs.scene("health-demo-scene", Some(0))?;
    scene.add(color)?;

    let output = obs
        .simple_output_builder("health-demo", ObsPath::new("output-health.mkv"))
        .x264_encoder(X264Preset::VeryFast)
        .video_bitrate(2_000)
        .build()?;
    output.start()?;

    let mut bitrate = OutputBitrateSampler::new();
    for _ in 0..3 {
        thread::sleep(Duration::from_secs(1));
        let status = output.status_snapshot()?;
        let sample = bitrate.sample(&output)?;
        println!(
            "active={} paused={} reconnecting={} frames={} bytes={:?} bitrate={:?}",
            status.active,
            status.paused,
            status.reconnecting,
            status.total_output_frames,
            status.total_bytes,
            sample.bits_per_second,
        );
    }

    output.stop()?;
    println!("wrote output-health.mkv");
    Ok(())
}
