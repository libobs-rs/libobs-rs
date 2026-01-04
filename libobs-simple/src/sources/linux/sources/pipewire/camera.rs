use libobs_simple_macro::obs_object_builder;
use libobs_wrapper::{
    data::ObsObjectBuilder,
    sources::{ObsSourceBuilder, ObsSourceRef},
};

#[derive(Debug)]
/// A source for PipeWire camera capture via camera portal.
///
/// This source captures video from camera devices through PipeWire's camera portal,
/// providing secure access to camera devices in sandboxed environments.
#[obs_object_builder("pipewire-camera-source")]
pub struct PipeWireCameraSourceBuilder {
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

impl ObsSourceBuilder for PipeWireCameraSourceBuilder {
    type T = ObsSourceRef;

    fn build(self) -> Result<Self::T, libobs_wrapper::utils::ObsError>
    where
        Self: Sized,
    {
        let runtime = self.runtime.clone();
        let info = self.object_build()?;

        let source = ObsSourceRef::new_from_info(info, runtime)?;
        Ok(source)
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
