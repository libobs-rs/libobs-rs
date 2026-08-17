//! OBS Context Management
//!
//! This module provides the core functionality for interacting with libobs.
//! The primary type is [`ObsContext`], which serves as the main entry point for
//! all OBS operations.
//!
//! # Overview
//!
//! The `ObsContext` represents an initialized OBS environment and provides methods to:
//! - Initialize the OBS runtime
//! - Create and manage scenes
//! - Create and manage outputs (recording, streaming)
//! - Access and configure video/audio settings
//! - Download and bootstrap OBS binaries at runtime
//!
//! # Thread Safety
//!
//! OBS operations must be performed on a single thread. The `ObsContext` handles
//! this requirement by creating a dedicated thread for OBS operations and providing
//! a thread-safe interface to interact with it.
//!
//! # Examples
//!
//! Creating a basic OBS context:
//!
//! ```no_run
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use libobs_wrapper::context::ObsContext;
//! use libobs_wrapper::utils::StartupInfo;
//!
//! let info = StartupInfo::default();
//! let context = ObsContext::new(info)?;
//! # Ok(())
//! # }
//! ```
//!
//! For more examples refer to the [examples](https://github.com/libobs-rs/libobs-rs/tree/main/examples) directory in the repository.

mod registry;

use std::{
    ffi::CStr,
    sync::{Arc, Condvar, Mutex, RwLock},
    thread::ThreadId,
};

#[cfg(target_os = "linux")]
use crate::utils::initialization::PlatformType;
use crate::{
    data::output::{ObsOutputTrait, ObsOutputTraitSealed, ObsReplayBufferOutputRef},
    display::{ObsDisplayCreationData, ObsDisplayRef},
};
use crate::{
    data::{output::ObsOutputRef, video::ObsVideoInfo, ObsData},
    enums::{ObsLogLevel, ObsResetVideoStatus},
    logger::LOGGER,
    run_with_obs,
    runtime::ObsRuntime,
    scenes::ObsSceneRef,
    sources::{ObsFilterRef, ObsSourceBuilder},
    unsafe_send::Sendable,
    utils::{FilterInfo, ObsError, ObsModules, ObsString, OutputInfo, StartupInfo},
};
use getters0::Getters;
use libobs::{audio_output, video_output};
use registry::ObjectRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObsRuntimeState {
    Idle,
    Starting,
    Active(ThreadId),
    ShuttingDown,
}

lazy_static::lazy_static! {
    static ref OBS_RUNTIME_STATE: (Mutex<ObsRuntimeState>, Condvar) =
        (Mutex::new(ObsRuntimeState::Idle), Condvar::new());
}

pub(crate) fn reserve_runtime_slot() -> Result<(), ObsError> {
    let (lock, changed) = &*OBS_RUNTIME_STATE;
    let mut state = lock.lock().map_err(|_| ObsError::MutexFailure)?;
    loop {
        match *state {
            ObsRuntimeState::Idle => {
                *state = ObsRuntimeState::Starting;
                return Ok(());
            }
            ObsRuntimeState::ShuttingDown => {
                state = changed.wait(state).map_err(|_| ObsError::MutexFailure)?;
            }
            ObsRuntimeState::Starting | ObsRuntimeState::Active(_) => {
                return Err(ObsError::ThreadFailure);
            }
        }
    }
}

pub(crate) fn cancel_runtime_start() {
    let (lock, changed) = &*OBS_RUNTIME_STATE;
    let mut state = match lock.lock() {
        Ok(state) => state,
        Err(poisoned) => poisoned.into_inner(),
    };
    if *state == ObsRuntimeState::Starting {
        *state = ObsRuntimeState::Idle;
        changed.notify_all();
    }
}

pub(crate) fn activate_runtime_slot(thread_id: ThreadId) -> Result<(), ObsError> {
    let (lock, _) = &*OBS_RUNTIME_STATE;
    let mut state = lock.lock().map_err(|_| ObsError::MutexFailure)?;
    if *state != ObsRuntimeState::Starting {
        return Err(ObsError::ThreadFailure);
    }
    *state = ObsRuntimeState::Active(thread_id);
    Ok(())
}

pub(crate) fn begin_runtime_shutdown() {
    let (lock, _) = &*OBS_RUNTIME_STATE;
    match lock.lock() {
        Ok(mut state) => {
            if matches!(*state, ObsRuntimeState::Active(_)) {
                *state = ObsRuntimeState::ShuttingDown;
            }
        }
        Err(poisoned) => {
            let mut state = poisoned.into_inner();
            *state = ObsRuntimeState::ShuttingDown;
        }
    }
}

pub(crate) fn release_runtime_slot() -> Result<(), ObsError> {
    let (lock, changed) = &*OBS_RUNTIME_STATE;
    let mut state = lock.lock().map_err(|_| ObsError::MutexFailure)?;
    *state = ObsRuntimeState::Idle;
    changed.notify_all();
    Ok(())
}

/// Interface to the process-global OBS context.
///
/// Context-level native objects are owned by an internal registry. Each native handle
/// retains the runtime independently, so correctness does not depend on Rust struct
/// field declaration order.
#[derive(Debug, Getters, Clone)]
#[skip_new]
pub struct ObsContext {
    /// Keeps C-referenced startup configuration alive for the OBS lifetime.
    startup_info: Arc<RwLock<StartupInfo>>,

    #[skip_getter]
    objects: Arc<ObjectRegistry>,

    #[skip_getter]
    _obs_modules: Arc<ObsModules>,

    runtime: ObsRuntime,

    #[cfg(target_os = "linux")]
    glib_loop: Arc<RwLock<Option<crate::utils::linux::LinuxGlibLoop>>>,
}

impl ObsContext {
    /// Checks if the installed OBS version matches the expected version.
    /// Returns true if the major version matches, false otherwise.
    pub fn check_version_compatibility() -> bool {
        // Safety: This is fine, we are just getting a version string, which doesn't allocate any memory or have side effects.
        unsafe {
            #[allow(unknown_lints)]
            #[allow(ensure_obs_call_in_runtime)]
            let version = libobs::obs_get_version_string();
            if version.is_null() {
                return false;
            }

            let version_str = match CStr::from_ptr(version).to_str() {
                Ok(s) => s,
                Err(_) => return false,
            };

            let version_parts: Vec<&str> = version_str.split('.').collect();
            if version_parts.len() != 3 {
                return false;
            }

            let major = match version_parts[0].parse::<u64>() {
                Ok(v) => v,
                Err(_) => return false,
            };

            major == libobs::LIBOBS_API_MAJOR_VER as u64
        }
    }

    pub fn builder() -> StartupInfo {
        StartupInfo::new()
    }

    /// Initializes libobs on the current thread.
    ///
    /// Note that there can be only one ObsContext
    /// initialized at a time. This is because
    /// libobs is not completely thread-safe.
    ///
    /// Also note that this might leak a very tiny
    /// amount of memory. As a result, it is
    /// probably a good idea not to restart the
    /// OBS context repeatedly over a very long
    /// period of time. Unfortunately the memory
    /// leak is caused by a bug in libobs itself.
    ///
    /// On Linux, make sure to call `ObsContext::check_version_compatibility` before
    /// initializing the context. If that method returns false, it may be possible for the binary to crash.
    ///
    /// If initialization fails, an `ObsError` is returned.
    pub fn new(info: StartupInfo) -> Result<ObsContext, ObsError> {
        log::trace!("Getting version number...");

        #[allow(unknown_lints)]
        #[allow(ensure_obs_call_in_runtime)]
        // Safety: This is fine, we are just getting a version number, which does not require
        // to be on the OBS thread.
        let version_numb = unsafe { libobs::obs_get_version() };
        if version_numb == 0 {
            return Err(ObsError::InvalidDll);
        }

        // Spawning runtime, I'll keep this as function for now
        let (runtime, obs_modules, info) = ObsRuntime::startup(info)?;
        #[cfg(target_os = "linux")]
        let linux_opt = if info.start_glib_loop {
            Some(crate::utils::linux::LinuxGlibLoop::new())
        } else {
            None
        };

        Ok(Self {
            _obs_modules: Arc::new(obs_modules),
            objects: Arc::new(ObjectRegistry::default()),
            runtime: runtime.clone(),
            startup_info: Arc::new(RwLock::new(info)),
            #[cfg(target_os = "linux")]
            glib_loop: Arc::new(RwLock::new(linux_opt)),
        })
    }

    #[cfg(target_os = "linux")]
    pub fn get_platform(&self) -> Result<PlatformType, ObsError> {
        self.runtime.get_platform()
    }

    pub fn get_version(&self) -> Result<String, ObsError> {
        Self::get_version_global()
    }

    pub fn get_version_global() -> Result<String, ObsError> {
        unsafe {
            #[allow(unknown_lints)]
            #[allow(ensure_obs_call_in_runtime)]
            // Safety: This is fine, it just returns a globally allocated variable
            let version = libobs::obs_get_version_string();
            if version.is_null() {
                return Err(ObsError::NullPointer(Some(
                    "OBS version string".to_string(),
                )));
            }
            let version_cstr = CStr::from_ptr(version);
            Ok(version_cstr.to_string_lossy().into_owned())
        }
    }

    pub fn log(&self, level: ObsLogLevel, msg: &str) {
        let mut log = LOGGER
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        log.log(level, msg.to_string());
    }

    /// Resets the OBS video context. This is often called
    /// when one wants to change a setting related to the
    /// OBS video info sent on startup.
    ///
    /// It is important to register your video encoders to
    /// a video handle after you reset the video context
    /// if you are using a video handle other than the
    /// main video handle. For convenience, this function
    /// sets all video encoder back to the main video handler
    /// by default.
    ///
    /// Note that you cannot reset the graphics module
    /// without destroying the entire OBS context. Trying
    /// so will result in an error.
    pub fn reset_video(&mut self, ovi: ObsVideoInfo) -> Result<(), ObsError> {
        // You cannot change the graphics module without
        // completely destroying the entire OBS context.
        if self
            .startup_info
            .read()
            .map_err(|_| {
                ObsError::LockError("Failed to acquire read lock on startup info".to_string())
            })?
            .obs_video_info
            .graphics_module()
            != ovi.graphics_module()
        {
            return Err(ObsError::ResetVideoFailureGraphicsModule);
        }

        if self.objects.any_output_active()? {
            return Err(ObsError::ResetVideoFailureOutputActive);
        }

        // Resets the video context. Note that this
        // is similar to Self::reset_video, but it
        // does not call that function because the
        // ObsContext struct is not created yet,
        // and also because there is no need to free
        // anything tied to the OBS context.
        let vid_ptr = Sendable(ovi.as_ptr());
        let reset_video_status = run_with_obs!(self.runtime, (vid_ptr), move || unsafe {
            // Safety: OVI is still in scope, so the pointer is valid as well.
            libobs::obs_reset_video(vid_ptr.0)
        })?;

        let reset_video_status = num_traits::FromPrimitive::from_i32(reset_video_status);

        let reset_video_status = match reset_video_status {
            Some(x) => x,
            None => ObsResetVideoStatus::Failure,
        };

        if reset_video_status == ObsResetVideoStatus::Success {
            self.startup_info
                .write()
                .map_err(|_| {
                    ObsError::LockError("Failed to acquire write lock on startup info".to_string())
                })?
                .obs_video_info = ovi;

            Ok(())
        } else {
            Err(ObsError::ResetVideoFailure(reset_video_status))
        }
    }

    /// Returns a pointer to the video output.
    ///
    /// # Safety
    /// This function is unsafe because it returns a raw pointer that must be handled carefully. Only use this pointer if you REALLY know what you are doing.
    pub unsafe fn get_video_ptr(&self) -> Result<*mut video_output, ObsError> {
        // Removed safeguards here because ptr are not sendable and this OBS context should never be used across threads
        run_with_obs!(self.runtime, || unsafe {
            // Safety: This can be called as long as OBS hasn't shutdown, which it hasn't.
            Sendable(libobs::obs_get_video())
        })
        .map(|ptr| ptr.0)
    }

    /// Returns a pointer to the audio output.
    ///
    /// # Safety
    /// This function is unsafe because it returns a raw pointer that must be handled carefully. Only use this pointer if you REALLY know what you are doing.
    pub unsafe fn get_audio_ptr(&self) -> Result<*mut audio_output, ObsError> {
        // Removed safeguards here because ptr are not sendable and this OBS context should never be used across threads
        run_with_obs!(self.runtime, || unsafe {
            // Safety: This can be called as long as OBS hasn't shutdown, which it hasn't.
            Sendable(libobs::obs_get_audio())
        })
        .map(|ptr| ptr.0)
    }

    pub fn data(&self) -> Result<ObsData, ObsError> {
        ObsData::new(self.runtime.clone())
    }

    pub fn replay_buffer(
        &mut self,
        info: OutputInfo,
    ) -> Result<ObsReplayBufferOutputRef, ObsError> {
        let output = ObsReplayBufferOutputRef::new(info, self.runtime.clone());

        match output {
            Ok(x) => {
                let tmp = x.clone();
                self.objects.add_output(x)?;
                Ok(tmp)
            }

            Err(x) => Err(x),
        }
    }

    pub fn output(&mut self, info: OutputInfo) -> Result<ObsOutputRef, ObsError> {
        let output = ObsOutputRef::new(info, self.runtime.clone());

        match output {
            Ok(x) => {
                let tmp = x.clone();
                self.objects.add_output(x)?;
                Ok(tmp)
            }

            Err(x) => Err(x),
        }
    }

    pub fn obs_filter(&mut self, info: FilterInfo) -> Result<ObsFilterRef, ObsError> {
        let filter = ObsFilterRef::new(
            info.id,
            info.name,
            info.settings,
            info.hotkey_data,
            self.runtime.clone(),
        );

        match filter {
            Ok(x) => {
                let tmp = x.clone();
                self.objects.add_filter(x)?;
                Ok(tmp)
            }

            Err(x) => Err(x),
        }
    }

    /// Creates a new display and returns its ID.
    ///
    /// You must call `update_color_space` on the display when the window is moved, resized or the display settings change.
    ///
    /// Note: When calling `set_size` or `set_pos`, `update_color_space` is called automatically.
    ///
    /// Another note: On Linux, this method is unsafe because you must ensure that every display reference is dropped before your window exits.
    #[cfg(not(target_os = "linux"))]
    pub fn display(&mut self, data: ObsDisplayCreationData) -> Result<ObsDisplayRef, ObsError> {
        self.inner_display_fn(data)
    }

    /// Creates a new display and returns its ID.
    ///
    /// You must call `update_color_space` on the display when the window is moved, resized or the display settings change.
    ///
    /// # Safety
    /// All references of the `ObsDisplayRef` **MUST** be dropped before your window closes, otherwise you **will** have crashes.
    /// This includes calling `remove_display` or `remove_display_by_id` to remove the display from the context.
    ///
    /// Also on X11, make sure that the provided window handle was created using the same display as the one provided in the `NixDisplay` in the `StartupInfo`.
    ///
    /// Note: When calling `set_size` or `set_pos`, `update_color_space` is called automatically.
    #[cfg(target_os = "linux")]
    pub unsafe fn display(
        &mut self,
        data: ObsDisplayCreationData,
    ) -> Result<ObsDisplayRef, ObsError> {
        self.inner_display_fn(data)
    }

    /// This function is used internally to create displays.
    fn inner_display_fn(
        &mut self,
        data: ObsDisplayCreationData,
    ) -> Result<ObsDisplayRef, ObsError> {
        #[cfg(target_os = "linux")]
        {
            // We'll need to check if a custom display was provided because libobs will crash if the display didn't create the window the user is giving us
            // X11 allows having a separate display however.
            let nix_display = self
                .startup_info
                .read()
                .map_err(|_| {
                    ObsError::LockError("Failed to acquire read lock on startup info".to_string())
                })?
                .nix_display
                .clone();

            let is_wayland_handle = data.window_handle.is_wayland;
            if is_wayland_handle && nix_display.is_none() {
                return Err(ObsError::DisplayCreationError(
                    "Wayland window handle provided but no NixDisplay was set in StartupInfo."
                        .to_string(),
                ));
            }

            if let Some(nix_display) = &nix_display {
                if is_wayland_handle {
                    match nix_display {
                        crate::utils::NixDisplay::X11(_display) => {
                            return Err(ObsError::DisplayCreationError(
                                "Provided NixDisplay is X11, but the window handle is Wayland."
                                    .to_string(),
                            ));
                        }
                        crate::utils::NixDisplay::Wayland(display) => {
                            use crate::utils::linux::wl_proxy_get_display;
                            if !data.window_handle.is_wayland {
                                return Err(ObsError::DisplayCreationError(
                            "Provided window handle is not a Wayland handle, but the NixDisplay is Wayland.".to_string(),
                        ));
                            }

                            let surface_handle = data.window_handle.window.0.display;
                            let display_from_surface = unsafe {
                                // Safety: The display handle is valid as long as the surface is valid.
                                wl_proxy_get_display(surface_handle)
                            };
                            if let Err(e) = display_from_surface {
                                log::warn!("Could not get display from surface handle on wayland. Make sure your wayland client is at least version 1.23. Error: {:?}", e);
                            } else if let Ok(display_from_surface) = display_from_surface {
                                if display_from_surface != display.as_ptr() {
                                    return Err(ObsError::DisplayCreationError(
                            "Provided surface handle's Wayland display does not match the NixDisplay's Wayland display.".to_string(),
                        ));
                                }
                            }
                        }
                    }
                }
            }
        }

        let display = ObsDisplayRef::new(data, self.runtime.clone())
            .map_err(|e| ObsError::DisplayCreationError(e.to_string()))?;

        self.objects.add_display(display.clone())?;
        Ok(display)
    }

    pub fn remove_display(&mut self, display: &ObsDisplayRef) -> Result<(), ObsError> {
        self.remove_display_by_id(display.id())
    }

    pub fn remove_display_by_id(&mut self, id: usize) -> Result<(), ObsError> {
        self.objects.remove_display(id)
    }

    pub fn get_display_by_id(&self, id: usize) -> Result<Option<ObsDisplayRef>, ObsError> {
        self.objects.display(id)
    }

    pub fn get_output(&self, name: &str) -> Result<Option<Arc<dyn ObsOutputTrait>>, ObsError> {
        self.objects.output(name)
    }

    pub fn update_output(&self, name: &str, settings: ObsData) -> Result<(), ObsError> {
        self.objects
            .output(name)?
            .ok_or(ObsError::OutputNotFound)?
            .update_settings(settings)
    }

    pub fn get_filter(&self, name: &str) -> Result<Option<ObsFilterRef>, ObsError> {
        self.objects.filter(name)
    }

    /// Creates a new scene
    ///
    /// If the channel is provided, the scene will be set to that output channel.
    ///
    /// There are 64 channels that you can assign scenes to,
    /// which will draw on top of each other in ascending index order
    /// when a output is rendered.
    ///
    /// # Arguments
    /// * `name` - The name of the scene. This must be unique.
    /// * `channel` - Optional channel to bind the scene to. If provided, the scene will be set as active for that channel.
    ///
    /// # Returns
    /// A Result containing the new ObsSceneRef or an error
    pub fn scene<T: Into<ObsString> + Send + Sync>(
        &mut self,
        name: T,
        channel: Option<u32>,
    ) -> Result<ObsSceneRef, ObsError> {
        let scene = ObsSceneRef::new(name.into(), self.runtime.clone())?;

        let tmp = scene.clone();
        self.objects.add_scene(scene)?;

        if let Some(channel) = channel {
            tmp.set_to_channel(channel)?;
        }
        Ok(tmp)
    }

    pub fn get_scene(&self, name: &str) -> Result<Option<ObsSceneRef>, ObsError> {
        self.objects.scene(name)
    }

    pub fn source_builder<T: ObsSourceBuilder, K: Into<ObsString> + Send + Sync>(
        &self,
        name: K,
    ) -> Result<T, ObsError> {
        T::new(name.into(), self.runtime.clone())
    }
}
