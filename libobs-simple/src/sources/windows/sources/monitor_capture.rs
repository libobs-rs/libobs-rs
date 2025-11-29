//! Monitor capture source for Windows using libobs-rs
//! This source captures the entire monitor and is used for screen recording.

use super::ObsDisplayCaptureMethod;
use crate::define_object_manager;
use crate::error::ObsSimpleError;
/// Note: This does not update the capture method directly, instead the capture method gets
/// stored in the struct. The capture method is being set to WGC at first, then the source is created and then the capture method is updated to the desired method.
use display_info::DisplayInfo;
use libobs_simple_macro::obs_object_impl;
use libobs_wrapper::{
    data::{ObsObjectBuilder, ObsObjectUpdater},
    scenes::ObsSceneRef,
    sources::{ObsSourceBuilder, ObsSourceRef},
    unsafe_send::Sendable,
    utils::ObsError,
};
use num_traits::ToPrimitive;

// Usage example
define_object_manager!(
    /// Provides an easy-to-use builder for the monitor capture source.
    #[derive(Debug)]
    struct MonitorCaptureSource("monitor_capture") for ObsSourceRef {
        #[obs_property(type_t = "string", settings_key = "monitor_id")]
        monitor_id_raw: String,

        #[obs_property(type_t = "bool")]
        /// Sets whether the cursor should be captured.
        capture_cursor: bool,

        #[obs_property(type_t = "bool")]
        /// Compatibility mode for the monitor capture source.
        compatibility: bool,

        #[obs_property(type_t = "bool")]
        /// If the capture should force SDR
        force_sdr: bool,

        capture_method: Option<ObsDisplayCaptureMethod>,
    }
);

#[obs_object_impl]
impl MonitorCaptureSource {
    /// Gets all available monitors
    pub fn get_monitors() -> Result<Vec<Sendable<DisplayInfo>>, ObsSimpleError> {
        Ok(DisplayInfo::all()
            .map_err(ObsSimpleError::DisplayInfoError)?
            .into_iter()
            .map(Sendable)
            .collect())
    }

    pub fn set_monitor(self, monitor: &Sendable<DisplayInfo>) -> Self {
        self.set_monitor_id_raw(monitor.0.name.as_str())
    }
}

impl<'a> MonitorCaptureSourceUpdater<'a> {
    pub fn set_capture_method(mut self, method: ObsDisplayCaptureMethod) -> Self {
        self.get_settings_updater()
            .set_int_ref("method", method.to_i32().unwrap() as i64);

        self
    }
}

impl MonitorCaptureSourceBuilder {
    /// Sets the capture method for the monitor capture source.
    /// Only MethodWgc works for now as the other DXGI method does not work and only records a black screen (Failed to DuplicateOutput1)
    /// Workaround for black screen bug: [issue](https://github.com/libobs-rs/libobs-rs/issues/5)
    pub fn set_capture_method(mut self, method: ObsDisplayCaptureMethod) -> Self {
        self.capture_method = Some(method);
        self
    }
}

impl ObsSourceBuilder for MonitorCaptureSourceBuilder {
    fn add_to_scene(mut self, scene: &mut ObsSceneRef) -> Result<ObsSourceRef, ObsError>
    where
        Self: Sized,
    {
        // Because of a black screen bug, we need to set the method to WGC first and then update
        self.get_settings_updater().set_int_ref(
            "method",
            ObsDisplayCaptureMethod::MethodWgc.to_i32().unwrap() as i64,
        );

        let method_to_set = self.capture_method;
        let runtime = self.runtime.clone();

        let b = self.build()?;
        let mut res = scene.add_source(b)?;

        if let Some(method) = method_to_set {
            MonitorCaptureSourceUpdater::create_update(runtime, &mut res)?
                .set_capture_method(method)
                .update()?;
        }

        Ok(res)
    }
}
