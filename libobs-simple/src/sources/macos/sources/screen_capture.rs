//! ScreenCaptureKit-backed screen capture for macOS.

use libobs_simple_macro::obs_object_builder;
use libobs_wrapper::{
    data::ObsObjectBuilder,
    sources::{ObsSourceBuilder, ObsSourceRef},
    utils::ObsError,
};

/// Capture mode accepted by OBS's macOS `screen_capture` source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i64)]
pub enum ScreenCaptureType {
    /// Capture an entire display.
    #[default]
    Display = 0,
    /// Capture one window by window id.
    Window = 1,
    /// Capture the visible windows belonging to one application.
    Application = 2,
}

impl ScreenCaptureType {
    pub const fn as_i64(self) -> i64 {
        self as i64
    }
}

/// Builder for OBS's native macOS ScreenCaptureKit source (`screen_capture`).
///
/// Which selector is used depends on [`ScreenCaptureType`]: `display_uuid` for display
/// capture, `window` for window capture, and `application` for application capture.
#[obs_object_builder("screen_capture")]
pub struct ScreenCaptureSourceBuilder {
    #[obs_property(type_t = "int", settings_key = "type")]
    /// Capture mode. Prefer [`ScreenCaptureSourceBuilder::set_capture_mode`].
    capture_type: i64,

    #[obs_property(type_t = "string", settings_key = "display_uuid")]
    /// UUID of the display to capture.
    display_uuid: String,

    #[obs_property(type_t = "string", settings_key = "application")]
    /// Application bundle identifier, for example `com.apple.Safari`.
    application: String,

    #[obs_property(type_t = "int", settings_key = "window")]
    /// macOS window id used in window-capture mode.
    window: i64,

    #[obs_property(type_t = "int", settings_key = "display")]
    /// Legacy numeric display id. Prefer `display_uuid` on modern OBS versions.
    display: i64,

    #[obs_property(type_t = "bool")]
    /// Include the mouse cursor in the captured video.
    show_cursor: bool,

    #[obs_property(type_t = "bool")]
    /// Capture system/application audio when supported by macOS and OBS.
    audio_capture: bool,

    #[obs_property(type_t = "bool")]
    /// Exclude OBS windows from capture.
    hide_obs: bool,

    #[obs_property(type_t = "bool")]
    /// Include otherwise hidden windows in capture selection.
    show_hidden_windows: bool,

    #[obs_property(type_t = "bool")]
    /// Include windows whose title is empty.
    show_empty_names: bool,
}

impl ScreenCaptureSourceBuilder {
    /// Set the strongly typed capture mode.
    pub fn set_capture_mode(self, capture_type: ScreenCaptureType) -> Self {
        self.set_capture_type(capture_type.as_i64())
    }
}

impl ObsSourceBuilder for ScreenCaptureSourceBuilder {
    type T = ObsSourceRef;

    fn build(self) -> Result<Self::T, ObsError> {
        let runtime = self.runtime().clone();
        ObsSourceRef::new_from_info(self.object_build()?, runtime)
    }
}

#[cfg(test)]
mod tests {
    use super::ScreenCaptureType;

    #[test]
    fn capture_type_values_match_obs_plugin_contract() {
        assert_eq!(ScreenCaptureType::Display.as_i64(), 0);
        assert_eq!(ScreenCaptureType::Window.as_i64(), 1);
        assert_eq!(ScreenCaptureType::Application.as_i64(), 2);
    }
}
