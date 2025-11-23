use libobs_source_macro::obs_object_impl;
use libobs_wrapper::sources::{ObsSourceBuilder, ObsSourceRef};

use crate::macro_helper::define_object_manager;

define_object_manager!(
    #[derive(Debug)]
    /// A source for PulseAudio audio input.
    ///
    /// This source captures audio from PulseAudio devices on Linux systems.
    /// PulseAudio is a higher-level sound server that sits on top of ALSA
    /// and provides more advanced audio routing and mixing capabilities.
    struct PulseInputSource("pulse_input_capture") for ObsSourceRef {
        /// PulseAudio device name/ID
        #[obs_property(type_t = "string")]
        device_id: String,
    }
);

define_object_manager!(
    #[derive(Debug)]
    /// A source for PulseAudio audio input.
    ///
    /// This source captures audio from PulseAudio devices on Linux systems.
    /// PulseAudio is a higher-level sound server that sits on top of ALSA
    /// and provides more advanced audio routing and mixing capabilities.
    struct PulseOutputSource("pulse_output_capture") for ObsSourceRef {
        /// PulseAudio device name/ID
        #[obs_property(type_t = "string")]
        device_id: String,
    }
);

#[obs_object_impl]
impl PulseInputSource {
    /// Set the default PulseAudio input device
    pub fn set_default_device(self) -> Self {
        self.set_device_id("default")
    }
}

#[obs_object_impl]
impl PulseOutputSource {
    /// Set the default PulseAudio output device
    pub fn set_default_device(self) -> Self {
        self.set_device_id("default")
    }
}

impl ObsSourceBuilder for PulseInputSourceBuilder {}
impl ObsSourceBuilder for PulseOutputSourceBuilder {}
