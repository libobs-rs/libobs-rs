use libobs_wrapper::sources::{ObsSourceBuilder, ObsSourceRef};

use crate::macro_helper::define_object_manager;

define_object_manager!(
    #[derive(Debug)]
    /// A source to capture X11 windows using XComposite.
    ///
    /// This source provides window capture functionality on Linux systems running X11
    /// using the XComposite extension. It can capture individual windows with their
    /// transparency and effects intact.
    struct XCompositeInputSource("xcomposite_input") for ObsSourceRef {
        /// Window to capture (window ID as string)
        #[obs_property(type_t = "string")]
        capture_window: String,

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

        /// Whether to show the cursor in the capture
        #[obs_property(type_t = "bool")]
        show_cursor: bool,

        /// Include window border/decorations
        #[obs_property(type_t = "bool")]
        include_border: bool,

        /// Exclude alpha channel (disable transparency)
        #[obs_property(type_t = "bool")]
        exclude_alpha: bool,
    }
);

impl ObsSourceBuilder for XCompositeInputSourceBuilder {}
