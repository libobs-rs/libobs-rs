//! Runtime management for safe OBS API access across threads
//!
//! This module provides the core thread management functionality for the libobs-wrapper.
//! It ensures that OBS API calls are always executed on the same thread, as required by
//! the OBS API, while still allowing application code to interact with OBS from any thread.
//!
//! # Thread Safety
//!
//! The OBS C API is not thread-safe and requires that all operations occur on the same thread.
//! The `ObsRuntime` struct creates a dedicated thread for all OBS operations and manages
//! message passing between application threads and the OBS thread.
//!
//! # Blocking APIs
//!
//! The runtime locking APIs:
//! - By default all operations are synchronous
//!
//! # Example
//!
//! ```no_run
//! use libobs_wrapper::runtime::ObsRuntime;
//! use libobs_wrapper::utils::StartupInfo;
//!
//! fn example() {
//!     // Assuming that the OBS context is already initialized
//!
//!     // Run an operation on the OBS thread
//!     let runtime = context.runtime();

//!     runtime.run_with_obs(|| {
//!         // This code runs on the OBS thread
//!         println!("Running on OBS thread");
//!     }).unwrap();
//! }
//! ```

#[cfg(feature = "enable_runtime")]
use std::any;
use std::ffi::CStr;
use std::rc::Rc;
use std::sync::Arc;
use std::{ptr, thread};

use crate::context::ObsContext;
use crate::crash_handler::main_crash_handler;
use crate::enums::{ObsLogLevel, ObsResetVideoStatus};
use crate::logger::{extern_log_callback, internal_log_global, LOGGER};
#[cfg(target_os = "linux")]
use crate::run_with_obs;
use crate::utils::initialization::{platform_specific_setup, PlatformSpecificGuard};
use crate::utils::{ObsError, ObsModules, ObsString};
use crate::{context::OBS_THREAD_ID, utils::StartupInfo};

#[cfg(feature = "enable_runtime")]
use crate::unsafe_send::Sendable;
use std::fmt::Debug;
#[cfg(feature = "enable_runtime")]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "enable_runtime")]
use std::sync::mpsc::{channel, Sender};
#[cfg(feature = "enable_runtime")]
use std::sync::Mutex;
#[cfg(feature = "enable_runtime")]
use std::thread::JoinHandle;

/// Command type for operations to perform on the OBS thread
#[cfg(feature = "enable_runtime")]
enum ObsCommand {
    /// Execute a function on the OBS thread and send result back if sender is provided
    Execute(
        Box<dyn FnOnce() -> Box<dyn any::Any + Send> + Send>,
        Option<oneshot::Sender<Box<dyn any::Any + Send>>>,
    ),
    /// Signal the OBS thread to terminate
    Terminate,
}

/// Core runtime that manages the OBS thread
///
/// This struct represents the runtime environment for OBS operations.
/// It creates and manages a dedicated thread for OBS API calls to
/// ensure thread safety while allowing interaction from any thread.
///
/// # Thread Safety
///
/// `ObsRuntime` can be safely cloned and shared across threads. All operations
/// are automatically dispatched to the dedicated OBS thread.
///
/// # Lifecycle Management
///
/// When the last `ObsRuntime` instance is dropped, the OBS thread is automatically
/// shut down and all OBS resources are properly released.
#[derive(Debug, Clone)]
pub struct ObsRuntime {
    #[cfg(feature = "enable_runtime")]
    command_sender: Arc<Sender<ObsCommand>>,
    #[cfg(feature = "enable_runtime")]
    queued_commands: Arc<AtomicUsize>,
    thread_id: std::thread::ThreadId,
    _guard: Arc<_ObsRuntimeGuard>,

    #[cfg(not(feature = "enable_runtime"))]
    _platform_specific: Option<Rc<PlatformSpecificGuard>>,
}

impl ObsRuntime {
    /// Initializes the OBS runtime.
    ///
    /// This function starts up OBS on a dedicated thread and prepares it for use.
    /// It handles bootstrapping (if configured), OBS initialization, module loading,
    /// and setup of audio/video subsystems.
    ///
    /// # Parameters
    ///
    /// * `options` - The startup configuration for OBS
    ///
    /// # Returns
    ///
    /// A `Result` containing:
    /// - `(ObsRuntime, ObsModules, StartupInfo)`: The initialized runtime, loaded modules, and startup info.
    /// - `ObsError`: If initialization fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use libobs_wrapper::runtime::{ObsRuntime, ObsRuntimeReturn};
    /// use libobs_wrapper::utils::StartupInfo;
    ///
    /// fn initialize() {
    ///     let startup_info = StartupInfo::default();
    ///     match ObsRuntime::startup(startup_info) {
    ///         Ok((runtime, modules, info)) => {
    ///             // Use the initialized runtime
    ///         },
    ///         Err(e) => {
    ///             // Handle initialization error
    ///         }
    ///     }
    /// }
    /// ```
    #[allow(unused_mut)]
    pub(crate) fn startup(
        mut options: StartupInfo,
    ) -> Result<(ObsRuntime, ObsModules, StartupInfo), ObsError> {
        // Check if OBS is already running on another thread
        let obs_id = OBS_THREAD_ID.lock().map_err(|_e| ObsError::MutexFailure)?;
        if obs_id.is_some() {
            return Err(ObsError::ThreadFailure);
        }

        drop(obs_id);

        log::trace!("Initializing OBS context");
        ObsRuntime::init(options)
            .map_err(|e| ObsError::Unexpected(format!("Failed to initialize OBS runtime: {:?}", e)))
    }

    /// Internal initialization method
    ///
    /// Creates the OBS thread and performs core initialization.
    #[cfg(not(feature = "enable_runtime"))]
    fn init(info: StartupInfo) -> Result<(ObsRuntime, ObsModules, StartupInfo), ObsError> {
        let (startup, mut modules, platform_specific) = unsafe { Self::initialize_inner(info)? };

        let runtime = Self {
            thread_id: thread::current().id(),
            _guard: Arc::new(_ObsRuntimeGuard {}),
            _platform_specific: platform_specific,
        };

        modules.runtime = Some(runtime.clone());
        Ok((runtime, modules, startup))
    }

    /// Internal initialization method
    ///
    /// Creates the OBS thread and performs core initialization.
    #[cfg(feature = "enable_runtime")]
    fn init(info: StartupInfo) -> Result<(ObsRuntime, ObsModules, StartupInfo), ObsError> {
        static RUNTIME_THREAD_NAME: &str = "libobs-wrapper-obs-runtime";

        let (command_sender, command_receiver) = channel();
        let (init_tx, init_rx) = oneshot::channel();
        let queued_commands = Arc::new(AtomicUsize::new(0));

        let queued_commands_clone = queued_commands.clone();
        let handle = std::thread::Builder::new()
            .name(RUNTIME_THREAD_NAME.to_string())
            .spawn(move || {
                log::trace!("Starting OBS thread");

                let res = unsafe {
                    // Safety: This is safe to can because we are in the dedicated OBS thread.
                    Self::initialize_inner(info)
                };

                match res {
                    Ok((info, modules, _platform_specific_guard)) => {
                        log::trace!("OBS context initialized successfully");

                        let e = init_tx.send(Ok((Sendable(modules), info)));
                        if let Err(err) = e {
                            log::error!("Failed to send initialization signal: {:?}", err);
                        }

                        // Process commands until termination
                        while let Ok(command) = command_receiver.recv() {
                            match command {
                                ObsCommand::Execute(func, result_sender) => {
                                    let result = func();
                                    if let Some(result_sender) = result_sender {
                                        let _ = result_sender.send(result);
                                    }

                                    queued_commands_clone.fetch_sub(1, Ordering::SeqCst);
                                }
                                ObsCommand::Terminate => break,
                            }
                        }

                        let r = unsafe {
                            // Safety: We are in the OBS thread, so it's safe to call shutdown here.
                            Self::shutdown_inner()
                        };
                        if let Err(err) = r {
                            log::error!("Failed to shut down OBS context: {:?}", err);
                        }
                    }
                    Err(err) => {
                        log::error!("Failed to initialize OBS context: {:?}", err);
                        let _ = init_tx.send(Err(err));
                    }
                }
            })
            .map_err(|_e| ObsError::ThreadFailure)?;

        log::trace!("Waiting for OBS thread to initialize");
        // Wait for initialization to complete
        let (mut m, info) = init_rx.recv().map_err(|_| {
            ObsError::RuntimeChannelError("Failed to receive initialization result".to_string())
        })??;

        let thread_id = handle.thread().id();
        let handle = Arc::new(Mutex::new(Some(handle)));
        let command_sender = Arc::new(command_sender);
        let runtime = Self {
            command_sender: command_sender.clone(),
            thread_id,
            queued_commands,
            _guard: Arc::new(_ObsRuntimeGuard {
                handle,
                command_sender,
            }),
        };

        m.0.runtime = Some(runtime.clone());
        Ok((runtime, m.0, info))
    }

    /// Executes an operation on the OBS thread *without* blocking. This method *will not wait* for the result.
    ///
    /// # Parameters
    ///
    /// * `operation` - A function to execute on the OBS thread
    ///
    /// # Returns
    ///
    /// A `Result` indicating whether the operation was successfully dispatched
    ///
    /// # Examples
    ///
    /// ```
    /// use libobs_wrapper::runtime::ObsRuntime;
    ///
    /// async fn example(runtime: &ObsRuntime) {
    ///     runtime.run_with_obs(|| {
    ///         // This code runs on the OBS thread
    ///         println!("Hello from the OBS thread!");
    ///     }).await.unwrap();
    /// }
    /// ```
    #[cfg(feature = "enable_runtime")]
    pub fn run_with_obs_no_block<F>(&self, operation: F) -> Result<(), ObsError>
    where
        F: FnOnce() + Send + 'static,
    {
        let is_within_runtime = std::thread::current().id() == self.thread_id;

        if is_within_runtime {
            operation();

            return Ok(());
        }

        let val = self.queued_commands.fetch_add(1, Ordering::SeqCst);
        if val > 50 {
            log::warn!("More than 50 queued commands. Try to batch them together.");
        }

        let wrapper = move || -> Box<dyn std::any::Any + Send> {
            operation();
            Box::new(())
        };

        self.command_sender
            .send(ObsCommand::Execute(Box::new(wrapper), None))
            .map_err(|_| {
                ObsError::RuntimeChannelError("Failed to send command to OBS thread".to_string())
            })?;

        Ok(())
    }

    /// Because you have the `enable_runtime` feature disabled, this is a no-op function and will still block. This is just so the run_with_obs macro works.
    #[cfg(not(feature = "enable_runtime"))]
    pub fn run_with_obs_no_block<F>(&self, operation: F) -> Result<(), ObsError>
    where
        F: FnOnce() + 'static,
    {
        let is_within_runtime = std::thread::current().id() == self.thread_id;
        if !is_within_runtime {
            return Err(ObsError::RuntimeOutsideThread);
        }

        operation();
        return Ok(());
    }

    /// No-Op function, as you have the runtime disabled. This is just so the run_with_obs macro still works
    #[cfg(not(feature = "enable_runtime"))]
    pub fn run_with_obs_result<F, T>(&self, operation: F) -> Result<T, ObsError>
    where
        F: FnOnce() -> T,
    {
        let is_within_runtime = std::thread::current().id() == self.thread_id;
        if !is_within_runtime {
            return Err(ObsError::RuntimeOutsideThread);
        }

        Ok(operation())
    }

    /// Executes an operation on the OBS thread, waits for the call to finish and returns a result
    ///
    /// This method dispatches a task to the OBS thread and blocks and waits for the result.
    ///
    /// # Parameters
    ///
    /// * `operation` - A function to execute on the OBS thread
    ///
    /// # Returns
    ///
    /// A `Result` containing the value returned by the operation
    ///
    /// # Examples
    ///
    /// ```
    /// use libobs_wrapper::runtime::ObsRuntime;
    ///
    /// async fn example(runtime: &ObsRuntime) {
    ///     let version = runtime.run_with_obs_result(|| {
    ///         // This code runs on the OBS thread
    ///         unsafe { libobs::obs_get_version_string() }
    ///     }).await.unwrap();
    ///
    ///     println!("OBS Version: {:?}", version);
    /// }
    /// ```
    #[cfg(feature = "enable_runtime")]
    pub fn run_with_obs_result<F, T>(&self, operation: F) -> Result<T, ObsError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let is_within_runtime = std::thread::current().id() == self.thread_id;
        if is_within_runtime {
            let result = operation();
            return Ok(result);
        }
        let (tx, rx) = oneshot::channel();

        // Create a wrapper closure that boxes the result as Any
        let wrapper = move || -> Box<dyn std::any::Any + Send> {
            let result = operation();
            Box::new(result)
        };

        let val = self.queued_commands.fetch_add(1, Ordering::SeqCst);
        if val > 50 {
            log::warn!("More than 50 queued commands. Try to batch them together.");
        }

        self.command_sender
            .send(ObsCommand::Execute(Box::new(wrapper), Some(tx)))
            .map_err(|_| {
                ObsError::RuntimeChannelError("Failed to send command to OBS thread".to_string())
            })?;

        let result = rx.recv().map_err(|_| {
            ObsError::RuntimeChannelError("OBS thread dropped the response channel".to_string())
        })?;

        // Downcast the Any type back to T
        let res = result.downcast::<T>().map(|boxed| *boxed).map_err(|_| {
            ObsError::RuntimeChannelError(
                "Failed to downcast result to the expected type".to_string(),
            )
        })?;

        Ok(res)
    }

    /// Initializes the libobs context and prepares it for recording.
    ///
    /// This method handles core OBS initialization including:
    /// - Starting up the OBS core (`obs_startup`)
    /// - Resetting video and audio subsystems
    /// - Loading OBS modules
    ///
    /// # Parameters
    ///
    /// * `info` - The startup configuration for OBS
    ///
    /// # Returns
    ///
    /// A `Result` containing the updated startup info and loaded modules, or an error
    ///
    /// # Safety
    /// This function must be called within the OBS runtime context to ensure thread safety.
    #[allow(unknown_lints)]
    #[allow(ensure_obs_call_in_runtime)]
    unsafe fn initialize_inner(
        mut info: StartupInfo,
    ) -> Result<(StartupInfo, ObsModules, Option<Rc<PlatformSpecificGuard>>), ObsError> {
        // Checks that there are no other threads
        // using libobs using a static Mutex.
        //
        // Fun fact: this code caused a huge debate
        // about whether AtomicBool is UB or whatever
        // in the Rust Programming Discord server.
        // I didn't read too closely into it because
        // they were talking about what architecture
        // fridges have or something.
        //
        // Since this function is not meant to be
        // high-performance or called a thousand times,
        // a Mutex is fine here.#
        let mut mutex_value = OBS_THREAD_ID.lock().map_err(|_e| ObsError::MutexFailure)?;

        // Directly checks if the value of the
        // Mutex is false. If true, then error.
        // We've checked already but keeping this
        if (*mutex_value).is_some() {
            return Err(ObsError::ThreadFailure);
        }

        // If the Mutex is None, then change
        // it to current thread ID so that no
        // other thread can use libobs while
        // the current thread is using it.
        *mutex_value = Some(thread::current().id());

        // Install DLL blocklist hook here

        #[cfg(windows)]
        unsafe {
            // Safety: We are in the OBS thread, so it's safe to call this here.
            libobs::obs_init_win32_crash_handler();
        }

        // Set logger, load debug privileges and crash handler
        unsafe {
            // Safety: We are in the OBS thread, so it's safe to call this here.
            libobs::base_set_crash_handler(Some(main_crash_handler), std::ptr::null_mut());
        }

        let native = unsafe {
            // Safety: We are in the OBS thread and the nix_display can only be set
            platform_specific_setup(info.nix_display.clone())?
        };
        unsafe {
            // Safety: We are in the OBS thread, so it's safe to call this here.
            libobs::base_set_log_handler(Some(extern_log_callback), std::ptr::null_mut());
        }

        let mut log_callback = LOGGER.lock().map_err(|_e| ObsError::MutexFailure)?;

        *log_callback = info.logger.take().expect("Logger can never be null");
        drop(log_callback);

        // Locale will only be used internally by
        // libobs for logging purposes, making it
        // unnecessary to support other languages.
        let locale_str = ObsString::new("en-US");
        let startup_status = unsafe {
            // Safety: All pointers are valid here.
            libobs::obs_startup(locale_str.as_ptr().0, ptr::null(), ptr::null_mut())
        };

        let version = unsafe { libobs::obs_get_version_string() };
        let version_cstr = unsafe { CStr::from_ptr(version) };
        let version_str = version_cstr.to_string_lossy().into_owned();

        internal_log_global(ObsLogLevel::Info, format!("OBS {}", version_str));

        // Check version compatibility
        if !ObsContext::check_version_compatibility() {
            internal_log_global(
                ObsLogLevel::Warning,
                "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!".to_string(),
            );
            internal_log_global(
                ObsLogLevel::Warning,
                format!(
                    "OBS major version mismatch: installed version is {}, but expected major version {}. Expect crashes or bugs!!",
                    version_str,
                    libobs::LIBOBS_API_MAJOR_VER
                ),
            );
            internal_log_global(
                ObsLogLevel::Warning,
                "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!".to_string(),
            );
        }

        internal_log_global(
            ObsLogLevel::Info,
            "---------------------------------".to_string(),
        );

        if !startup_status {
            return Err(ObsError::Failure);
        }

        let mut obs_modules = unsafe {
            // Safety: This is running in the OBS thread, so it's safe to call this here.
            ObsModules::add_paths(&info.startup_paths)
        };

        // Note that audio is meant to only be reset
        // once. See the link below for information.
        //
        // https://docs.obsproject.com/frontends
        unsafe {
            // Safety: The audio_info pointer is valid here.
            libobs::obs_reset_audio2(info.obs_audio_info.as_ptr().0);
        }

        // Resets the video context. Note that this
        // is similar to Self::reset_video, but it
        // does not call that function because the
        // ObsContext struct is not created yet,
        // and also because there is no need to free
        // anything tied to the OBS context.
        let reset_video_status = num_traits::FromPrimitive::from_i32(unsafe {
            // Safety: The video_info pointer is valid here.
            libobs::obs_reset_video(info.obs_video_info.as_ptr())
        });

        let reset_video_status = match reset_video_status {
            Some(x) => x,
            None => ObsResetVideoStatus::Failure,
        };

        if reset_video_status != ObsResetVideoStatus::Success {
            return Err(ObsError::ResetVideoFailure(reset_video_status));
        }

        let sdr_info = info.obs_video_info.get_sdr_info();
        unsafe {
            // Safety: These are just numbers, so it's safe to call this here. Also graphics are initialized, so we can call this.
            libobs::obs_set_video_levels(sdr_info.sdr_white_level, sdr_info.hdr_nominal_peak_level);
        }

        unsafe {
            obs_modules.load_modules();
        }

        internal_log_global(
            ObsLogLevel::Info,
            "==== Startup complete ===============================================".to_string(),
        );

        #[cfg(windows)]
        if let Some(e) = native.as_ref() {
            e.unset_dpi_awareness();
        }

        Ok((info, obs_modules, native))
    }

    /// Shuts down the OBS context and cleans up resources
    ///
    /// This method performs a clean shutdown of OBS, including:
    /// - Removing sources from output channels
    /// - Calling `obs_shutdown` to clean up OBS resources
    /// - Removing log and crash handlers
    /// - Checking for memory leaks
    ///
    /// Safety: Always run this in the OBS runtime context.
    #[allow(unknown_lints)]
    #[allow(ensure_obs_call_in_runtime)]
    unsafe fn shutdown_inner() -> Result<(), ObsError> {
        // Clean up sources
        for i in 0..libobs::MAX_CHANNELS {
            unsafe { libobs::obs_set_output_source(i, ptr::null_mut()) };
        }

        unsafe {
            // Safety: We are in the OBS thread, so it's safe to call this here. Also by this time, we _should_ have dropped all OBS resources.
            libobs::obs_shutdown()
        }

        let r = LOGGER.lock();
        match r {
            Ok(mut logger) => {
                logger.log(ObsLogLevel::Info, "OBS context shutdown.".to_string());
                let allocs = unsafe {
                    // Safety: Can always be called because it just returns a number.
                    libobs::bnum_allocs()
                };

                // Increasing this to 1 because of whats described below
                let mut notice = "";
                let level = if allocs > 1 {
                    ObsLogLevel::Error
                } else {
                    notice = " (this is an issue in the OBS source code that cannot be fixed)";
                    ObsLogLevel::Info
                };
                // One memory leak is expected here because OBS does not free array elements of the obs_data_path when calling obs_add_data_path
                // even when obs_remove_data_path is called. This is a bug in OBS.
                logger.log(
                    level,
                    format!("Number of memory leaks: {}{}", allocs, notice),
                );

                #[cfg(any(feature = "__test_environment", test))]
                {
                    assert_eq!(allocs, 1, "Memory leaks detected: {}", allocs);
                }
            }
            Err(_) => {
                println!("OBS context shutdown. (but couldn't lock logger)");
            }
        }

        unsafe {
            // Safety: We are in the OBS thread, so it's safe to call this here.
            // Clean up log and crash handler
            libobs::base_set_crash_handler(None, std::ptr::null_mut());
            libobs::base_set_log_handler(None, std::ptr::null_mut());
        }

        let mut mutex_value = OBS_THREAD_ID.lock().map_err(|_e| ObsError::MutexFailure)?;

        *mutex_value = None;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub fn get_platform(&self) -> Result<crate::utils::initialization::PlatformType, ObsError> {
        run_with_obs!(self, || {
            let raw_platform = unsafe {
                // Safety: This is safe to call as long as OBS is initialized.
                libobs::obs_get_nix_platform()
            };

            match raw_platform {
                libobs::obs_nix_platform_type_OBS_NIX_PLATFORM_X11_EGL => {
                    crate::utils::initialization::PlatformType::X11
                }
                libobs::obs_nix_platform_type_OBS_NIX_PLATFORM_WAYLAND => {
                    crate::utils::initialization::PlatformType::Wayland
                }
                _ => crate::utils::initialization::PlatformType::Invalid,
            }
        })
    }
}

/// Guard object to ensure proper cleanup when the runtime is dropped
///
/// This guard ensures that when the last reference to the runtime is dropped,
/// the OBS thread is properly terminated and all resources are cleaned up.
#[derive(Debug)]
pub struct _ObsRuntimeGuard {
    /// Thread handle for the OBS thread
    #[cfg(feature = "enable_runtime")]
    #[cfg_attr(
        all(
            feature = "no_blocking_drops",
            not(feature = "__test_environment"),
            not(test)
        ),
        allow(dead_code)
    )]
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// Sender channel for the OBS thread
    #[cfg(feature = "enable_runtime")]
    command_sender: Arc<Sender<ObsCommand>>,
}

#[cfg(feature = "enable_runtime")]
impl Drop for _ObsRuntimeGuard {
    /// Ensures the OBS thread is properly shut down when the runtime is dropped
    fn drop(&mut self) {
        log::trace!("Dropping ObsRuntime and shutting down OBS thread");
        // Theoretically the queued_commands is zero and should be increased but because
        // we are shutting down, we don't care about that.
        let r = self.command_sender.send(ObsCommand::Terminate);

        if thread::panicking() {
            return;
        }

        r.expect("Failed to send termination command to OBS thread");
        #[cfg(any(
            not(feature = "no_blocking_drops"),
            test,
            feature = "__test_environment"
        ))]
        {
            if cfg!(feature = "enable_runtime") {
                // Wait for the thread to finish
                let handle = self.handle.lock();
                if handle.is_err() {
                    log::error!("Failed to lock OBS thread handle for shutdown");
                    return;
                }

                let mut handle = handle.unwrap();
                let handle = handle.take().expect("Handle can not be empty");

                handle.join().expect("Failed to join OBS thread");
            }
        }
    }
}

#[cfg(not(feature = "enable_runtime"))]
impl Drop for _ObsRuntimeGuard {
    /// Ensures the OBS thread is properly shut down when the runtime is dropped
    fn drop(&mut self) {
        log::trace!("Dropping ObsRuntime and shutting down OBS thread");
        let r = unsafe { ObsRuntime::shutdown_inner() };

        if thread::panicking() {
            return;
        }

        r.unwrap();
    }
}
