use libobs_wrapper::{
    data::ObsObjectBuilder,
    runtime::ObsRuntime,
    sources::ObsSourceRef,
    utils::{ObsError, SourceInfo},
};
use std::env;

use crate::sources::linux::{
    sources::x11_capture::X11CaptureSourceBuilder, PipeWireDesktopCaptureSourceBuilder,
};

/// Display server type detection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayServerType {
    /// Wayland display server
    Wayland,
    /// X11/Xorg display server
    X11,
    /// Unknown or undetected display server
    Unknown,
}

impl DisplayServerType {
    /// Detect the current display server type using environment variables.
    ///
    /// Checks in order:
    /// 1. `XDG_SESSION_TYPE` (most reliable)
    /// 2. `WAYLAND_DISPLAY` (indicates Wayland)
    /// 3. `DISPLAY` (indicates X11)
    pub fn detect() -> Self {
        // First, check XDG_SESSION_TYPE (most reliable)
        if let Ok(session_type) = env::var("XDG_SESSION_TYPE") {
            let session_type = session_type.to_lowercase();
            if session_type.contains("wayland") {
                return DisplayServerType::Wayland;
            } else if session_type.contains("x11") {
                return DisplayServerType::X11;
            }
        }

        // Check WAYLAND_DISPLAY (if set, we're on Wayland)
        if env::var("WAYLAND_DISPLAY").is_ok() {
            return DisplayServerType::Wayland;
        }

        // Check DISPLAY (if set and no Wayland indicators, we're on X11)
        if env::var("DISPLAY").is_ok() {
            return DisplayServerType::X11;
        }

        DisplayServerType::Unknown
    }

    /// Returns whether PipeWire should be preferred for this display server.
    ///
    /// PipeWire is the modern capture API and works on both X11 and Wayland,
    /// but is essential for Wayland and optional for X11.
    pub fn prefer_pipewire(&self) -> bool {
        match self {
            DisplayServerType::Wayland => true, // PipeWire is required for Wayland
            DisplayServerType::X11 => false,    // X11 has native capture
            DisplayServerType::Unknown => true, // Default to PipeWire for safety
        }
    }
}

/// General Linux screen capture source that automatically selects the best capture method.
///
/// This wrapper automatically chooses between:
/// - **PipeWire capture** (for Wayland or modern Linux setups)
/// - **X11 screen capture** (for traditional X11 setups)
///
/// The selection is based on the detected display server type.
///
/// # Example
///
/// ```no_run
/// use libobs_simple::sources::linux::LinuxGeneralScreenCapture;
/// use libobs_wrapper::{context::ObsContext, sources::ObsSourceBuilder, utils::StartupInfo};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let startup_info = StartupInfo::default();
/// # let mut context = ObsContext::new(startup_info)?;
/// # let mut scene = context.scene("Main Scene", Some(0))?;
///
/// // Automatically selects PipeWire or X11 based on display server
/// let capture = LinuxGeneralScreenCapture::auto_detect(
///     &mut context,
///     "Screen Capture"
/// )?;
///
/// // Add to scene
/// scene.add(&capture)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct LinuxGeneralScreenCapture {
    info: SourceInfo,
    capture_type: CaptureType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureType {
    PipeWire,
    X11,
}

impl LinuxGeneralScreenCapture {
    /// Create a screen capture source by auto-detecting the display server type.
    ///
    /// This is the recommended way to create a screen capture on Linux.
    //TODO rework this API, the best way to use this is to create a SourceBuilder that does the detection internally
    pub fn auto_detect(
        runtime: ObsRuntime,
        name: &str,
        restore_token: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let display_type = DisplayServerType::detect();
        Self::new(runtime, name, display_type, restore_token)
    }

    /// Create a screen capture source for a specific display server type.
    ///
    /// # Arguments
    ///
    /// * `runtime` - The OBS runtime
    /// * `name` - Name for the source
    /// * `display_type` - The display server type to create a source for
    /// * `restore_token` - Optional restore token for restoring source settings (this is only for pipewire)
    pub fn new(
        runtime: ObsRuntime,
        name: &str,
        display_type: DisplayServerType,
        restore_token: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if display_type.prefer_pipewire() {
            Self::new_pipewire(runtime, name, restore_token)
        } else {
            Self::new_x11(runtime, name)
        }
    }

    /// Create a PipeWire-based screen capture source.
    pub fn new_pipewire(
        runtime: ObsRuntime,
        name: &str,
        restore_token: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let builder =
            PipeWireDesktopCaptureSourceBuilder::new(name, runtime.clone())?.set_show_cursor(true);
        let builder = if let Some(token) = restore_token {
            builder.set_restore_token(token)
        } else {
            builder
        };

        let info = builder.build()?;
        Ok(LinuxGeneralScreenCapture {
            info,
            capture_type: CaptureType::PipeWire,
        })
    }

    /// Create an X11-based screen capture source.
    pub fn new_x11(runtime: ObsRuntime, name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let builder = X11CaptureSourceBuilder::new(name, runtime.clone())?;
        let info = builder.set_show_cursor(true).set_screen(0).build()?;
        Ok(LinuxGeneralScreenCapture {
            info,
            capture_type: CaptureType::X11,
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
            CaptureType::X11 => "X11",
        }
    }
}

impl AsRef<SourceInfo> for LinuxGeneralScreenCapture {
    fn as_ref(&self) -> &SourceInfo {
        &self.info
    }
}
