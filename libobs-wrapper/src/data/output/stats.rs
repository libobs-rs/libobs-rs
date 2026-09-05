//! Output health snapshots and restart-aware bitrate sampling.

use std::{
    ffi::CStr,
    time::{Duration, Instant},
};

use crate::{run_with_obs, utils::ObsError};

use super::ObsOutputTrait;

/// A native metric whose support may be optional for an output plugin.
#[derive(Clone, Debug, PartialEq)]
pub enum OutputMetric<T> {
    /// The metric is semantically unavailable for this output category.
    Unavailable,
    /// A non-sentinel value proves that the plugin is reporting the metric.
    Available(T),
    /// libobs returned the metric's zero/sentinel value but its public API does not expose whether
    /// the optional plugin callback exists. This is intentionally not presented as a known zero.
    Indeterminate(T),
}

impl<T> OutputMetric<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Available(value) | Self::Indeterminate(value) => Some(value),
            Self::Unavailable => None,
        }
    }
}

/// Point-in-time output health. Dropped frames are network-congestion drops, not rendering or
/// encoding lag.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputStatusSnapshot {
    pub active: bool,
    pub paused: bool,
    pub reconnecting: bool,
    pub total_bytes: OutputMetric<u64>,
    pub dropped_network_frames: OutputMetric<u64>,
    pub total_output_frames: u64,
    pub last_error: Option<String>,
}

/// Additive statistics API for any managed OBS output.
pub trait ObsOutputStatisticsExt: ObsOutputTrait {
    fn status_snapshot(&self) -> Result<OutputStatusSnapshot, ObsError> {
        let output = self.__native_handle();
        let runtime = self.runtime().clone();
        let flags = self.native_output_flags()?;
        let service_output = flags & libobs::OBS_OUTPUT_SERVICE != 0;

        run_with_obs!(runtime, (output), move || unsafe {
            // Safety: output is retained by its managed handle for the actor call.
            let active = libobs::obs_output_active(output.get_ptr());
            let paused = libobs::obs_output_paused(output.get_ptr());
            let reconnecting = libobs::obs_output_reconnecting(output.get_ptr());
            let bytes = libobs::obs_output_get_total_bytes(output.get_ptr());
            let dropped = libobs::obs_output_get_frames_dropped(output.get_ptr()).max(0) as u64;
            let total_frames = libobs::obs_output_get_total_frames(output.get_ptr()).max(0) as u64;
            let error_ptr = libobs::obs_output_get_last_error(output.get_ptr());
            let last_error = if error_ptr.is_null() {
                None
            } else {
                let value = CStr::from_ptr(error_ptr).to_string_lossy().into_owned();
                (!value.is_empty()).then_some(value)
            };

            let total_bytes = if bytes == 0 {
                OutputMetric::Indeterminate(0)
            } else {
                OutputMetric::Available(bytes)
            };
            let dropped_network_frames = if !service_output {
                OutputMetric::Unavailable
            } else if dropped == 0 {
                OutputMetric::Indeterminate(0)
            } else {
                OutputMetric::Available(dropped)
            };

            OutputStatusSnapshot {
                active,
                paused,
                reconnecting,
                total_bytes,
                dropped_network_frames,
                total_output_frames: total_frames,
                last_error,
            }
        })
    }
}

impl<T: ObsOutputTrait + ?Sized> ObsOutputStatisticsExt for T {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitrateSampleState {
    FirstSample,
    Rate,
    CounterReset,
    Unavailable,
    TooSoon,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BitrateSample {
    pub state: BitrateSampleState,
    pub bits_per_second: Option<f64>,
    pub elapsed: Option<Duration>,
}

/// Stateful byte-counter sampler. Reuse one sampler for one output lifecycle.
#[derive(Debug, Default)]
pub struct OutputBitrateSampler {
    previous: Option<(Instant, u64)>,
}

impl OutputBitrateSampler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.previous = None;
    }

    pub fn sample<T: ObsOutputStatisticsExt + ?Sized>(
        &mut self,
        output: &T,
    ) -> Result<BitrateSample, ObsError> {
        let snapshot = output.status_snapshot()?;
        self.sample_counter(Instant::now(), snapshot.total_bytes)
    }

    fn sample_counter(
        &mut self,
        now: Instant,
        bytes: OutputMetric<u64>,
    ) -> Result<BitrateSample, ObsError> {
        let bytes = match bytes {
            OutputMetric::Unavailable => {
                self.previous = None;
                return Ok(BitrateSample {
                    state: BitrateSampleState::Unavailable,
                    bits_per_second: None,
                    elapsed: None,
                });
            }
            OutputMetric::Available(bytes) | OutputMetric::Indeterminate(bytes) => bytes,
        };

        let Some((previous_time, previous_bytes)) = self.previous.replace((now, bytes)) else {
            return Ok(BitrateSample {
                state: BitrateSampleState::FirstSample,
                bits_per_second: None,
                elapsed: None,
            });
        };

        let elapsed = now.saturating_duration_since(previous_time);
        if elapsed.is_zero() {
            return Ok(BitrateSample {
                state: BitrateSampleState::TooSoon,
                bits_per_second: None,
                elapsed: Some(elapsed),
            });
        }
        if bytes < previous_bytes {
            return Ok(BitrateSample {
                state: BitrateSampleState::CounterReset,
                bits_per_second: None,
                elapsed: Some(elapsed),
            });
        }

        let delta_bytes = bytes - previous_bytes;
        Ok(BitrateSample {
            state: BitrateSampleState::Rate,
            bits_per_second: Some(delta_bytes as f64 * 8.0 / elapsed.as_secs_f64()),
            elapsed: Some(elapsed),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitrate_sampler_handles_first_rate_and_counter_reset() {
        let mut sampler = OutputBitrateSampler::new();
        let base = Instant::now();
        assert_eq!(
            sampler
                .sample_counter(base, OutputMetric::Available(1_000))
                .unwrap()
                .state,
            BitrateSampleState::FirstSample
        );
        let rate = sampler
            .sample_counter(
                base + Duration::from_secs(2),
                OutputMetric::Available(3_000),
            )
            .unwrap();
        assert_eq!(rate.state, BitrateSampleState::Rate);
        assert_eq!(rate.bits_per_second, Some(8_000.0));

        let reset = sampler
            .sample_counter(
                base + Duration::from_secs(3),
                OutputMetric::Indeterminate(0),
            )
            .unwrap();
        assert_eq!(reset.state, BitrateSampleState::CounterReset);
        assert_eq!(reset.bits_per_second, None);
    }

    #[test]
    fn unavailable_metric_clears_sampler_generation() {
        let mut sampler = OutputBitrateSampler::new();
        let base = Instant::now();
        sampler
            .sample_counter(base, OutputMetric::Available(10))
            .unwrap();
        assert_eq!(
            sampler
                .sample_counter(base + Duration::from_secs(1), OutputMetric::Unavailable)
                .unwrap()
                .state,
            BitrateSampleState::Unavailable
        );
        assert_eq!(
            sampler
                .sample_counter(base + Duration::from_secs(2), OutputMetric::Available(20))
                .unwrap()
                .state,
            BitrateSampleState::FirstSample
        );
    }
}
