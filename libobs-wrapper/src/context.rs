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
//!
//! let context = ObsContext::builder().start()?;
//! # Ok(())
//! # }
//! ```
//!
//! For more examples refer to the [examples](https://github.com/joshprk/libobs-rs/tree/main/examples) directory in the repository.

use std::{
    collections::HashMap,
    ffi::CStr,
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
    thread::ThreadId,
};

use crate::display::{ObsDisplayCreationData, ObsDisplayRef};
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
use libobs::{audio_output, obs_scene_t, video_output};

lazy_static::lazy_static! {
    pub(crate) static ref OBS_THREAD_ID: Mutex<Option<ThreadId>> = Mutex::new(None);
}

// Note to developers of this library:
// I've updated everything in the ObsContext to use Rc and RefCell.
// Then the obs context shutdown hook is given to each children of for example scenes and displays.
// That way, obs is not shut down as long as there are still displays or scenes alive.
// This is a bit of a hack, but it works would be glad to hear your thoughts on this.

// Factor complex display map type out to satisfy clippy::type_complexity
pub(crate) type DisplayMap = HashMap<usize, Arc<Pin<Box<ObsDisplayRef>>>>;

/// Interface to the OBS context. Only one context
/// can exist across all threads and any attempt to
/// create a new context while there is an existing
/// one will error.
///
/// Note that the order of the struct values is
/// important! OBS is super specific about how it
/// does everything. Things are freed early to
/// latest from top to bottom.
#[derive(Debug, Getters, Clone)]
#[skip_new]
pub struct ObsContext {
    /// Stores startup info for safe-keeping. This
    /// prevents any use-after-free as these do not
    /// get copied in libobs.
    startup_info: Arc<RwLock<StartupInfo>>,
    #[get_mut]
    // Key is display id, value is the display fixed in heap
    displays: Arc<RwLock<DisplayMap>>,

    /// Outputs must be stored in order to prevent
    /// early freeing.
    #[allow(dead_code)]
    #[get_mut]
    pub(crate) outputs: Arc<RwLock<Vec<ObsOutputRef>>>,

    #[get_mut]
    pub(crate) scenes: Arc<RwLock<Vec<ObsSceneRef>>>,

    // Filters are on the level of the context because they are not scene specific
    #[get_mut]
    pub(crate) filters: Arc<RwLock<Vec<ObsFilterRef>>>,

    #[skip_getter]
    pub(crate) active_scene: Arc<RwLock<Option<Sendable<*mut obs_scene_t>>>>,

    #[skip_getter]
    pub(crate) _obs_modules: Arc<ObsModules>,

    /// This struct must be the last element which makes sure
    /// that everything else has been freed already before the runtime
    /// shuts down
    pub(crate) runtime: ObsRuntime,

    #[cfg(target_os = "linux")]
    pub(crate) glib_loop: Arc<RwLock<Option<crate::utils::linux::LinuxGlibLoop>>>,
}

impl ObsContext {
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
    /// If the `bootstrapper` feature is enabled, and ObsContextReturn::Restart is returned,
    /// the application must be restarted to apply the updates and initialization can not continue.
    pub fn new(info: StartupInfo) -> Result<ObsContext, ObsError> {
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
            active_scene: Default::default(),
            displays: Default::default(),
            outputs: Default::default(),
            scenes: Default::default(),
            filters: Default::default(),
            runtime,
            startup_info: Arc::new(RwLock::new(info)),

            #[cfg(target_os = "linux")]
            glib_loop: Arc::new(RwLock::new(linux_opt)),
        })
    }

    pub fn get_version(&self) -> Result<String, ObsError> {
        let res = run_with_obs!(self.runtime, || unsafe {
            let version = libobs::obs_get_version_string();
            let version_cstr = CStr::from_ptr(version);

            version_cstr.to_string_lossy().into_owned()
        })?;

        Ok(res)
    }

    pub fn log(&self, level: ObsLogLevel, msg: &str) {
        let mut log = LOGGER.lock().unwrap();
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

        let has_active_outputs = {
            self.outputs
                .read()
                .map_err(|_| {
                    ObsError::LockError("Failed to acquire read lock on outputs".to_string())
                })?
                .iter()
                .any(|output| output.is_active().unwrap_or_default())
        };

        if has_active_outputs {
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
            libobs::obs_reset_video(vid_ptr)
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
    pub unsafe fn get_video_ptr(&self) -> Result<Sendable<*mut video_output>, ObsError> {
        // Removed safeguards here because ptr are not sendable and this OBS context should never be used across threads
        run_with_obs!(self.runtime, || unsafe {
            Sendable(libobs::obs_get_video())
        })
    }

    /// Returns a pointer to the audio output.
    ///
    /// # Safety
    /// This function is unsafe because it returns a raw pointer that must be handled carefully. Only use this pointer if you REALLY know what you are doing.
    pub unsafe fn get_audio_ptr(&self) -> Result<Sendable<*mut audio_output>, ObsError> {
        // Removed safeguards here because ptr are not sendable and this OBS context should never be used across threads
        run_with_obs!(self.runtime, || unsafe {
            Sendable(libobs::obs_get_audio())
        })
    }

    pub fn data(&self) -> Result<ObsData, ObsError> {
        ObsData::new(self.runtime.clone())
    }

    pub fn output(&mut self, info: OutputInfo) -> Result<ObsOutputRef, ObsError> {
        let output = ObsOutputRef::new(info, self.runtime.clone());

        match output {
            Ok(x) => {
                let tmp = x.clone();
                self.outputs
                    .write()
                    .map_err(|_| {
                        ObsError::LockError("Failed to acquire write lock on outputs".to_string())
                    })?
                    .push(x);
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
                self.filters
                    .write()
                    .map_err(|_| {
                        ObsError::LockError("Failed to acquire write lock on filters".to_string())
                    })?
                    .push(x);
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
    pub fn display(
        &mut self,
        data: ObsDisplayCreationData,
    ) -> Result<Pin<Box<ObsDisplayRef>>, ObsError> {
        let display = ObsDisplayRef::new(data, self.runtime.clone())
            .map_err(|e| ObsError::DisplayCreationError(e.to_string()))?;

        let display_clone = display.clone();

        let id = display.id();
        self.displays
            .write()
            .map_err(|_| {
                ObsError::LockError("Failed to acquire write lock on displays".to_string())
            })?
            .insert(id, Arc::new(display));
        Ok(display_clone)
    }

    pub fn remove_display(&mut self, display: &ObsDisplayRef) -> Result<(), ObsError> {
        self.remove_display_by_id(display.id())
    }

    pub fn remove_display_by_id(&mut self, id: usize) -> Result<(), ObsError> {
        self.displays
            .write()
            .map_err(|_| {
                ObsError::LockError("Failed to acquire write lock on displays".to_string())
            })?
            .remove(&id);

        Ok(())
    }

    pub fn get_display_by_id(
        &self,
        id: usize,
    ) -> Result<Option<Arc<Pin<Box<ObsDisplayRef>>>>, ObsError> {
        let d = self
            .displays
            .read()
            .map_err(|_| {
                ObsError::LockError("Failed to acquire read lock on displays".to_string())
            })?
            .get(&id)
            .cloned();

        Ok(d)
    }

    pub fn get_output(&mut self, name: &str) -> Result<Option<ObsOutputRef>, ObsError> {
        let o = self
            .outputs
            .read()
            .map_err(|_| ObsError::LockError("Failed to acquire read lock on outputs".to_string()))?
            .iter()
            .find(|x| x.name().to_string().as_str() == name)
            .cloned();

        Ok(o)
    }

    pub fn update_output(&mut self, name: &str, settings: ObsData) -> Result<(), ObsError> {
        match self
            .outputs
            .write()
            .map_err(|_| {
                ObsError::LockError("Failed to acquire write lock on outputs".to_string())
            })?
            .iter_mut()
            .find(|x| x.name().to_string().as_str() == name)
        {
            Some(output) => output.update_settings(settings),
            None => Err(ObsError::OutputNotFound),
        }
    }

    pub fn get_filter(&mut self, name: &str) -> Result<Option<ObsFilterRef>, ObsError> {
        let f = self
            .filters
            .read()
            .map_err(|_| ObsError::LockError("Failed to acquire read lock on filters".to_string()))?
            .iter()
            .find(|x| x.name().to_string().as_str() == name)
            .cloned();

        Ok(f)
    }

    pub fn scene<T: Into<ObsString> + Send + Sync>(
        &mut self,
        name: T,
    ) -> Result<ObsSceneRef, ObsError> {
        let scene = ObsSceneRef::new(name.into(), self.active_scene.clone(), self.runtime.clone())?;

        let tmp = scene.clone();
        self.scenes
            .write()
            .map_err(|_| ObsError::LockError("Failed to acquire write lock on scenes".to_string()))?
            .push(scene);

        Ok(tmp)
    }

    pub fn get_scene(&mut self, name: &str) -> Result<Option<ObsSceneRef>, ObsError> {
        let r = self
            .scenes
            .read()
            .map_err(|_| ObsError::LockError("Failed to acquire read lock on scenes".to_string()))?
            .iter()
            .find(|x| x.name().to_string().as_str() == name)
            .cloned();
        Ok(r)
    }

    pub fn source_builder<T: ObsSourceBuilder, K: Into<ObsString> + Send + Sync>(
        &self,
        name: K,
    ) -> Result<T, ObsError> {
        T::new(name.into(), self.runtime.clone())
    }
}
