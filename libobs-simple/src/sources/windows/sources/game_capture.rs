use libobs_simple_macro::obs_object_impl;
#[cfg(feature = "window-list")]
use libobs_window_helper::{get_all_windows, WindowInfo, WindowSearchMode};
use libobs_wrapper::{
    data::{ObsObjectBuilder, ObsObjectUpdater, StringEnum},
    sources::{ObsSourceBuilder, ObsSourceRef},
};

use super::{ObsHookRate, ObsWindowPriority};
use crate::define_object_manager;
use crate::error::ObsSimpleError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Describes the capture mode of the game capture source.
pub enum ObsGameCaptureMode {
    /// Captures any fullscreen application
    Any,
    /// Captures a specific window, specified under the `window` property
    CaptureSpecificWindow,
    /// CApture the foreground window when a hotkey is pressed
    CaptureForegroundWindow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObsGameCaptureRgbaSpace {
    /// sRGB color space
    SRgb,
    /// Rec. 2100 (PQ)
    RGBA2100pq,
}

impl StringEnum for ObsGameCaptureRgbaSpace {
    fn to_str(&self) -> &str {
        match self {
            ObsGameCaptureRgbaSpace::SRgb => "sRGB",
            ObsGameCaptureRgbaSpace::RGBA2100pq => "Rec. 2100 (PQ)",
        }
    }
}

impl StringEnum for ObsGameCaptureMode {
    fn to_str(&self) -> &str {
        match self {
            ObsGameCaptureMode::Any => "any_fullscreen",
            ObsGameCaptureMode::CaptureSpecificWindow => "window",
            ObsGameCaptureMode::CaptureForegroundWindow => "hotkey",
        }
    }
}

define_object_manager!(
    #[derive(Debug)]
    /// A source to capture a game or fullscreen application.
    ///
    ///
    /// Use `GameCaptureSourceBuilder::get_windows` to get a list of windows that can be captured (feature `window-list` needs to be enabled).
    /// Requires OBS to be running with administrator privileges to capture certain games.
    ///
    /// ## Important Notice
    /// This source fails to capture if another instance (OBS studio, another instance of your program, etc.) has a game capture source for the same game/application active.
    /// If the window can be captured can be checked using `GameCaptureSourceBuilder::is_window_in_use_by_other_instance` (feature `window-list` needs to be enabled).
    struct GameCaptureSource("game_capture") for ObsSourceRef {
        /// Sets the capture mode for the game capture source. Look at doc for `ObsGameCaptureMode`
        #[obs_property(type_t = "enum_string")]
        capture_mode: ObsGameCaptureMode,

        /// Sets the window to capture.
        ///
        /// # Arguments
        ///
        /// * `window` - The window to capture, represented as `ObsString`. Must be in the format of an obs window id
        ///
        /// # Returns
        ///
        /// The updated `WindowCaptureSourceBuilder` instance.
        #[obs_property(type_t = "string", settings_key = "window")]
        window_raw: String,

        #[obs_property(type_t = "enum")]
        /// Window Match Priority
        priority: ObsWindowPriority,

        #[obs_property(type_t = "bool")]
        /// SLI/Crossfire Capture Mode (Slow)
        sli_compatability: bool,

        #[obs_property(type_t = "bool")]
        /// Whether the cursor should be captured
        capture_cursor: bool,

        #[obs_property(type_t = "bool")]
        /// If transparency of windows should be allowed
        allow_transparency: bool,

        #[obs_property(type_t = "bool")]
        /// Premultiplied Alpha
        premultiplied_alpha: bool,

        /// Limit capture framerate
        #[obs_property(type_t = "bool")]
        limit_framerate: bool,

        /// Capture third party overlays (such as steam overlays)
        #[obs_property(type_t = "bool")]
        capture_overlays: bool,

        /// Use anti-cheat compatibility hook
        #[obs_property(type_t = "bool")]
        anti_cheat_hook: bool,

        /// Hook rate (Ranging from slow to fastest)
        #[obs_property(type_t = "enum")]
        hook_rate: ObsHookRate,

        /// The color space to capture in
        #[obs_property(type_t = "enum_string")]
        rgb10a2_space: ObsGameCaptureRgbaSpace,

        /// Whether to capture audio from window source (BETA) <br>
        /// When enabled, creates an "Application Audio Capture" source that automatically updates to the currently captured window/application. <br>
        /// Note that if Desktop Audio is configured, this could result in doubled audio.
        capture_audio: bool,
    }
);

#[cfg(feature = "window-list")]
impl GameCaptureSourceBuilder {
    /// Gets a list of windows that can be captured by this source.
    pub fn get_windows(mode: WindowSearchMode) -> Result<Vec<WindowInfo>, ObsSimpleError> {
        get_all_windows(mode)
            .map(|e| e.into_iter().filter(|x| x.is_game).collect::<Vec<_>>())
            .map_err(ObsSimpleError::WindowHelperError)
    }

    /// Checks if a window with the given process ID can be captured by this source.
    /// This does not guarantee that the window can be captured, only that it is not black
    pub fn is_window_in_use_by_other_instance(window_pid: u32) -> std::io::Result<bool> {
        use libobs_window_helper::is_window_in_use_by_other_instance;

        is_window_in_use_by_other_instance(window_pid)
    }

    /// Sets the window to capture.
    ///
    /// # Arguments
    ///
    /// * `window` - The window to capture. A list of available windows can be retrieved using `GameCaptureSourceBuilder::get_windows`
    ///
    /// # Returns
    ///
    /// The updated `GameCaptureSourceBuilder` instance.
    pub fn set_window(self, window: &WindowInfo) -> Self {
        self.set_window_raw(window.obs_id.as_str())
    }
}

#[obs_object_impl]
impl GameCaptureSource {
    pub fn set_capture_audio(mut self, capture_audio: bool) -> Result<Self, ObsSimpleError> {
        use crate::sources::windows::audio_capture_available;

        if capture_audio && !audio_capture_available() {
            return Err(ObsSimpleError::FeatureNotAvailable(
                "Game Audio Capture is not available on this system",
            ));
        }

        self.get_settings_updater()
            .set_bool_ref("capture_audio", capture_audio);

        Ok(self)
    }
}

impl ObsSourceBuilder for GameCaptureSourceBuilder {}
