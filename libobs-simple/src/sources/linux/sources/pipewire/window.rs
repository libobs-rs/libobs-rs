use libobs_simple_macro::obs_object_builder;

use crate::sources::linux::pipewire::{impl_pipewire_source_builder, ObsPipeWireSourceType};

#[obs_object_builder("pipewire-window-capture-source")]
pub struct PipeWireWindowCaptureSourceBuilder {
    /// Restore token for reconnecting to previous sessions
    #[obs_property(type_t = "string", settings_key = "RestoreToken")]
    restore_token: String,

    /// Whether to show cursor (for screen capture)
    #[obs_property(type_t = "bool", settings_key = "ShowCursor")]
    show_cursor: bool,
}

impl_pipewire_source_builder!(
    PipeWireWindowCaptureSourceBuilder,
    ObsPipeWireSourceType::WindowCapture
);
