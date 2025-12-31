use libobs_simple_macro::{obs_object_builder, obs_object_updater};
use libobs_wrapper::{
    data::{object::ObsObjectTrait, ObsDataGetters},
    run_with_obs,
    sources::{ObsSourceRef, ObsSourceTrait},
    utils::ObsError,
};

use crate::sources::macro_helper::{define_object_manager, impl_default_builder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// PipeWire source type
pub enum ObsPipeWireSourceType {
    /// Screen capture via desktop portal
    DesktopCapture,
    /// Camera capture via camera portal
    CameraCapture,
}
/*
define_object_manager!(
    #[derive(Debug)]
    /// A source for PipeWire screen/camera capture.
    ///
    /// PipeWire is a modern multimedia framework for Linux that handles audio and video.
    /// This source can capture screen content through the desktop portal or camera
    /// content through the camera portal, providing sandboxed capture capabilities.
    struct PipeWireDesktopCaptureSource("pipewire-desktop-capture-source") for ObsSourceRef {
        /// Restore token for reconnecting to previous sessions
        #[obs_property(type_t = "string", settings_key="RestoreToken")]
        restore_token: String,

        /// Whether to show cursor (for screen capture)
        #[obs_property(type_t = "bool", settings_key="ShowCursor")]
        show_cursor: bool,
    }
);
 */

#[obs_object_builder("pipewire-desktop-capture-source")]
pub struct PipeWireDesktopCaptureSourceBuilder {
    /// Restore token for reconnecting to previous sessions
    #[obs_property(type_t = "string", settings_key = "RestoreToken")]
    restore_token: String,

    /// Whether to show cursor (for screen capture)
    #[obs_property(type_t = "bool", settings_key = "ShowCursor")]
    show_cursor: bool,
}

#[obs_object_updater("pipewire-desktop-capture-source", ObsSourceRef)]
pub struct PipeWireDesktopCaptureSourceUpdater {
    /// Whether to show cursor (for screen capture)
    #[obs_property(type_t = "bool", settings_key = "ShowCursor")]
    show_cursor: bool,
}

#[obs_object_builder("pipewire-window-capture-source")]
pub struct PipeWireWindowCaptureSourceBuilder {
    /// Restore token for reconnecting to previous sessions
    #[obs_property(type_t = "string", settings_key = "RestoreToken")]
    restore_token: String,

    /// Whether to show cursor (for screen capture)
    #[obs_property(type_t = "bool", settings_key = "ShowCursor")]
    show_cursor: bool,
}

#[obs_object_updater("pipewire-window-capture-source", ObsSourceRef)]
pub struct PipeWireWindowCaptureSourceUpdater {
    /// Whether to show cursor (for screen capture)
    #[obs_property(type_t = "bool", settings_key = "ShowCursor")]
    show_cursor: bool,
}

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

#[obs_object_updater("pipewire-screen-capture-source", ObsSourceRef)]
/// This struct is used to update a PipeWire screen capture source (so window + desktop capture).
pub struct PipeWireScreenCaptureSourceUpdater {
    /// Whether to show cursor (for screen capture)
    #[obs_property(type_t = "bool", settings_key = "ShowCursor")]
    show_cursor: bool,
}

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

/// This trait provides additional methods for PipeWire sources.
pub trait PipeWireSourceExtTrait {
    /// Gets the restore token used for reconnecting to previous sessions for `pipewire-desktop-capture-source` and `pipewire-window-capture-source` sources.
    ///
    /// As of right now, there is no callback or signal to notify when the token has been set, you have to call this method to get the restore token.
    ///
    /// The restore token will most probably be of `Some(String)` after the user has selected a screen or window to capture.
    fn get_restore_token(&self) -> Result<Option<String>, ObsError>;
}

impl PipeWireSourceExtTrait for ObsSourceRef {
    fn get_restore_token(&self) -> Result<Option<String>, ObsError> {
        if self.id() != "pipewire-desktop-capture-source"
            && self.id() != "pipewire-window-capture-source"
            && self.id() != "pipewire-screen-capture-source"
        {
            return Err(ObsError::InvalidOperation(format!("Can't call 'get_restore_token' on a source of id {}. Expected 'pipewire-desktop-capture-source', 'pipewire-window-capture-source' or 'pipewire-screen-capture-source'", self.id())));
        }

        let source_ptr = self.as_ptr();
        run_with_obs!(self.runtime(), (source_ptr), move || unsafe {
            libobs::obs_source_save(source_ptr);
        })?;

        let settings = self.settings()?;
        let token = settings.get_string("RestoreToken")?;
        Ok(token)
    }
}

impl PipeWireDesktopCaptureSourceBuilder {
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

impl_default_builder!(PipeWireDesktopCaptureSourceBuilder);
impl_default_builder!(PipeWireWindowCaptureSourceBuilder);
impl_default_builder!(PipeWireScreenCaptureSourceBuilder);
impl_default_builder!(PipeWireCameraSourceBuilder);
