use libobs_wrapper::sources::{ObsSourceBuilder, ObsSourceRef};

use crate::macro_helper::define_object_manager;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// PipeWire source type
pub enum ObsPipeWireSourceType {
    /// Screen capture via desktop portal
    DesktopCapture,
    /// Camera capture via camera portal  
    CameraCapture,
}

define_object_manager!(
    #[derive(Debug)]
    /// A source for PipeWire screen/camera capture.
    ///
    /// PipeWire is a modern multimedia framework for Linux that handles audio and video.
    /// This source can capture screen content through the desktop portal or camera
    /// content through the camera portal, providing sandboxed capture capabilities.
    struct PipeWireCaptureSource("pipewire-desktop-capture-source") for ObsSourceRef {
        /// Restore token for reconnecting to previous sessions
        #[obs_property(type_t = "string")]
        restore_token: String,

        /// Portal session token
        #[obs_property(type_t = "string")]
        session_token: String,

        /// Whether to show cursor (for screen capture)
        #[obs_property(type_t = "bool")]
        show_cursor: bool,
    }
);

define_object_manager!(
    #[derive(Debug)]
    /// A source for PipeWire camera capture via camera portal.
    ///
    /// This source captures video from camera devices through PipeWire's camera portal,
    /// providing secure access to camera devices in sandboxed environments.
    struct PipeWireCameraSource("pipewire-camera-source") for ObsSourceRef {
        /// Camera device node (e.g., "/dev/video0")
        #[obs_property(type_t = "string")]
        camera_id: String,

        /// Video format (FOURCC as string)
        #[obs_property(type_t = "string")]
        video_format: String,

        /// Resolution as "width x height"
        #[obs_property(type_t = "string")]
        resolution: String,

        /// Framerate as "num/den"
        #[obs_property(type_t = "string")]
        framerate: String,
    }
);

impl PipeWireCaptureSourceBuilder {
    /// Enable cursor capture for screen recording
    pub fn with_cursor(self) -> Self {
        self.set_show_cursor(true)
    }
}

impl PipeWireCameraSourceBuilder {
    /// Set resolution using width and height values
    pub fn set_resolution_values(self, width: u32, height: u32) -> Self {
        self.set_resolution(format!("{}x{}", width, height))
    }

    /// Set framerate using numerator and denominator
    pub fn set_framerate_values(self, num: u32, den: u32) -> Self {
        self.set_framerate(format!("{}/{}", num, den))
    }
}

impl ObsSourceBuilder for PipeWireCaptureSourceBuilder {}
impl ObsSourceBuilder for PipeWireCameraSourceBuilder {}
