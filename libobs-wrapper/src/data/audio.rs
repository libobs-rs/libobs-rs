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
    /// Builds an OBS audio reset configuration.
    ///
    /// `max_buffering_ms = 0` lets libobs choose buffering dynamically. Fixed buffering can reduce
    /// latency/jitter variance, but smaller buffers make underruns more likely under CPU pressure.
    /// libobs may normalize a requested fixed buffer to a backend/frame-compatible value; query
    /// [`crate::context::ObsContext::audio_info`] after startup when the effective value matters.
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

    /// Convenience configuration for a fixed 20 ms audio buffer. This trades buffering headroom
    /// for lower latency and should be used only when the application can reliably service audio.
    pub fn new_low_latency(
        samples_per_second: ObsSamplesPerSecond,
        speakers: ObsSpeakerLayout,
    ) -> Self {
        Self::new(samples_per_second, speakers, 20, true)
    }

    pub fn samples_per_second(&self) -> ObsSamplesPerSecond {
        self.samples_per_sec
    }

    pub fn speakers(&self) -> ObsSpeakerLayout {
        self.speakers
    }

    pub fn max_buffering_ms(&self) -> u32 {
        self.max_buffering_ms
    }

    pub fn fixed_buffering(&self) -> bool {
        self.fixed_buffering
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

/// Selects one of libobs's global audio mixes (0-based).
///
/// This is the mix that an audio encoder reads from. It is deliberately distinct from
/// [`AudioEncoderSlot`], which selects where an encoder is attached on an output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AudioMixIndex(u8);

impl AudioMixIndex {
    pub fn new(index: usize) -> Result<Self, crate::utils::ObsError> {
        if index >= libobs::MAX_AUDIO_MIXES as usize {
            return Err(crate::utils::ObsError::AudioMixerIndexOutOfBounds {
                index,
                max: (libobs::MAX_AUDIO_MIXES - 1) as usize,
            });
        }
        Ok(Self(index as u8))
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn bit(self) -> u32 {
        1_u32 << self.0
    }
}

impl TryFrom<usize> for AudioMixIndex {
    type Error = crate::utils::ObsError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Selects an encoded-audio attachment slot on an OBS output (0-based).
///
/// An output slot is not the same concept as an [`AudioMixIndex`]. The encoder attached at this
/// slot may read from any mix selected when that encoder was created.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AudioEncoderSlot(u8);

impl AudioEncoderSlot {
    pub fn new(index: usize) -> Result<Self, crate::utils::ObsError> {
        if index >= libobs::MAX_AUDIO_MIXES as usize {
            return Err(crate::utils::ObsError::AudioMixerIndexOutOfBounds {
                index,
                max: (libobs::MAX_AUDIO_MIXES - 1) as usize,
            });
        }
        Ok(Self(index as u8))
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl TryFrom<usize> for AudioEncoderSlot {
    type Error = crate::utils::ObsError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Bitmask describing which native audio mixes a source contributes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct AudioMixMask(u32);

impl AudioMixMask {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn all() -> Self {
        Self((1_u32 << libobs::MAX_AUDIO_MIXES) - 1)
    }

    pub fn from_bits(bits: u32) -> Result<Self, crate::utils::ObsError> {
        let allowed = Self::all().0;
        if bits & !allowed != 0 {
            return Err(crate::utils::ObsError::InvalidOperation(format!(
                "audio mix mask {bits:#x} references mixes outside 0..{}",
                libobs::MAX_AUDIO_MIXES
            )));
        }
        Ok(Self(bits))
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, mix: AudioMixIndex) -> bool {
        self.0 & mix.bit() != 0
    }

    pub const fn with_mix(self, mix: AudioMixIndex) -> Self {
        Self(self.0 | mix.bit())
    }

    pub const fn without_mix(self, mix: AudioMixIndex) -> Self {
        Self(self.0 & !mix.bit())
    }
}

/// Linear source volume multiplier. `1.0` is unity gain, `0.0` is silence, and values above one
/// amplify. This type prevents confusing libobs's linear multiplier with decibels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearVolume(f32);

impl LinearVolume {
    pub fn new(multiplier: f32) -> Result<Self, crate::utils::ObsError> {
        if !multiplier.is_finite() || multiplier < 0.0 {
            return Err(crate::utils::ObsError::InvalidOperation(format!(
                "linear volume must be finite and non-negative, got {multiplier}"
            )));
        }
        Ok(Self(multiplier))
    }

    pub const fn silence() -> Self {
        Self(0.0)
    }

    pub const fn unity() -> Self {
        Self(1.0)
    }

    pub const fn multiplier(self) -> f32 {
        self.0
    }
}

/// Signed source A/V synchronization offset in nanoseconds, matching libobs's native unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct AudioSyncOffset(i64);

impl AudioSyncOffset {
    pub const fn from_nanoseconds(nanoseconds: i64) -> Self {
        Self(nanoseconds)
    }

    pub fn from_milliseconds(milliseconds: i64) -> Result<Self, crate::utils::ObsError> {
        milliseconds
            .checked_mul(1_000_000)
            .map(Self)
            .ok_or_else(|| {
                crate::utils::ObsError::InvalidOperation(
                    "audio sync offset overflow converting milliseconds to nanoseconds".into(),
                )
            })
    }

    pub const fn nanoseconds(self) -> i64 {
        self.0
    }
}

#[cfg(test)]
mod typed_audio_tests {
    use super::*;

    #[test]
    fn mix_and_output_slot_are_bounded_and_distinct_types() {
        assert_eq!(AudioMixIndex::new(0).unwrap().index(), 0);
        assert_eq!(AudioEncoderSlot::new(0).unwrap().index(), 0);
        assert!(AudioMixIndex::new(libobs::MAX_AUDIO_MIXES as usize).is_err());
        assert!(AudioEncoderSlot::new(libobs::MAX_AUDIO_MIXES as usize).is_err());
    }

    #[test]
    fn mix_mask_rejects_unknown_bits() {
        let mix0 = AudioMixIndex::new(0).unwrap();
        let mix2 = AudioMixIndex::new(2).unwrap();
        let mask = AudioMixMask::empty().with_mix(mix0).with_mix(mix2);
        assert_eq!(mask.bits(), 0b0101);
        assert!(mask.contains(mix0));
        assert!(AudioMixMask::from_bits(1 << libobs::MAX_AUDIO_MIXES).is_err());
    }

    #[test]
    fn typed_volume_and_sync_units_validate_boundaries() {
        assert_eq!(LinearVolume::unity().multiplier(), 1.0);
        assert!(LinearVolume::new(-0.1).is_err());
        assert!(LinearVolume::new(f32::NAN).is_err());
        assert_eq!(
            AudioSyncOffset::from_milliseconds(250)
                .unwrap()
                .nanoseconds(),
            250_000_000
        );
    }
}
