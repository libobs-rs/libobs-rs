use libobs_wrapper::{
    data::StringEnum,
    sources::{ObsSourceBuilder, ObsSourceRef},
};
use num_derive::{FromPrimitive, ToPrimitive};

use crate::macro_helper::define_object_manager;

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
/// Video color range for V4L2 input
pub enum ObsV4L2ColorRange {
    /// Default color range
    Default = 0,
    /// Partial color range (limited)
    Partial = 1,
    /// Full color range
    Full = 2,
}

impl StringEnum for ObsV4L2ColorRange {
    fn to_str(&self) -> &str {
        match self {
            ObsV4L2ColorRange::Default => "Default",
            ObsV4L2ColorRange::Partial => "Partial",
            ObsV4L2ColorRange::Full => "Full",
        }
    }
}

define_object_manager!(
    #[derive(Debug)]
    /// A source for Video4Linux2 (V4L2) camera input.
    ///
    /// This source captures video from V4L2 compatible devices such as webcams,
    /// capture cards, and other video input devices on Linux.
    struct V4L2InputSource("v4l2_input") for ObsSourceRef {
        /// Device ID/path (e.g., "/dev/video0")
        #[obs_property(type_t = "string")]
        device_id: String,

        /// Input number on the device
        #[obs_property(type_t = "int")]
        input: i64,

        /// Pixel format (FOURCC code as integer)
        #[obs_property(type_t = "int")]
        pixelformat: i64,

        /// Video standard for analog inputs
        #[obs_property(type_t = "int")]
        standard: i64,

        /// DV timing for digital inputs
        #[obs_property(type_t = "int")]
        dv_timing: i64,

        #[obs_property(type_t = "int")]
        resolution: i64,

        #[obs_property(type_t = "int")]
        framerate: i64,

        /// Color range setting
        #[obs_property(type_t = "int")]
        color_range: i64,

        /// Auto-reset on timeout
        #[obs_property(type_t = "bool")]
        auto_reset: bool,

        /// Frames until timeout
        #[obs_property(type_t = "int")]
        timeout_frames: i64,
    }
);

impl V4L2InputSourceBuilder {
    /// Set the color range using the enum
    pub fn set_color_range_enum(self, color_range: ObsV4L2ColorRange) -> Self {
        use num_traits::ToPrimitive;
        self.set_color_range(color_range.to_i64().unwrap())
    }
}

impl ObsSourceBuilder for V4L2InputSourceBuilder {}
