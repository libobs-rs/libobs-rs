//! Runtime management for serialized OBS access.
//!
//! libobs has process-global state and a number of thread-affine operations.  The
//! default runtime therefore treats OBS as an actor: callers submit bounded work to
//! one dedicated thread while native resource destruction uses a separate cleanup
//! queue.  The cleanup queue is intentionally unbounded because Rust destructors must
//! never deadlock behind normal application work.

use std::ffi::CStr;
use std::fmt::Debug;
use std::ptr;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;
#[cfg(feature = "enable_runtime")]
use std::thread::JoinHandle;

#[cfg(feature = "enable_runtime")]
use crossbeam_channel::{bounded, select, unbounded, Sender, TrySendError};
#[cfg(feature = "enable_runtime")]
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicU8, Ordering},
        Mutex,
    },
};

use crate::context::ObsContext;
use crate::crash_handler::main_crash_handler;
use crate::enums::{ObsLogLevel, ObsResetVideoStatus};
use crate::logger::{extern_log_callback, internal_log_global, LOGGER};
#[cfg(target_os = "linux")]
use crate::run_with_obs;
use crate::unsafe_send::NativeObjectRegistry;
use crate::utils::initialization::{platform_specific_setup, PlatformSpecificGuard};
use crate::utils::{ObsError, ObsModules, ObsString};
use crate::{
    context::{
        activate_runtime_slot, begin_runtime_shutdown, cancel_runtime_start, release_runtime_slot,
        reserve_runtime_slot,
    },
    utils::StartupInfo,
};

#[cfg(feature = "enable_runtime")]
const RUNTIME_QUEUE_CAPACITY: usize = 128;
#[cfg(feature = "enable_runtime")]
const RUNTIME_RUNNING: u8 = 0;
#[cfg(feature = "enable_runtime")]
const RUNTIME_SHUTTING_DOWN: u8 = 1;
#[cfg(feature = "enable_runtime")]
const RUNTIME_PANICKED: u8 = 2;
#[cfg(feature = "enable_runtime")]
const RUNTIME_STOPPED: u8 = 3;

#[cfg(feature = "enable_runtime")]
type RuntimeTask = Box<dyn FnOnce() + Send + 'static>;

#[cfg(feature = "enable_runtime")]
enum ObsCommand {
    Execute(RuntimeTask),
}

#[cfg(feature = "enable_runtime")]
fn execute_command(command: ObsCommand) {
    match command {
        ObsCommand::Execute(task) => task(),
    }
}

#[cfg(feature = "enable_runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QueueSubmitError {
    Full,
    Disconnected,
}

#[cfg(feature = "enable_runtime")]
fn try_submit_command(
    sender: &Sender<ObsCommand>,
    command: ObsCommand,
) -> Result<(), QueueSubmitError> {
    match sender.try_send(command) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => Err(QueueSubmitError::Full),
        Err(TrySendError::Disconnected(_)) => Err(QueueSubmitError::Disconnected),
    }
}

#[cfg(feature = "enable_runtime")]
fn run_actor_work<F>(work: F) -> bool
where
    F: FnOnce(),
{
    catch_unwind(AssertUnwindSafe(work)).is_ok()
}

struct InitializationRollback {
    armed: bool,
    obs_started: bool,
}

impl InitializationRollback {
    fn new() -> Self {
        Self {
            armed: true,
            obs_started: false,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
impl Drop for InitializationRollback {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }

        log::warn!("Rolling back incomplete OBS initialization");
        if self.obs_started {
            unsafe {
                // SAFETY: Initialization and rollback occur on the owning OBS thread.
                libobs::obs_shutdown();
            }
        }
        unsafe {
            // SAFETY: These process-global handlers were installed by initialize_inner.
            libobs::base_set_crash_handler(None, std::ptr::null_mut());
            libobs::base_set_log_handler(None, std::ptr::null_mut());
        }
        let _ = release_runtime_slot();
    }
}

/// Core runtime that serializes access to libobs.
#[derive(Clone)]
pub struct ObsRuntime {
    #[cfg(feature = "enable_runtime")]
    command_sender: Arc<Sender<ObsCommand>>,
    #[cfg(feature = "enable_runtime")]
    cleanup_sender: Arc<Sender<RuntimeTask>>,
    #[cfg(feature = "enable_runtime")]
    health: Arc<AtomicU8>,
    native_registry: Arc<NativeObjectRegistry>,
    thread_id: std::thread::ThreadId,
    _guard: Arc<_ObsRuntimeGuard>,

    #[cfg(not(feature = "enable_runtime"))]
    _platform_specific: Option<Rc<PlatformSpecificGuard>>,
}

impl Debug for ObsRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObsRuntime")
            .field("thread_id", &self.thread_id)
            .field("native_objects", &self.native_registry.len())
            .finish_non_exhaustive()
    }
}

impl ObsRuntime {
    pub(crate) fn startup(
        options: StartupInfo,
    ) -> Result<(ObsRuntime, ObsModules, StartupInfo), ObsError> {
        reserve_runtime_slot()?;
        log::trace!("Initializing OBS context");
        match Self::init(options) {
            Ok(initialized) => Ok(initialized),
            Err(err) => {
                // Only undo a reservation that never became active. initialize_inner
                // owns rollback after it activates the process-global slot.
                cancel_runtime_start();
                Err(err)
            }
        }
    }

    #[cfg(not(feature = "enable_runtime"))]
    fn init(info: StartupInfo) -> Result<(ObsRuntime, ObsModules, StartupInfo), ObsError> {
        let (startup, mut modules, platform_specific) = unsafe { Self::initialize_inner(info)? };
        let runtime = Self {
            native_registry: Arc::new(NativeObjectRegistry::default()),
            thread_id: thread::current().id(),
            _guard: Arc::new(_ObsRuntimeGuard {}),
            _platform_specific: platform_specific,
        };
        modules.runtime = Some(runtime.clone());
        Ok((runtime, modules, startup))
    }

    #[cfg(feature = "enable_runtime")]
    #[allow(unknown_lints)]
    #[allow(ensure_obs_call_in_runtime)]
    fn init(info: StartupInfo) -> Result<(ObsRuntime, ObsModules, StartupInfo), ObsError> {
        static RUNTIME_THREAD_NAME: &str = "libobs-wrapper-obs-runtime";

        let health = Arc::new(AtomicU8::new(RUNTIME_RUNNING));
        let actor_health = health.clone();
        let (command_sender, command_receiver) = bounded(RUNTIME_QUEUE_CAPACITY);
        let (cleanup_sender, cleanup_receiver) = unbounded::<RuntimeTask>();
        let (shutdown_sender, shutdown_receiver) = unbounded::<()>();
        let (init_tx, init_rx) = bounded(1);

        let handle = std::thread::Builder::new()
            .name(RUNTIME_THREAD_NAME.to_string())
            .spawn(move || {
                log::trace!("Starting OBS actor thread");
                // SAFETY: This closure is the dedicated OBS actor thread and owns the
                // complete initialize/use/shutdown sequence for libobs. Catch Rust panics
                // as well as typed initialization failures so the process-global slot
                // cannot be stranded in Starting/Active.
                let initialized = catch_unwind(AssertUnwindSafe(|| {
                    // SAFETY: This closure is executing on the dedicated OBS actor that
                    // owns initialization for the complete process-global OBS lifetime.
                    unsafe { Self::initialize_inner(info) }
                }));

                let (info, modules, platform_specific_guard) = match initialized {
                    Ok(Ok(value)) => value,
                    Ok(Err(err)) => {
                        let _ = init_tx.send(Err(err));
                        return;
                    }
                    Err(_) => {
                        actor_health.store(RUNTIME_PANICKED, Ordering::Release);
                        // SAFETY: We are still on the owning OBS actor and initialization
                        // panicked before control could be returned to another thread.
                        // SAFETY: Shutdown recovery runs on the owning OBS actor after
                        // normal work has stopped; clearing process-global callbacks is safe.
                        unsafe {
                            libobs::base_set_crash_handler(None, std::ptr::null_mut());
                            libobs::base_set_log_handler(None, std::ptr::null_mut());
                        }
                        let _ = release_runtime_slot();
                        let _ = init_tx.send(Err(ObsError::RuntimePanicked));
                        return;
                    }
                };

                if init_tx.send(Ok((modules, info))).is_err() {
                    log::error!("OBS runtime initializer was dropped before startup completed");
                    // SAFETY: Initialization and this cleanup both run on the same OBS actor thread.
                    let _ = unsafe { Self::shutdown_inner() };
                    return;
                }

                // Keep platform-specific thread-affine state alive for the complete OBS lifetime.
                let _platform_specific_guard = platform_specific_guard;

                let actor_survived = run_actor_work(|| {
                    'runtime: loop {
                        select! {
                            recv(cleanup_receiver) -> cleanup => match cleanup {
                                Ok(task) => task(),
                                Err(_) => break 'runtime,
                            },
                            recv(command_receiver) -> command => match command {
                                Ok(command) => execute_command(command),
                                Err(_) => break 'runtime,
                            },
                            recv(shutdown_receiver) -> _ => break 'runtime,
                        }
                    }

                    // A normal last-runtime drop may leave fire-and-forget work and deferred
                    // destruction. Preserve FIFO ordering before obs_shutdown().
                    while let Ok(command) = command_receiver.try_recv() {
                        execute_command(command);
                    }
                    while let Ok(cleanup) = cleanup_receiver.try_recv() {
                        cleanup();
                    }
                });

                if !actor_survived {
                    actor_health.store(RUNTIME_PANICKED, Ordering::Release);
                    log::error!("OBS actor command panicked; rejecting pending work and shutting down safely");

                    // Pending normal commands must not execute after an arbitrary command panic.
                    // Dropping them disconnects any synchronous result channels. Native cleanup
                    // is still attempted before shutdown so owned OBS references are released.
                    while command_receiver.try_recv().is_ok() {}
                    while let Ok(cleanup) = cleanup_receiver.try_recv() {
                        if !run_actor_work(cleanup) {
                            log::error!("A deferred OBS cleanup task also panicked during actor recovery");
                        }
                    }
                }

                let shutdown_result = catch_unwind(AssertUnwindSafe(|| unsafe {
                    // SAFETY: The actor owns the complete libobs lifetime and shutdown
                    // runs on the same thread that initialized it.
                    Self::shutdown_inner()
                }));
                match shutdown_result {
                    Ok(Ok(())) => {}
                    Ok(Err(err)) => log::error!("Failed to shut down OBS context: {err:?}"),
                    Err(_) => {
                        log::error!("OBS shutdown panicked; clearing handlers and force-releasing the global runtime slot");
                        // SAFETY: This recovery branch still executes on the owning OBS actor
                        // after normal work has stopped; clearing global callbacks is safe here.
                        unsafe {
                            libobs::base_set_crash_handler(None, std::ptr::null_mut());
                            libobs::base_set_log_handler(None, std::ptr::null_mut());
                        }
                        let _ = release_runtime_slot();
                    }
                }
                if actor_health.load(Ordering::Acquire) != RUNTIME_PANICKED {
                    actor_health.store(RUNTIME_STOPPED, Ordering::Release);
                }
            })
            .map_err(|_| ObsError::ThreadFailure)?;

        let (mut modules, info) = init_rx.recv().map_err(|_| {
            ObsError::RuntimeChannelError("OBS actor exited during initialization".to_string())
        })??;

        let thread_id = handle.thread().id();
        let command_sender = Arc::new(command_sender);
        let cleanup_sender = Arc::new(cleanup_sender);
        let shutdown_sender = Arc::new(shutdown_sender);
        let runtime = Self {
            command_sender: command_sender.clone(),
            cleanup_sender: cleanup_sender.clone(),
            health: health.clone(),
            native_registry: Arc::new(NativeObjectRegistry::default()),
            thread_id,
            _guard: Arc::new(_ObsRuntimeGuard {
                handle: Mutex::new(Some(handle)),
                shutdown_sender,
                health,
            }),
        };

        modules.runtime = Some(runtime.clone());
        Ok((runtime, modules, info))
    }

    /// Returns true when called from the dedicated OBS actor thread.
    pub fn is_actor_thread(&self) -> bool {
        std::thread::current().id() == self.thread_id
    }

    /// Returns true when both values belong to the same process-global OBS runtime.
    pub fn same_instance(&self, other: &Self) -> bool {
        self.native_registry.runtime_id() == other.native_registry.runtime_id()
    }

    pub(crate) fn ensure_same_runtime(&self, other: &Self) -> Result<(), ObsError> {
        if self.same_instance(other) {
            Ok(())
        } else {
            Err(ObsError::RuntimeMismatch)
        }
    }

    #[cfg(feature = "enable_runtime")]
    fn unavailable_error(&self) -> ObsError {
        match self.health.load(Ordering::Acquire) {
            RUNTIME_PANICKED => ObsError::RuntimePanicked,
            _ => ObsError::RuntimeChannelError("OBS actor is shutting down or stopped".to_string()),
        }
    }

    /// Dispatches work without waiting for completion.
    ///
    /// The regular actor queue is bounded.  If it is full this returns
    /// [`ObsError::RuntimeQueueFull`] instead of growing memory without bound.
    #[cfg(feature = "enable_runtime")]
    pub fn run_with_obs_no_block<F>(&self, operation: F) -> Result<(), ObsError>
    where
        F: FnOnce() + Send + 'static,
    {
        if self.is_actor_thread() {
            operation();
            return Ok(());
        }
        if self.health.load(Ordering::Acquire) != RUNTIME_RUNNING {
            return Err(self.unavailable_error());
        }

        match try_submit_command(
            &self.command_sender,
            ObsCommand::Execute(Box::new(operation)),
        ) {
            Ok(()) => Ok(()),
            Err(QueueSubmitError::Full) => Err(ObsError::RuntimeQueueFull {
                capacity: RUNTIME_QUEUE_CAPACITY,
            }),
            Err(QueueSubmitError::Disconnected) => Err(self.unavailable_error()),
        }
    }

    #[cfg(not(feature = "enable_runtime"))]
    pub fn run_with_obs_no_block<F>(&self, operation: F) -> Result<(), ObsError>
    where
        F: FnOnce() + 'static,
    {
        self.run_with_obs_result(operation)
    }

    /// Runs work on the OBS actor and returns its typed result.
    ///
    /// Unlike the old implementation this does not erase the result into `Any`, so
    /// there is no runtime downcast or corresponding impossible error path.
    #[cfg(feature = "enable_runtime")]
    pub fn run_with_obs_result<F, T>(&self, operation: F) -> Result<T, ObsError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        if self.is_actor_thread() {
            return Ok(operation());
        }
        if self.health.load(Ordering::Acquire) != RUNTIME_RUNNING {
            return Err(self.unavailable_error());
        }

        let (result_tx, result_rx) = bounded(1);
        let task = move || {
            let result = operation();
            let _ = result_tx.send(result);
        };

        match try_submit_command(&self.command_sender, ObsCommand::Execute(Box::new(task))) {
            Ok(()) => {}
            Err(QueueSubmitError::Full) => {
                return Err(ObsError::RuntimeQueueFull {
                    capacity: RUNTIME_QUEUE_CAPACITY,
                });
            }
            Err(QueueSubmitError::Disconnected) => return Err(self.unavailable_error()),
        }

        result_rx.recv().map_err(|_| self.unavailable_error())
    }

    #[cfg(not(feature = "enable_runtime"))]
    pub fn run_with_obs_result<F, T>(&self, operation: F) -> Result<T, ObsError>
    where
        F: FnOnce() -> T,
    {
        if std::thread::current().id() != self.thread_id {
            return Err(ObsError::RuntimeOutsideThread);
        }
        Ok(operation())
    }

    pub(crate) fn native_registry(&self) -> Arc<NativeObjectRegistry> {
        self.native_registry.clone()
    }

    /// Queue native destruction without making a Rust `Drop` implementation wait for
    /// the normal actor queue.  This is public only so exported wrapper macros can use
    /// it from companion crates; application code normally has no reason to call it.
    #[doc(hidden)]
    #[cfg(feature = "enable_runtime")]
    pub fn defer_obs_cleanup<F>(&self, cleanup: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if std::thread::current().id() == self.thread_id {
            cleanup();
            return;
        }

        if self.cleanup_sender.send(Box::new(cleanup)).is_err() {
            // We must never panic from Drop.  At this point the actor is gone, so the
            // only safe option is to report the leak rather than call libobs off-thread.
            log::error!("OBS actor stopped before deferred native cleanup could be queued");
        }
    }

    #[doc(hidden)]
    #[cfg(not(feature = "enable_runtime"))]
    pub fn defer_obs_cleanup<F>(&self, cleanup: F)
    where
        F: FnOnce() + 'static,
    {
        if std::thread::current().id() == self.thread_id {
            cleanup();
        } else {
            log::error!("Native OBS cleanup requested from outside the owning thread");
        }
    }

    #[allow(unknown_lints)]
    #[allow(ensure_obs_call_in_runtime)]
    unsafe fn initialize_inner(
        mut info: StartupInfo,
    ) -> Result<(StartupInfo, ObsModules, Option<Rc<PlatformSpecificGuard>>), ObsError> {
        // `startup` reserved the process-global slot before spawning this actor.
        // Transition that reservation to the concrete owning thread.
        activate_runtime_slot(thread::current().id())?;

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

        let native = match unsafe {
            // Safety: We are in the OBS thread and the nix_display can only be set here.
            platform_specific_setup(info.nix_display.clone())
        } {
            Ok(native) => native,
            Err(err) => {
                unsafe {
                    libobs::base_set_crash_handler(None, std::ptr::null_mut());
                }
                let _ = release_runtime_slot();
                return Err(err);
            }
        };
        let mut rollback = InitializationRollback::new();
        unsafe {
            // Safety: We are in the OBS thread, so it's safe to call this here.
            libobs::base_set_log_handler(Some(extern_log_callback), std::ptr::null_mut());
        }

        let mut log_callback = LOGGER.lock().map_err(|_e| ObsError::MutexFailure)?;

        *log_callback = info.logger.take().ok_or_else(|| {
            ObsError::InvalidOperation("startup logger was already consumed".to_string())
        })?;
        drop(log_callback);

        // Locale will only be used internally by
        // libobs for logging purposes, making it
        // unnecessary to support other languages.
        let locale_str = ObsString::new("en-US");
        let module_config_path = info
            .module_config_path
            .as_ref()
            .map_or(ptr::null(), |path| path.as_ptr().0);
        internal_log_global(
            ObsLogLevel::Info,
            format!(
                "[libobs-wrapper]: Module config path: {}",
                info.module_config_path
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "<unset>".to_string())
            ),
        );
        let startup_status = unsafe {
            // Safety: Both strings are owned by this startup frame and remain valid
            // for the duration of obs_startup. A null module config path preserves
            // libobs's documented "no configuration directory" behavior.
            libobs::obs_startup(locale_str.as_ptr().0, module_config_path, ptr::null_mut())
        };
        if !startup_status {
            return Err(ObsError::Failure);
        }
        // From this exact point onward any Rust error/panic must run obs_shutdown().
        rollback.obs_started = true;

        // SAFETY: libobs startup succeeded on this actor; the returned pointer is either
        // null or a libobs-owned NUL-terminated version string.
        let version = unsafe { libobs::obs_get_version_string() };
        let version_str = if version.is_null() {
            "unknown".to_string()
        } else {
            // SAFETY: `version` was checked non-null immediately above and libobs owns
            // the NUL-terminated string for the process lifetime.
            let version_cstr = unsafe { CStr::from_ptr(version) };
            version_cstr.to_string_lossy().into_owned()
        };

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

        let mut obs_modules = unsafe {
            // Safety: This is running in the OBS thread, so it's safe to call this here.
            ObsModules::add_paths(&info.startup_paths)
        };

        // Note that audio is meant to only be reset
        // once. See the link below for information.
        //
        // https://docs.obsproject.com/frontends
        let audio_ready = unsafe {
            // Safety: The audio_info pointer is valid here.
            libobs::obs_reset_audio2(info.obs_audio_info.as_ptr().0)
        };
        if !audio_ready {
            return Err(ObsError::ResetAudioFailure);
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

        rollback.disarm();
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
                if allocs != 1 {
                    log::error!("OBS leak check expected 1 allocation, observed {allocs}");
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

        release_runtime_slot()?;
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

/// Guard for the process-global actor lifetime.
#[derive(Debug)]
pub struct _ObsRuntimeGuard {
    #[cfg(feature = "enable_runtime")]
    handle: Mutex<Option<JoinHandle<()>>>,
    #[cfg(feature = "enable_runtime")]
    shutdown_sender: Arc<Sender<()>>,
    #[cfg(feature = "enable_runtime")]
    health: Arc<AtomicU8>,
}

#[cfg(feature = "enable_runtime")]
impl Drop for _ObsRuntimeGuard {
    fn drop(&mut self) {
        log::trace!("Last ObsRuntime dropped; requesting actor shutdown");
        begin_runtime_shutdown();
        self.health.store(RUNTIME_SHUTTING_DOWN, Ordering::Release);
        if self.shutdown_sender.send(()).is_err() {
            let _ = release_runtime_slot();
        }

        // Never wait for native execution from a production destructor.  Dropping a
        // JoinHandle detaches the worker, which owns everything it needs to finish the
        // queued cleanup and call obs_shutdown safely.
        let handle = match self.handle.get_mut() {
            Ok(handle) => handle.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        };

        #[cfg(any(test, feature = "__test_environment"))]
        if let Some(handle) = handle {
            // Integration tests need deterministic shutdown before the next test starts.
            if handle.thread().id() != std::thread::current().id() && handle.join().is_err() {
                log::error!("OBS actor panicked during test shutdown");
            }
        }

        #[cfg(not(any(test, feature = "__test_environment")))]
        drop(handle);
    }
}

#[cfg(not(feature = "enable_runtime"))]
impl Drop for _ObsRuntimeGuard {
    fn drop(&mut self) {
        log::trace!("Last local ObsRuntime dropped; shutting down OBS");
        begin_runtime_shutdown();
        if let Err(err) = unsafe { ObsRuntime::shutdown_inner() } {
            log::error!("Failed to shut down OBS context: {err:?}");
        }
    }
}

#[cfg(all(test, feature = "enable_runtime"))]
mod tests {
    use super::*;

    #[test]
    fn bounded_submission_reports_full_without_blocking() {
        let (sender, _receiver) = bounded(1);
        try_submit_command(&sender, ObsCommand::Execute(Box::new(|| {})))
            .expect("first command fits");
        assert_eq!(
            try_submit_command(&sender, ObsCommand::Execute(Box::new(|| {}))),
            Err(QueueSubmitError::Full)
        );
    }

    #[test]
    fn bounded_submission_reports_disconnect() {
        let (sender, receiver) = bounded(1);
        drop(receiver);
        assert_eq!(
            try_submit_command(&sender, ObsCommand::Execute(Box::new(|| {}))),
            Err(QueueSubmitError::Disconnected)
        );
    }

    #[test]
    fn actor_work_contains_command_panics() {
        assert!(!run_actor_work(|| panic!("synthetic actor command panic")));
        assert!(run_actor_work(|| {}));
    }
}
