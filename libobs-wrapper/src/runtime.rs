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

use std::ffi::CStr;
use std::sync::Arc;
use std::{ptr, thread};

use crate::crash_handler::main_crash_handler;
use crate::enums::{ObsLogLevel, ObsResetVideoStatus};
use crate::logger::{extern_log_callback, internal_log_global, LOGGER};
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
    /// Execute a function on the OBS thread and send result back
    Execute(
        Box<dyn FnOnce() -> Box<dyn std::any::Any + Send> + Send>,
        oneshot::Sender<Box<dyn std::any::Any + Send>>,
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
/// ```
#[derive(Debug, Clone)]
pub struct ObsRuntime {
    #[cfg(feature = "enable_runtime")]
    command_sender: Arc<Sender<ObsCommand>>,
    #[cfg(feature = "enable_runtime")]
    queued_commands: Arc<AtomicUsize>,
    _guard: Arc<_ObsRuntimeGuard>,

    #[cfg(not(feature = "enable_runtime"))]
    _platform_specific: Option<Arc<PlatformSpecificGuard>>,
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
    /// A `Result` containing either:
    /// - `ObsRuntimeReturn::Done` with the initialized runtime, modules, and startup info
    /// - `ObsRuntimeReturn::Restart` if OBS needs to be updated and the application should restart
    /// - An `ObsError` if initialization fails
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
    ///         Ok(ObsRuntimeReturn::Done(runtime_components)) => {
    ///             // Use the initialized runtime
    ///         },
    ///         Ok(ObsRuntimeReturn::Restart) => {
    ///             // Handle restart for OBS update
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
    fn init(info: StartupInfo) -> anyhow::Result<(ObsRuntime, ObsModules, StartupInfo)> {
        let (startup, mut modules, platform_specific) = Self::initialize_inner(info)?;

        let runtime = Self {
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
    fn init(info: StartupInfo) -> anyhow::Result<(ObsRuntime, ObsModules, StartupInfo)> {
        let (command_sender, command_receiver) = channel();
        let (init_tx, init_rx) = oneshot::channel();
        let queued_commands = Arc::new(AtomicUsize::new(0));

        let queued_commands_clone = queued_commands.clone();
        let handle = std::thread::spawn(move || {
            log::trace!("Starting OBS thread");

            let res = Self::initialize_inner(info);

            match res {
                Ok((info, modules, _platform_specific)) => {
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
                                let _ = result_sender.send(result);
                                queued_commands_clone.fetch_sub(1, Ordering::SeqCst);
                            }
                            ObsCommand::Terminate => break,
                        }
                    }

                    let r = Self::shutdown_inner();
                    if let Err(err) = r {
                        log::error!("Failed to shut down OBS context: {:?}", err);
                    }
                }
                Err(err) => {
                    log::error!("Failed to initialize OBS context: {:?}", err);
                    let _ = init_tx.send(Err(err));
                }
            }
        });

        log::trace!("Waiting for OBS thread to initialize");
        // Wait for initialization to complete
        let (mut m, info) = init_rx.recv()??;

        let handle = Arc::new(Mutex::new(Some(handle)));
        let command_sender = Arc::new(command_sender);
        let runtime = Self {
            command_sender: command_sender.clone(),
            queued_commands,
            _guard: Arc::new(_ObsRuntimeGuard {
                handle,
                command_sender,
            }),
        };

        m.0.runtime = Some(runtime.clone());
        Ok((runtime, m.0, info))
    }

    /// Executes an operation on the OBS thread without returning a value
    ///
    /// This is a convenience wrapper around `run_with_obs_result` for operations
    /// that don't need to return a value.
    ///
    /// # Parameters
    ///
    /// * `operation` - A function to execute on the OBS thread
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure
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
    pub fn run_with_obs<F>(&self, operation: F) -> anyhow::Result<()>
    where
        F: FnOnce() + Send + 'static,
    {
        self.run_with_obs_result(move || {
            operation();
            Result::<(), anyhow::Error>::Ok(())
        })??;

        Ok(())
    }

    /// Executes an operation on the OBS thread and returns a result
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
    pub fn run_with_obs_result<F, T>(&self, operation: F) -> anyhow::Result<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        #[cfg(feature = "enable_runtime")]
        {
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
                .send(ObsCommand::Execute(Box::new(wrapper), tx))
                .map_err(|_| anyhow::anyhow!("Failed to send command to OBS thread"))?;

            let result = rx
                .recv()
                .map_err(|_| anyhow::anyhow!("OBS thread dropped the response channel"))?;

            // Downcast the Any type back to T
            let res = result
                .downcast::<T>()
                .map(|boxed| *boxed)
                .map_err(|_| anyhow::anyhow!("Failed to downcast result to the expected type"))?;

            Ok(res)
        }

        #[cfg(not(feature = "enable_runtime"))]
        {
            let result = operation();
            Ok(result)
        }
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
    fn initialize_inner(
        mut info: StartupInfo,
    ) -> Result<(StartupInfo, ObsModules, Option<Arc<PlatformSpecificGuard>>), ObsError> {
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
            libobs::obs_init_win32_crash_handler();
        }

        // Set logger, load debug privileges and crash handler
        unsafe {
            libobs::base_set_crash_handler(Some(main_crash_handler), std::ptr::null_mut());
        }

        let native = platform_specific_setup()?;

        unsafe {
            libobs::base_set_log_handler(Some(extern_log_callback), std::ptr::null_mut());
        }

        let mut log_callback = LOGGER.lock().map_err(|_e| ObsError::MutexFailure)?;

        *log_callback = info.logger.take().expect("Logger can never be null");
        drop(log_callback);

        // Locale will only be used internally by
        // libobs for logging purposes, making it
        // unnecessary to support other languages.
        let locale_str = ObsString::new("en-US");
        let startup_status =
            unsafe { libobs::obs_startup(locale_str.as_ptr().0, ptr::null(), ptr::null_mut()) };

        let version = unsafe { libobs::obs_get_version_string() };
        let version_cstr = unsafe { CStr::from_ptr(version) };
        let version_str = version_cstr.to_string_lossy().into_owned();

        internal_log_global(ObsLogLevel::Info, format!("OBS {}", version_str));
        internal_log_global(
            ObsLogLevel::Info,
            "---------------------------------".to_string(),
        );

        if !startup_status {
            return Err(ObsError::Failure);
        }

        let mut obs_modules = ObsModules::add_paths(&info.startup_paths);

        // Note that audio is meant to only be reset
        // once. See the link below for information.
        //
        // https://docs.obsproject.com/frontends
        unsafe {
            libobs::obs_reset_audio2(info.obs_audio_info.as_ptr().0);
        }

        // Resets the video context. Note that this
        // is similar to Self::reset_video, but it
        // does not call that function because the
        // ObsContext struct is not created yet,
        // and also because there is no need to free
        // anything tied to the OBS context.
        let reset_video_status = num_traits::FromPrimitive::from_i32(unsafe {
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
            libobs::obs_set_video_levels(sdr_info.sdr_white_level, sdr_info.hdr_nominal_peak_level);
        }

        obs_modules.load_modules();

        internal_log_global(
            ObsLogLevel::Info,
            "==== Startup complete ===============================================".to_string(),
        );

        Ok((info, obs_modules, native))
    }

    /// Shuts down the OBS context and cleans up resources
    ///
    /// This method performs a clean shutdown of OBS, including:
    /// - Removing sources from output channels
    /// - Calling `obs_shutdown` to clean up OBS resources
    /// - Removing log and crash handlers
    /// - Checking for memory leaks
    fn shutdown_inner() -> Result<(), ObsError> {
        // Clean up sources
        for i in 0..libobs::MAX_CHANNELS {
            unsafe { libobs::obs_set_output_source(i, ptr::null_mut()) };
        }

        unsafe { libobs::obs_shutdown() }

        let r = LOGGER.lock();
        match r {
            Ok(mut logger) => {
                logger.log(ObsLogLevel::Info, "OBS context shutdown.".to_string());
                let allocs = unsafe { libobs::bnum_allocs() };

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
            // Clean up log and crash handler
            libobs::base_set_crash_handler(None, std::ptr::null_mut());
            libobs::base_set_log_handler(None, std::ptr::null_mut());
        }

        let mut mutex_value = OBS_THREAD_ID.lock().map_err(|_e| ObsError::MutexFailure)?;

        *mutex_value = None;
        Ok(())
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
        let r = self
            .command_sender
            .send(ObsCommand::Terminate)
            .map_err(|_| anyhow::anyhow!("Failed to send termination command to OBS thread"));

        if thread::panicking() {
            return;
        }

        r.unwrap();
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
        let r = ObsRuntime::shutdown_inner();

        if thread::panicking() {
            return;
        }

        r.unwrap();
    }
}
