//! Typed source-level audio controls and managed meter/fader lifetimes.

use std::{ffi::c_void, sync::Arc};

use crate::{
    data::audio::{AudioMixMask, AudioSyncOffset, LinearVolume},
    run_with_obs,
    runtime::ObsRuntime,
    signals::{SignalHub, SignalReceiver},
    sources::ObsSourceTrait,
    unsafe_send::{Sendable, SmartPointerSendable},
    utils::ObsError,
};

/// Decibel value used by managed OBS faders.
///
/// Negative infinity represents silence. NaN and positive infinity are rejected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decibels(f32);

impl Decibels {
    pub fn new(db: f32) -> Result<Self, ObsError> {
        if db.is_nan() || db == f32::INFINITY {
            return Err(ObsError::InvalidOperation(format!(
                "decibel value must be finite or negative infinity, got {db}"
            )));
        }
        Ok(Self(db))
    }

    pub const fn value(self) -> f32 {
        self.0
    }
}

/// OBS fader mapping used for GUI deflection and meter conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObsFaderScale {
    Cubic,
    Iec,
    Logarithmic,
}

impl ObsFaderScale {
    fn native(self) -> libobs::obs_fader_type {
        match self {
            Self::Cubic => libobs::obs_fader_type_OBS_FADER_CUBIC,
            Self::Iec => libobs::obs_fader_type_OBS_FADER_IEC,
            Self::Logarithmic => libobs::obs_fader_type_OBS_FADER_LOG,
        }
    }
}

/// Peak measurement strategy for a managed volume meter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObsPeakMeterType {
    SamplePeak,
    TruePeak,
}

impl ObsPeakMeterType {
    fn native(self) -> libobs::obs_peak_meter_type {
        match self {
            Self::SamplePeak => libobs::obs_peak_meter_type_SAMPLE_PEAK_METER,
            Self::TruePeak => libobs::obs_peak_meter_type_TRUE_PEAK_METER,
        }
    }
}

/// How a source is routed to the selected OBS monitoring device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioMonitoringType {
    None,
    MonitorOnly,
    MonitorAndOutput,
}

impl AudioMonitoringType {
    fn native(self) -> libobs::obs_monitoring_type {
        match self {
            Self::None => libobs::obs_monitoring_type_OBS_MONITORING_TYPE_NONE,
            Self::MonitorOnly => libobs::obs_monitoring_type_OBS_MONITORING_TYPE_MONITOR_ONLY,
            Self::MonitorAndOutput => {
                libobs::obs_monitoring_type_OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT
            }
        }
    }

    fn from_native(value: libobs::obs_monitoring_type) -> Result<Self, ObsError> {
        match value {
            libobs::obs_monitoring_type_OBS_MONITORING_TYPE_NONE => Ok(Self::None),
            libobs::obs_monitoring_type_OBS_MONITORING_TYPE_MONITOR_ONLY => Ok(Self::MonitorOnly),
            libobs::obs_monitoring_type_OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT => {
                Ok(Self::MonitorAndOutput)
            }
            other => Err(ObsError::EnumConversionError(format!(
                "unknown OBS monitoring type {other}"
            ))),
        }
    }
}

fn ensure_audio_source<T: ObsSourceTrait + ?Sized>(source: &T) -> Result<(), ObsError> {
    let source_ptr = source.__native_handle();
    let runtime = source.runtime().clone();
    let flags = run_with_obs!(runtime, (source_ptr), move || unsafe {
        // Safety: the managed source handle retains the source for the actor call.
        libobs::obs_source_get_output_flags(source_ptr.get_ptr())
    })?;
    if flags & libobs::OBS_SOURCE_AUDIO == 0 {
        return Err(ObsError::InvalidOperation(format!(
            "source '{}' does not expose OBS_SOURCE_AUDIO",
            source.name()
        )));
    }
    Ok(())
}

/// Audio controls available on any managed source that exposes `OBS_SOURCE_AUDIO`.
pub trait ObsSourceAudioExt: ObsSourceTrait {
    fn set_volume(&self, volume: LinearVolume) -> Result<(), ObsError> {
        ensure_audio_source(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call; libobs accepts a linear multiplier.
            libobs::obs_source_set_volume(source.get_ptr(), volume.multiplier());
        })
    }

    fn volume(&self) -> Result<LinearVolume, ObsError> {
        ensure_audio_source(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        let value = run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_get_volume(source.get_ptr())
        })?;
        LinearVolume::new(value)
    }

    fn set_muted(&self, muted: bool) -> Result<(), ObsError> {
        ensure_audio_source(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_set_muted(source.get_ptr(), muted);
        })
    }

    fn muted(&self) -> Result<bool, ObsError> {
        ensure_audio_source(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_muted(source.get_ptr())
        })
    }

    fn set_sync_offset(&self, offset: AudioSyncOffset) -> Result<(), ObsError> {
        ensure_audio_source(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call. libobs's unit is nanoseconds.
            libobs::obs_source_set_sync_offset(source.get_ptr(), offset.nanoseconds());
        })
    }

    fn sync_offset(&self) -> Result<AudioSyncOffset, ObsError> {
        ensure_audio_source(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        let value = run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_get_sync_offset(source.get_ptr())
        })?;
        Ok(AudioSyncOffset::from_nanoseconds(value))
    }

    /// Routes this source to the selected native audio mixes/tracks.
    fn set_audio_mixes(&self, mixes: AudioMixMask) -> Result<(), ObsError> {
        ensure_audio_source(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained and AudioMixMask prevents out-of-range bits.
            libobs::obs_source_set_audio_mixers(source.get_ptr(), mixes.bits());
        })
    }

    fn audio_mixes(&self) -> Result<AudioMixMask, ObsError> {
        ensure_audio_source(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        let bits = run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_get_audio_mixers(source.get_ptr())
        })?;
        AudioMixMask::from_bits(bits)
    }

    fn set_monitoring_type(&self, monitoring: AudioMonitoringType) -> Result<(), ObsError> {
        ensure_audio_source(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call and enum maps to native constants.
            libobs::obs_source_set_monitoring_type(source.get_ptr(), monitoring.native());
        })
    }

    fn monitoring_type(&self) -> Result<AudioMonitoringType, ObsError> {
        ensure_audio_source(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        let native = run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_get_monitoring_type(source.get_ptr())
        })?;
        AudioMonitoringType::from_native(native)
    }

    fn create_fader(&self, scale: ObsFaderScale) -> Result<ObsFader, ObsError> {
        ObsFader::new(self, scale)
    }

    fn create_volume_meter(
        &self,
        scale: ObsFaderScale,
        peak_type: ObsPeakMeterType,
    ) -> Result<ObsVolumeMeter, ObsError> {
        ObsVolumeMeter::new(self, scale, peak_type)
    }
}

impl<T: ObsSourceTrait + ?Sized> ObsSourceAudioExt for T {}

extern "C" fn fader_changed_callback(param: *mut c_void, db: f32) {
    let Some(hub) = (unsafe {
        // Safety: callback userdata is Arc::as_ptr from ObsFader and survives native removal.
        (param as *const SignalHub<Decibels>).as_ref()
    }) else {
        return;
    };
    if let Ok(db) = Decibels::new(db) {
        hub.publish(db);
    }
}

/// Managed OBS fader attached to one source.
///
/// Drop removes the native callback before detaching/destroying the fader, and keeps both callback
/// state and the source lease alive until that actor-thread cleanup completes.
#[derive(Debug)]
pub struct ObsFader {
    runtime: ObsRuntime,
    fader: Sendable<*mut libobs::obs_fader_t>,
    _source: SmartPointerSendable<*mut libobs::obs_source_t>,
    changed: Arc<SignalHub<Decibels>>,
}

impl ObsFader {
    fn new<T: ObsSourceTrait + ?Sized>(source: &T, scale: ObsFaderScale) -> Result<Self, ObsError> {
        ensure_audio_source(source)?;
        let source_ptr = source.__native_handle();
        let source_for_actor = source_ptr.clone();
        let runtime = source.runtime().clone();
        let changed = Arc::new(SignalHub::new());
        let callback_state = changed.clone();
        let fader = run_with_obs!(
            runtime,
            (source_for_actor, callback_state),
            move || unsafe {
                // Safety: creation/attachment/callback registration are all actor-thread operations.
                let fader = libobs::obs_fader_create(scale.native());
                if fader.is_null() {
                    return Err(ObsError::NullPointer(Some("obs_fader_create".into())));
                }
                if !libobs::obs_fader_attach_source(fader, source_for_actor.get_ptr()) {
                    libobs::obs_fader_destroy(fader);
                    return Err(ObsError::InvalidOperation(
                        "failed to attach OBS fader to source".into(),
                    ));
                }
                libobs::obs_fader_add_callback(
                    fader,
                    Some(fader_changed_callback),
                    Arc::as_ptr(&callback_state) as *mut c_void,
                );
                Ok(Sendable(fader))
            }
        )??;

        Ok(Self {
            runtime,
            fader,
            _source: source_ptr,
            changed,
        })
    }

    pub fn on_changed(&self) -> SignalReceiver<Decibels> {
        self.changed.subscribe()
    }

    pub fn set_db(&self, db: Decibels) -> Result<bool, ObsError> {
        let fader = self.fader.clone();
        let runtime = self.runtime.clone();
        run_with_obs!(runtime, (fader), move || unsafe {
            // Safety: the fader remains owned by self through the actor call.
            libobs::obs_fader_set_db(fader.get(), db.value())
        })
    }

    pub fn db(&self) -> Result<Decibels, ObsError> {
        let fader = self.fader.clone();
        let runtime = self.runtime.clone();
        let db = run_with_obs!(runtime, (fader), move || unsafe {
            // Safety: the fader remains owned by self through the actor call.
            libobs::obs_fader_get_db(fader.get())
        })?;
        Decibels::new(db)
    }

    pub fn set_linear(&self, volume: LinearVolume) -> Result<bool, ObsError> {
        let fader = self.fader.clone();
        let runtime = self.runtime.clone();
        run_with_obs!(runtime, (fader), move || unsafe {
            // Safety: the fader remains owned by self through the actor call.
            libobs::obs_fader_set_mul(fader.get(), volume.multiplier())
        })
    }

    pub fn linear(&self) -> Result<LinearVolume, ObsError> {
        let fader = self.fader.clone();
        let runtime = self.runtime.clone();
        let value = run_with_obs!(runtime, (fader), move || unsafe {
            // Safety: the fader remains owned by self through the actor call.
            libobs::obs_fader_get_mul(fader.get())
        })?;
        LinearVolume::new(value)
    }
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
impl Drop for ObsFader {
    fn drop(&mut self) {
        let fader = self.fader.clone();
        let source = self._source.clone();
        let changed = self.changed.clone();
        self.runtime.defer_obs_cleanup(move || unsafe {
            // Safety: all C callback state is retained in this closure until removal finishes.
            libobs::obs_fader_remove_callback(
                fader.get(),
                Some(fader_changed_callback),
                Arc::as_ptr(&changed) as *mut c_void,
            );
            libobs::obs_fader_detach_source(fader.get());
            libobs::obs_fader_destroy(fader.get());
            drop(source);
        });
    }
}

/// Owned snapshot from an OBS volume-meter callback.
#[derive(Clone, Debug, PartialEq)]
pub struct VolumeMeterFrame {
    pub magnitude: [f32; libobs::MAX_AUDIO_CHANNELS as usize],
    pub peak: [f32; libobs::MAX_AUDIO_CHANNELS as usize],
    pub input_peak: [f32; libobs::MAX_AUDIO_CHANNELS as usize],
}

extern "C" fn volume_meter_callback(
    param: *mut c_void,
    magnitude: *const f32,
    peak: *const f32,
    input_peak: *const f32,
) {
    let Some(hub) = (unsafe {
        // Safety: callback userdata is Arc::as_ptr from ObsVolumeMeter and survives removal.
        (param as *const SignalHub<VolumeMeterFrame>).as_ref()
    }) else {
        return;
    };
    if magnitude.is_null() || peak.is_null() || input_peak.is_null() {
        return;
    }

    const N: usize = libobs::MAX_AUDIO_CHANNELS as usize;
    let mut frame = VolumeMeterFrame {
        magnitude: [0.0; N],
        peak: [0.0; N],
        input_peak: [0.0; N],
    };
    unsafe {
        // Safety: OBS declares all three callback pointers as float[MAX_AUDIO_CHANNELS].
        std::ptr::copy_nonoverlapping(magnitude, frame.magnitude.as_mut_ptr(), N);
        std::ptr::copy_nonoverlapping(peak, frame.peak.as_mut_ptr(), N);
        std::ptr::copy_nonoverlapping(input_peak, frame.input_peak.as_mut_ptr(), N);
    }
    hub.publish(frame);
}

/// Managed OBS volume meter attached to one source.
#[derive(Debug)]
pub struct ObsVolumeMeter {
    runtime: ObsRuntime,
    meter: Sendable<*mut libobs::obs_volmeter_t>,
    _source: SmartPointerSendable<*mut libobs::obs_source_t>,
    updates: Arc<SignalHub<VolumeMeterFrame>>,
    channels: usize,
}

impl ObsVolumeMeter {
    fn new<T: ObsSourceTrait + ?Sized>(
        source: &T,
        scale: ObsFaderScale,
        peak_type: ObsPeakMeterType,
    ) -> Result<Self, ObsError> {
        ensure_audio_source(source)?;
        let source_ptr = source.__native_handle();
        let source_for_actor = source_ptr.clone();
        let runtime = source.runtime().clone();
        let updates = Arc::new(SignalHub::new());
        let callback_state = updates.clone();
        let (meter, channels) = run_with_obs!(
            runtime,
            (source_for_actor, callback_state),
            move || unsafe {
                // Safety: creation/attachment/callback registration are actor-thread operations.
                let meter = libobs::obs_volmeter_create(scale.native());
                if meter.is_null() {
                    return Err(ObsError::NullPointer(Some("obs_volmeter_create".into())));
                }
                libobs::obs_volmeter_set_peak_meter_type(meter, peak_type.native());
                if !libobs::obs_volmeter_attach_source(meter, source_for_actor.get_ptr()) {
                    libobs::obs_volmeter_destroy(meter);
                    return Err(ObsError::InvalidOperation(
                        "failed to attach OBS volume meter to source".into(),
                    ));
                }
                libobs::obs_volmeter_add_callback(
                    meter,
                    Some(volume_meter_callback),
                    Arc::as_ptr(&callback_state) as *mut c_void,
                );
                let channels = libobs::obs_volmeter_get_nr_channels(meter).max(0) as usize;
                Ok((
                    Sendable(meter),
                    channels.min(libobs::MAX_AUDIO_CHANNELS as usize),
                ))
            }
        )??;

        Ok(Self {
            runtime,
            meter,
            _source: source_ptr,
            updates,
            channels,
        })
    }

    pub fn channel_count(&self) -> usize {
        self.channels
    }

    pub fn on_update(&self) -> SignalReceiver<VolumeMeterFrame> {
        self.updates.subscribe()
    }
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
impl Drop for ObsVolumeMeter {
    fn drop(&mut self) {
        let meter = self.meter.clone();
        let source = self._source.clone();
        let updates = self.updates.clone();
        self.runtime.defer_obs_cleanup(move || unsafe {
            // Safety: callback userdata and source lease live through native deregistration.
            libobs::obs_volmeter_remove_callback(
                meter.get(),
                Some(volume_meter_callback),
                Arc::as_ptr(&updates) as *mut c_void,
            );
            libobs::obs_volmeter_detach_source(meter.get());
            libobs::obs_volmeter_destroy(meter.get());
            drop(source);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitoring_enum_rejects_unknown_native_values() {
        assert_eq!(
            AudioMonitoringType::from_native(
                libobs::obs_monitoring_type_OBS_MONITORING_TYPE_MONITOR_ONLY
            )
            .unwrap(),
            AudioMonitoringType::MonitorOnly
        );
        // C enums are `i32` in the MSVC bindings and `u32` elsewhere.
        #[allow(clippy::unnecessary_cast)]
        let invalid = u32::MAX as libobs::obs_monitoring_type;
        assert!(AudioMonitoringType::from_native(invalid).is_err());
    }

    #[test]
    fn decibels_accept_silence_but_reject_invalid_values() {
        assert_eq!(
            Decibels::new(f32::NEG_INFINITY).unwrap().value(),
            f32::NEG_INFINITY
        );
        assert!(Decibels::new(f32::NAN).is_err());
        assert!(Decibels::new(f32::INFINITY).is_err());
    }
}
