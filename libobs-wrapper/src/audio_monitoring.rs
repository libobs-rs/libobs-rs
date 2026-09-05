//! Global OBS audio-monitoring device discovery and selection.

use std::{
    ffi::{c_char, c_void, CStr},
    ptr,
};

use crate::{
    context::ObsContext,
    run_with_obs,
    utils::{ObsError, ObsString},
};

/// One monitoring output device reported by libobs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioMonitoringDevice {
    name: String,
    id: String,
}

impl AudioMonitoringDevice {
    pub fn new(name: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            id: id.into(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

/// # Safety
/// libobs must call this only during the synchronous enumeration performed by
/// `audio_monitoring_devices`. `data` must point to that call's live device Vec, and non-null
/// `name`/`id` values must be callback-borrowed NUL-terminated strings.
unsafe extern "C" fn collect_monitoring_device(
    data: *mut c_void,
    name: *const c_char,
    id: *const c_char,
) -> bool {
    // SAFETY: callback registration supplies `data` as a pointer to a live Vec for this call.
    let Some(devices) = (unsafe { (data as *mut Vec<AudioMonitoringDevice>).as_mut() }) else {
        return false;
    };
    if name.is_null() || id.is_null() {
        return true;
    }
    // SAFETY: null was rejected above and libobs provides a C string for this callback.
    let name = unsafe { CStr::from_ptr(name) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: null was rejected above and libobs provides a C string for this callback.
    let id = unsafe { CStr::from_ptr(id) }.to_string_lossy().into_owned();
    devices.push(AudioMonitoringDevice { name, id });
    true
}

impl ObsContext {
    /// Returns whether this libobs build/platform exposes an audio monitoring backend.
    pub fn audio_monitoring_available(&self) -> Result<bool, ObsError> {
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (), move || unsafe {
            // Safety: process-global query executed on the OBS actor.
            libobs::obs_audio_monitoring_available()
        })
    }

    /// Enumerates monitoring devices synchronously and copies native strings before returning.
    pub fn audio_monitoring_devices(&self) -> Result<Vec<AudioMonitoringDevice>, ObsError> {
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (), move || {
            let mut devices = Vec::new();
            unsafe {
                // Safety: libobs enumerates synchronously; the Vec lives for all callbacks.
                libobs::obs_enum_audio_monitoring_devices(
                    Some(collect_monitoring_device),
                    &mut devices as *mut _ as *mut c_void,
                );
            }
            devices
        })
    }

    /// Returns the currently selected monitoring device, if libobs supplies one.
    pub fn audio_monitoring_device(&self) -> Result<Option<AudioMonitoringDevice>, ObsError> {
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (), move || {
            let mut name = ptr::null();
            let mut id = ptr::null();
            unsafe {
                // Safety: libobs writes borrowed strings; they are copied before returning.
                libobs::obs_get_audio_monitoring_device(&mut name, &mut id);
            }
            if name.is_null() || id.is_null() {
                return None;
            }
            Some(AudioMonitoringDevice {
                // SAFETY: both pointers were checked for null and remain valid for this actor call.
                name: unsafe { CStr::from_ptr(name) }
                    .to_string_lossy()
                    .into_owned(),
                // SAFETY: both pointers were checked for null and remain valid for this actor call.
                id: unsafe { CStr::from_ptr(id) }.to_string_lossy().into_owned(),
            })
        })
    }

    /// Selects the global audio monitoring device used by sources in monitor modes.
    pub fn set_audio_monitoring_device(
        &self,
        device: &AudioMonitoringDevice,
    ) -> Result<(), ObsError> {
        let name = ObsString::new(device.name());
        let id = ObsString::new(device.id());
        let runtime = self.runtime().clone();
        let selected = run_with_obs!(runtime, (name, id), move || unsafe {
            // Safety: strings are retained for the complete synchronous actor call.
            libobs::obs_set_audio_monitoring_device(name.as_ptr().0, id.as_ptr().0)
        })?;
        if selected {
            Ok(())
        } else {
            Err(ObsError::InvalidOperation(format!(
                "OBS rejected audio monitoring device '{}' ({})",
                device.name(),
                device.id()
            )))
        }
    }
}
