use libobs::obs_audio_info2;

use crate::{
    enums::{ObsSamplesPerSecond, ObsSpeakerLayout},
    unsafe_send::Sendable,
};

/// Information passed to libobs when attempting to
/// reset the audio context using `obs_reset_audio2`.
#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObsAudioInfo {
    samples_per_sec: ObsSamplesPerSecond,
    speakers: ObsSpeakerLayout,
    max_buffering_ms: u32,
    fixed_buffering: bool,
}

impl ObsAudioInfo {
    pub fn new(
        samples_per_second: ObsSamplesPerSecond,
        speakers: ObsSpeakerLayout,
        max_buffering_ms: u32,
        fixed_buffering: bool,
    ) -> Self {
        Self {
            samples_per_sec: samples_per_second,
            speakers,
            max_buffering_ms,
            fixed_buffering,
        }
    }

    pub fn new_low_latency(
        samples_per_second: ObsSamplesPerSecond,
        speakers: ObsSpeakerLayout,
    ) -> Self {
        Self::new(samples_per_second, speakers, 20, true)
    }

    pub(crate) fn as_ptr(&self) -> Sendable<*const obs_audio_info2> {
        Sendable(self as *const Self as *const obs_audio_info2)
    }
}

impl Default for ObsAudioInfo {
    fn default() -> Self {
        Self {
            samples_per_sec: ObsSamplesPerSecond::F44100,
            speakers: ObsSpeakerLayout::Stereo,
            max_buffering_ms: 0,
            fixed_buffering: false,
        }
    }
}
