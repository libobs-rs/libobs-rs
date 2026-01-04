//! Monitor capture source for Windows using libobs-rs
//! This source captures the entire monitor and is used for screen recording.

use std::sync::Arc;

use super::ObsDisplayCaptureMethod;
use crate::error::ObsSimpleError;
use crate::{define_object_manager, sources::macro_helper::impl_custom_source};
/// Note: This does not update the capture method directly, instead the capture method gets
/// stored in the struct. The capture method is being set to WGC at first, then the source is created and then the capture method is updated to the desired method.
use display_info::DisplayInfo;
use libobs_simple_macro::obs_object_impl;
use libobs_wrapper::run_with_obs;
use libobs_wrapper::runtime::ObsRuntime;
use libobs_wrapper::scenes::{SceneItemExtSceneTrait, SceneItemRef};
use libobs_wrapper::{
    data::{ObsObjectBuilder, ObsObjectUpdater},
    scenes::ObsSceneRef,
    sources::{ObsSourceBuilder, ObsSourceRef, ObsSourceTrait},
    unsafe_send::Sendable,
    utils::ObsError,
};
use num_traits::ToPrimitive;
use windows::Win32::UI::HiDpi::{
    GetAwarenessFromDpiAwarenessContext, GetThreadDpiAwarenessContext, DPI_AWARENESS_UNAWARE,
};

// Usage example
define_object_manager!(
    /// Provides an easy-to-use builder for the monitor capture source.
    #[derive(Debug)]
    struct MonitorCaptureSource("monitor_capture", *mut libobs::obs_source) for ObsSourceRef {
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

fn is_thread_dpi_unaware(runtime: &ObsRuntime) -> Result<bool, ObsError> {
    run_with_obs!(runtime, (), move || {
        unsafe {
            // Safety: This function can be called from any thread.
            let ctx = GetThreadDpiAwarenessContext();
            GetAwarenessFromDpiAwarenessContext(ctx) == DPI_AWARENESS_UNAWARE
        }
    })
}

impl<'a> MonitorCaptureSourceUpdater<'a> {
    pub fn set_capture_method(mut self, method: ObsDisplayCaptureMethod) -> Result<Self, ObsError> {
        if is_thread_dpi_unaware(self.runtime())? && method == ObsDisplayCaptureMethod::MethodDXGI {
            log::warn!("You are trying to capture the monitor using the DXGI capture method while the current thread is DPI unaware. This will lead to a black screen being captured. Please ensure that your application is DPI aware before using the DXGI capture method.");
            return Err(ObsError::InvalidOperation(
                "Cannot use DXGI capture method when the current thread is DPI unaware.".into(),
            ));
        }
        self.get_settings_updater()
            .set_int_ref("method", method.to_i32().unwrap() as i64);

        Ok(self)
    }
}

impl MonitorCaptureSourceBuilder {
    /// Sets the capture method for the monitor capture source.
    /// If you want to use DXGI, it is required for your application to be DPI aware.
    pub fn set_capture_method(mut self, method: ObsDisplayCaptureMethod) -> Self {
        self.capture_method = Some(method);

        self
    }
}

pub type GeneralSourceRef = Arc<Box<dyn ObsSourceTrait>>;
impl ObsSourceBuilder for MonitorCaptureSourceBuilder {
    type T = MonitorCaptureSource;

    fn build(self) -> Result<Self::T, ObsError>
    where
        Self: Sized,
    {
        if is_thread_dpi_unaware(self.runtime())?
            && self.capture_method == Some(ObsDisplayCaptureMethod::MethodDXGI)
        {
            log::warn!("You are trying to capture the monitor using the DXGI capture method while the current thread is DPI unaware. This will lead to a black screen being captured. Please ensure that your application is DPI aware before using the DXGI capture method.");
            return Err(ObsError::InvalidOperation(
                "Cannot use DXGI capture method when the current thread is DPI unaware.".into(),
            ));
        }

        let runtime = self.runtime.clone();
        let obj_info = self.object_build()?;

        let res = ObsSourceRef::new_from_info(obj_info, runtime)?;
        MonitorCaptureSource::new(res)
    }

    fn add_to_scene(mut self, scene: &mut ObsSceneRef) -> Result<SceneItemRef<Self::T>, ObsError>
    where
        Self: Sized,
    {
        // Because of a black screen bug, we need to set the method to WGC first and then update
        self.get_settings_updater().set_int_ref(
            "method",
            ObsDisplayCaptureMethod::MethodWgc.to_i32().unwrap() as i64,
        );

        let method_to_set = self.capture_method;

        let mut res = self.build()?;
        let scene_item = scene.add_source(res.clone())?;

        if let Some(method) = method_to_set {
            res.create_updater()?
                .set_capture_method(method)? //
                .update()?;
        }

        Ok(scene_item)
    }
}

impl_custom_source!(MonitorCaptureSource);
