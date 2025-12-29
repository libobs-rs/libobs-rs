use libobs_wrapper::sources::ObsSourceRef;

use crate::sources::macro_helper::{define_object_manager, impl_default_builder};

define_object_manager!(
    #[derive(Debug)]
    /// A source for ALSA (Advanced Linux Sound Architecture) audio input.
    ///
    /// This source captures audio from ALSA-compatible devices on Linux systems.
    /// It provides low-level access to audio hardware through the ALSA subsystem.
    struct AlsaInputSource("alsa_input_capture") for ObsSourceRef {
        /// ALSA device ID (e.g., "default", "hw:0,0", or custom PCM device)
        #[obs_property(type_t = "string")]
        device_id: String,

        /// Custom PCM device name (used when device_id is "__custom__")
        #[obs_property(type_t = "string")]
        custom_pcm: String,

        /// Audio sample rate in Hz (e.g., 44100, 48000)
        #[obs_property(type_t = "int")]
        rate: i64,
    }
);

impl AlsaInputSourceBuilder {
    /// Set a custom PCM device
    pub fn set_custom_device(self, pcm_device: &str) -> Self {
        self.set_device_id("__custom__").set_custom_pcm(pcm_device)
    }

    /// Set a standard ALSA device
    pub fn set_alsa_device(self, device: &str) -> Self {
        self.set_device_id(device)
    }
}

impl_default_builder!(AlsaInputSourceBuilder);
