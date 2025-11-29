use crate::define_object_manager;

use super::{ObsWindowCaptureMethod, ObsWindowPriority};
use crate::error::ObsSimpleError;
use libobs_simple_macro::obs_object_impl;
#[cfg(feature = "window-list")]
use libobs_window_helper::{get_all_windows, WindowInfo, WindowSearchMode};
use libobs_wrapper::{
    data::{ObsObjectBuilder, ObsObjectUpdater},
    scenes::ObsSceneRef,
    sources::{ObsSourceBuilder, ObsSourceRef},
    utils::ObsError,
};
use num_traits::ToPrimitive;

define_object_manager!(
    /// Provides an easy-to-use builder for the window capture source.
    #[derive(Debug)]
    struct WindowCaptureSource("window_capture") for ObsSourceRef {

    /// Sets the priority of the window capture source.
    /// Used to determine in which order windows are searched for.
    ///
    /// # Arguments
    ///
    /// * `priority` - The priority of the window capture source.
    ///
    /// # Returns
    ///
    /// The updated `WindowCaptureSourceBuilder` instance.
    #[obs_property(type_t = "enum")]
    priority: ObsWindowPriority,

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

    #[obs_property(type_t = "bool")]
    /// Sets whether the cursor should be captured
    cursor: bool,

    /// Whether to capture audio from window source (BETA) <br>
    /// When enabled, creates an "Application Audio Capture" source that automatically updates to the currently captured window/application. <br>
    /// Note that if Desktop Audio is configured, this could result in doubled audio.
    capture_audio: bool,

    #[obs_property(type_t = "bool")]
    /// Whether to force SDR color space for the window capture source.
    force_sdr: bool,

    #[obs_property(type_t = "bool")]
    /// Whether to capture the window's client area only (without borders, title bar and the main menu bar).
    client_area: bool,

    #[obs_property(type_t = "bool")]
    compatibility: bool,

    capture_method: Option<ObsWindowCaptureMethod>,
});

#[cfg(feature = "window-list")]
#[libobs_simple_macro::obs_object_impl]
impl WindowCaptureSource {
    /// Gets a list of windows that can be captured by this source.
    pub fn get_windows(
        mode: WindowSearchMode,
    ) -> Result<Vec<libobs_wrapper::unsafe_send::Sendable<WindowInfo>>, ObsSimpleError> {
        Ok(get_all_windows(mode)
            .map_err(ObsSimpleError::WindowHelperError)?
            .into_iter()
            .map(libobs_wrapper::unsafe_send::Sendable)
            .collect())
    }

    /// Sets the window to capture.
    ///
    /// # Arguments
    ///
    /// * `window` - The window to capture. A list of available windows can be retrieved using `WindowCaptureSourceBuilder::get_windows`
    ///
    /// # Returns
    ///
    /// The updated `WindowCaptureSourceBuilder` instance.
    pub fn set_window(self, window: &libobs_wrapper::unsafe_send::Sendable<WindowInfo>) -> Self {
        self.set_window_raw(window.0.obs_id.as_str())
    }
}

impl<'a> WindowCaptureSourceUpdater<'a> {
    pub fn set_capture_method(mut self, method: ObsWindowCaptureMethod) -> Self {
        self.get_settings_updater()
            .set_int_ref("method", method.to_i32().unwrap() as i64);

        self
    }
}

impl WindowCaptureSourceBuilder {
    /// Sets the capture method for the window capture source.
    pub fn set_capture_method(mut self, method: ObsWindowCaptureMethod) -> Self {
        self.capture_method = Some(method);
        self
    }
}

#[obs_object_impl]
impl WindowCaptureSource {
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

impl ObsSourceBuilder for WindowCaptureSourceBuilder {
    fn add_to_scene(mut self, scene: &mut ObsSceneRef) -> Result<ObsSourceRef, ObsError>
    where
        Self: Sized,
    {
        // Because of a black screen bug, we need to set the method to WGC first and then update (I've copied this code from the DisplayCapture source, they should have the same issue)
        self.get_settings_updater().set_int_ref(
            "method",
            ObsWindowCaptureMethod::MethodAuto.to_i32().unwrap() as i64,
        );

        let method_to_set = self.capture_method;
        let runtime = self.runtime.clone();

        let b = self.build()?;
        let mut res = scene.add_source(b)?;

        if let Some(method) = method_to_set {
            WindowCaptureSourceUpdater::create_update(runtime, &mut res)?
                .set_capture_method(method)
                .update()?;
        }

        Ok(res)
    }
}
