use std::{env, time::Duration};

use anyhow::{Context, Result};
use libobs_simple::output::simple::ObsContextSimpleExt;
use libobs_wrapper::{
    data::{
        audio::{
            AudioEncoderSlot, AudioMixIndex, AudioMixMask, AudioSyncOffset, LinearVolume,
            ObsAudioInfo,
        },
        output::ObsOutputTrait,
    },
    enums::{ObsSamplesPerSecond, ObsSpeakerLayout},
    sources::{ObsFaderScale, ObsPeakMeterType, ObsSourceAudioExt},
    utils::{ObsPath, StartupInfo},
};

fn main() -> Result<()> {
    let obs = StartupInfo::new()
        .set_audio_info(ObsAudioInfo::new(
            ObsSamplesPerSecond::F48000,
            ObsSpeakerLayout::Stereo,
            0,
            false,
        ))
        .start()?;

    // Source plugin IDs are platform/plugin dependent, so this example takes them from the
    // environment rather than hard-coding Windows/macOS/Linux device-source identifiers.
    let desktop_id = env::var("LIBOBS_DESKTOP_AUDIO_SOURCE_ID")
        .context("set LIBOBS_DESKTOP_AUDIO_SOURCE_ID to an installed desktop-audio source type")?;
    let mic_id = env::var("LIBOBS_MIC_AUDIO_SOURCE_ID")
        .context("set LIBOBS_MIC_AUDIO_SOURCE_ID to an installed microphone source type")?;

    let desktop_type = obs
        .source_type(&desktop_id)?
        .with_context(|| format!("OBS source type '{desktop_id}' is not installed"))?;
    let mic_type = obs
        .source_type(&mic_id)?
        .with_context(|| format!("OBS source type '{mic_id}' is not installed"))?;

    let desktop = obs.create_source(
        &desktop_type,
        "desktop-audio",
        desktop_type.default_settings_mut().ok(),
    )?;
    let mic = obs.create_source(
        &mic_type,
        "microphone",
        mic_type.default_settings_mut().ok(),
    )?;

    let combined = AudioMixIndex::new(0)?;
    let desktop_only = AudioMixIndex::new(1)?;
    let mic_only = AudioMixIndex::new(2)?;

    desktop.set_audio_mixes(
        AudioMixMask::empty()
            .with_mix(combined)
            .with_mix(desktop_only),
    )?;
    mic.set_audio_mixes(AudioMixMask::empty().with_mix(combined).with_mix(mic_only))?;

    desktop.set_volume(LinearVolume::unity())?;
    mic.set_volume(LinearVolume::new(0.85)?)?;
    mic.set_muted(false)?;
    mic.set_sync_offset(AudioSyncOffset::from_milliseconds(0)?)?;

    // Faders use dB while direct source volume above uses a typed linear multiplier.
    let _mic_fader = mic.create_fader(ObsFaderScale::Logarithmic)?;
    let mic_meter =
        mic.create_volume_meter(ObsFaderScale::Logarithmic, ObsPeakMeterType::SamplePeak)?;
    let meter_updates = mic_meter.on_update();

    let output = obs
        .simple_output_builder("multi-track-recording", ObsPath::new("audio-tracks.mkv"))
        // Keep the default combined mix 0 at output slot 0.
        .audio_track(desktop_only, AudioEncoderSlot::new(1)?)
        .audio_track(mic_only, AudioEncoderSlot::new(2)?)
        .build()?;

    output.start()?;
    if let Ok(levels) = meter_updates.recv_timeout(Duration::from_secs(1)) {
        println!(
            "microphone peak: {:?}",
            &levels.peak[..mic_meter.channel_count()]
        );
    }
    std::thread::sleep(Duration::from_secs(4));
    output.stop()?;

    println!("wrote combined + desktop-only + mic-only tracks to audio-tracks.mkv");
    Ok(())
}
