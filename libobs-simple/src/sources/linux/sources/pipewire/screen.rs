use crate::sources::linux::pipewire::{impl_pipewire_source_builder, ObsPipeWireSourceType};
use libobs_simple_macro::obs_object_builder;

#[obs_object_builder("pipewire-screen-capture-source")]
/// This struct is used to build a PipeWire screen capture source (so window + desktop capture).
pub struct PipeWireScreenCaptureSourceBuilder {
    /// Restore token for reconnecting to previous sessions
    #[obs_property(type_t = "string", settings_key = "RestoreToken")]
    restore_token: String,

    /// Whether to show cursor (for screen capture)
    #[obs_property(type_t = "bool", settings_key = "ShowCursor")]
    show_cursor: bool,
}

impl_pipewire_source_builder!(
    PipeWireScreenCaptureSourceBuilder,
    ObsPipeWireSourceType::ScreenCapture
);
