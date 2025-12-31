use libobs_wrapper::{data::StringEnum, sources::ObsSourceRef};

use crate::sources::macro_helper::{define_object_manager, impl_default_builder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Describes the X11 capture server type
pub enum ObsX11ServerType {
    /// Local X11 server
    Local,
    /// Custom X11 server
    Custom,
}

impl StringEnum for ObsX11ServerType {
    fn to_str(&self) -> &str {
        match self {
            ObsX11ServerType::Local => "local",
            ObsX11ServerType::Custom => "custom",
        }
    }
}

define_object_manager!(
    #[derive(Debug)]
    /// A source to capture X11 screen/window content.
    ///
    /// This source provides screen capture functionality on Linux systems running X11.
    /// It can capture the entire screen or specific areas with cropping options.
    struct X11CaptureSource("xshm_input") for ObsSourceRef {
        /// Screen/Display to capture
        #[obs_property(type_t = "int")]
        screen: i64,

        /// Whether to show the cursor in the capture
        #[obs_property(type_t = "bool")]
        show_cursor: bool,

        /// Enable advanced settings
        #[obs_property(type_t = "bool")]
        advanced: bool,

        /// X Server to connect to (when using advanced settings)
        #[obs_property(type_t = "string")]
        server: String,

        /// Crop from top (in pixels)
        #[obs_property(type_t = "int")]
        cut_top: i64,

        /// Crop from left (in pixels)
        #[obs_property(type_t = "int")]
        cut_left: i64,

        /// Crop from right (in pixels)
        #[obs_property(type_t = "int")]
        cut_right: i64,

        /// Crop from bottom (in pixels)
        #[obs_property(type_t = "int")]
        cut_bot: i64,
    }
);

impl_default_builder!(X11CaptureSourceBuilder);
