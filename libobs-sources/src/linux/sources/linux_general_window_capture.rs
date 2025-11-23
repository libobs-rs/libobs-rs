use libobs_wrapper::{
    data::ObsObjectBuilder,
    runtime::ObsRuntime,
    sources::ObsSourceRef,
    utils::{ObsError, SourceInfo},
};

use super::DisplayServerType;
use crate::linux::{
    sources::xcomposite_input::XCompositeInputSourceBuilder, PipeWireWindowCaptureSourceBuilder,
};

/// General Linux window capture source that automatically selects the best capture method.
///
/// This wrapper automatically chooses between:
/// - **PipeWire capture** (for Wayland - captures via desktop portal with window selection)
/// - **XComposite window capture** (for traditional X11 setups - direct window capture)
///
/// The selection is based on the detected display server type.
///
/// # Example
///
/// ```no_run
/// use libobs_sources::linux::LinuxGeneralWindowCapture;
/// use libobs_wrapper::{context::ObsContext, sources::ObsSourceBuilder, utils::StartupInfo};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let startup_info = StartupInfo::default();
/// # let mut context = ObsContext::new(startup_info)?;
/// # let mut scene = context.scene("Main Scene")?;
///
/// // Automatically selects PipeWire or XComposite based on display server
/// let capture = LinuxGeneralWindowCapture::auto_detect(
///     context.runtime().clone(),
///     "Window Capture"
/// )?;
///
/// // Add to scene
/// scene.add(&capture)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct LinuxGeneralWindowCapture {
    info: SourceInfo,
    capture_type: CaptureType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureType {
    PipeWire,
    XComposite,
}

impl LinuxGeneralWindowCapture {
    /// Create a window capture source by auto-detecting the display server type.
    ///
    /// This is the recommended way to create a window capture on Linux.
    #[must_use = "Use the 'add_to_scene' method to add the source to a scene"]
    pub fn auto_detect(
        runtime: ObsRuntime,
        name: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let display_type = DisplayServerType::detect();
        Self::new(runtime, name, display_type)
    }

    /// Create a window capture source for a specific display server type.
    ///
    /// # Arguments
    ///
    /// * `runtime` - The OBS runtime
    /// * `name` - Name for the source
    /// * `display_type` - The display server type to create a source for
    pub fn new(
        runtime: ObsRuntime,
        name: &str,
        display_type: DisplayServerType,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if display_type.prefer_pipewire() {
            Self::new_pipewire(runtime, name)
        } else {
            Self::new_xcomposite(runtime, name)
        }
    }

    /// Create a PipeWire-based window capture source.
    ///
    /// Note: On Wayland, window selection is handled by the desktop portal
    /// which will prompt the user to select a window.
    pub fn new_pipewire(
        runtime: ObsRuntime,
        name: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let builder = PipeWireWindowCaptureSourceBuilder::new(name, runtime.clone())?;
        let info = builder.set_show_cursor(true).build()?;
        Ok(LinuxGeneralWindowCapture {
            info,
            capture_type: CaptureType::PipeWire,
        })
    }

    /// Create an XComposite-based window capture source.
    ///
    /// # Arguments
    ///
    /// * `runtime` - The OBS runtime
    /// * `name` - Name for the source
    pub fn new_xcomposite(
        runtime: ObsRuntime,
        name: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let builder = XCompositeInputSourceBuilder::new(name, runtime.clone())?;
        let info = builder.set_show_cursor(true).build()?;
        Ok(LinuxGeneralWindowCapture {
            info,
            capture_type: CaptureType::XComposite,
        })
    }

    /// Create an XComposite-based window capture for a specific window.
    ///
    /// # Arguments
    ///
    /// * `runtime` - The OBS runtime
    /// * `name` - Name for the source
    /// * `window_id` - The X11 window ID to capture
    pub fn new_xcomposite_with_window(
        runtime: ObsRuntime,
        name: &str,
        window_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let builder = XCompositeInputSourceBuilder::new(name, runtime.clone())?;
        let info = builder
            .set_capture_window(window_id.to_string())
            .set_show_cursor(true)
            .build()?;
        Ok(LinuxGeneralWindowCapture {
            info,
            capture_type: CaptureType::XComposite,
        })
    }

    pub fn add_to_scene(
        self,
        scene: &mut libobs_wrapper::scenes::ObsSceneRef,
    ) -> Result<ObsSourceRef, ObsError> {
        scene.add_source(self.info)
    }

    /// Get the type of capture being used.
    pub fn capture_type_name(&self) -> &str {
        match self.capture_type {
            CaptureType::PipeWire => "PipeWire",
            CaptureType::XComposite => "XComposite",
        }
    }
}

impl AsRef<SourceInfo> for LinuxGeneralWindowCapture {
    fn as_ref(&self) -> &SourceInfo {
        &self.info
    }
}
