use libobs_simple_macro::obs_object_builder;

use crate::sources::linux::pipewire::{impl_pipewire_source_builder, ObsPipeWireSourceType};

#[obs_object_builder("pipewire-desktop-capture-source")]
pub struct PipeWireDesktopCaptureSourceBuilder {
    /// Restore token for reconnecting to previous sessions
    #[obs_property(type_t = "string", settings_key = "RestoreToken")]
    restore_token: String,

    /// Whether to show cursor (for screen capture)
    #[obs_property(type_t = "bool", settings_key = "ShowCursor")]
    show_cursor: bool,

    /// Existing PipeWire node id to capture directly. A value greater than zero can bypass
    /// the desktop portal picker when the loaded `linux-pipewire` plugin supports the
    /// `ConnectNode` extension. Zero keeps the normal portal flow.
    #[obs_property(type_t = "int", settings_key = "ConnectNode")]
    connect_node: i64,
}

impl_pipewire_source_builder!(
    PipeWireDesktopCaptureSourceBuilder,
    ObsPipeWireSourceType::DesktopCapture
);
