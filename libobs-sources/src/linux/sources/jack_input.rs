use libobs_wrapper::sources::{ObsSourceBuilder, ObsSourceRef};

use crate::macro_helper::define_object_manager;

define_object_manager!(
    #[derive(Debug)]
    /// A source for JACK (JACK Audio Connection Kit) audio input.
    ///
    /// This source captures audio from JACK-compatible applications and devices.
    /// JACK is a professional audio server that provides low-latency audio
    /// connections between applications and hardware.
    struct JackInputSource("jack_input_capture") for ObsSourceRef {
        /// JACK client name pattern or specific client to connect to
        #[obs_property(type_t = "string")]
        client_match: String,

        /// Whether to connect to system capture ports automatically
        #[obs_property(type_t = "bool")]
        connect_ports: bool,
    }
);

impl JackInputSourceBuilder {
    /// Set to capture from system audio input ports
    pub fn set_system_capture(self) -> Self {
        self.set_client_match("system").set_connect_ports(true)
    }
}

impl ObsSourceBuilder for JackInputSourceBuilder {}
