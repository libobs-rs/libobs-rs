use std::fmt::Display;

use crate::enums::ObsResetVideoStatus;

/// Error type for OBS function calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObsError {
    /// The `obs_startup` function failed on libobs.
    Failure,
    /// This error is emitted if a dummy DLL was loaded instead of the real libobs DLL. Make sure you bootstrap properly with libobs-bootstrapper
    InvalidDll,
    /// Failed to lock mutex describing whether there is a
    /// thread using libobs or not. Report to crate maintainer.
    MutexFailure,
    /// Some or no thread is already using libobs. This is a bug!
    ThreadFailure,
    /// Unable to reset video.
    ResetVideoFailure(ObsResetVideoStatus),
    /// Unable to reset video because the program attempted to
    /// change the graphics module. This is a bug!
    ResetVideoFailureGraphicsModule,
    /// Unable to reset video because some outputs were still active.
    ResetVideoFailureOutputActive,
    /// The function returned a null pointer, often indicating
    /// an error with creating the object of the requested
    /// pointer.
    NullPointer,
    OutputAlreadyActive,
    OutputStartFailure(Option<String>),
    OutputStopFailure(Option<String>),
    OutputPauseFailure(Option<String>),
    OutputNotFound,
    SourceNotFound,
    SourceNotAvailable(String),
    InvalidOperation(String),
    /// Error converting a string between Rust and OBS
    StringConversionError,

    /// Native error from the Windows API when creating a display
    DisplayCreationError(String),

    OutputSaveBufferFailure(String),

    /// The obs thread couldn't be called
    InvocationError(String),

    JsonParseError,
    /// Couldn't get the sender of the signal
    NoSenderError,
    NoAvailableEncoders,
    /// Error locking a mutex or RwLock. You should probably restart the application to avoid memory leaks.
    LockError(String),
    Unexpected(String),

    /// Encoder is still active, stop the attached output before proceeding
    EncoderActive,

    /// Error during platform-specific initialization
    PlatformInitError(String),

    /// Error reading/writing to a file
    IoError(String),

    /// Failed to get data from signal calldata
    SignalDataError(String),

    /// Failed to convert an enum from a code
    EnumConversionError(String),

    /// Failed to send/receive on a runtime channel
    RuntimeChannelError(String),

    /// Attempted to call a OBS runtime function from outside the OBS thread.
    /// This error should NEVER occur. If you are not using the runtime manually or have the "enable_runtime" feature enabled
    /// then please report this to the crate maintainer as this indicates a bug in the crate.
    RuntimeOutsideThread,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Display for ObsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OBS Error: ")?;

        match self {
            ObsError::Failure => write!(f, "`obs-startup` function failed on libobs"),
            ObsError::MutexFailure => write!(f, "Failed to lock mutex describing whether there is a thread using libobs or not. Report to crate maintainer."),
            ObsError::ThreadFailure => write!(f, "Some or no thread is already using libobs. This is a bug!"),
            ObsError::ResetVideoFailure(status) => write!(f, "Could not reset obs video. Status: {:?}", status),
            ObsError::ResetVideoFailureGraphicsModule => write!(f, "Unable to reset video because the program attempted to change the graphics module. This is a bug!"),
            ObsError::ResetVideoFailureOutputActive => write!(f, "Unable to reset video because some outputs were still active."),
            ObsError::NullPointer => write!(f, "The function returned a null pointer, often indicating an error with creating the object of the requested pointer."),
            ObsError::OutputAlreadyActive => write!(f, "Output is already active."),
            ObsError::OutputStartFailure(s) => write!(f, "Output failed to start. Error is {:?}", s),
            ObsError::OutputStopFailure(s) => write!(f, "Output failed to stop. Error is {:?}", s),
            ObsError::OutputNotFound => write!(f, "Output not found."),
            ObsError::DisplayCreationError(e) => write!(f, "Native error from the Windows API when creating a display: {:?}", e),
            ObsError::OutputSaveBufferFailure(e) => write!(f, "Couldn't save output buffer: {:?}", e),
            ObsError::SourceNotFound => write!(f, "Source not found."),
            ObsError::SourceNotAvailable(source_name) => write!(f, "Source {} is not available. See logs or similar to check why.", source_name),
            ObsError::InvocationError(e) => write!(f, "The obs thread couldn't be called: {:?}", e),
            ObsError::JsonParseError => write!(f, "Failed to parse JSON data."),
            ObsError::NoSenderError => write!(f, "Couldn't get the sender of the signal."),
            ObsError::NoAvailableEncoders => write!(f, "No available encoders found."),
            ObsError::OutputPauseFailure(s) => write!(f, "Output failed to pause. Error is {:?}", s),
            ObsError::LockError(e) => write!(f, "Error locking a mutex or RwLock: {:?}. You should probably restart the application to avoid memory leaks.", e),
            ObsError::Unexpected(e) => write!(f, "Unexpected error: {:?}", e),
            ObsError::EncoderActive => write!(f, "Encoder is still active, stop the attached output before proceeding"),
            ObsError::StringConversionError => write!(f, "Error converting a string between Rust and OBS"),
            ObsError::PlatformInitError(e) => write!(f, "Error during platform-specific initialization: {}", e),
            ObsError::InvalidOperation(e) => write!(f, "Invalid operation: {}", e),
            ObsError::IoError(e) => write!(f, "I/O error: {}", e),
            ObsError::SignalDataError(e) => write!(f, "Signal data error: {}", e),
            ObsError::EnumConversionError(e) => write!(f, "Enum conversion error: {}", e),
            ObsError::RuntimeChannelError(e) => write!(f, "Runtime channel error: {}", e),
            ObsError::InvalidDll => write!(f, "A dummy DLL was loaded instead of the real libobs DLL. Make sure you bootstrap properly with libobs-bootstrapper"),
            #[cfg(feature="enable_runtime")]
            ObsError::RuntimeOutsideThread => write!(f, "Attempted to call a OBS runtime function from outside the OBS thread. This is a bug in the crate!"),
            #[cfg(not(feature="enable_runtime"))]
            ObsError::RuntimeOutsideThread => write!(f, "Attempted to call a OBS runtime function from outside the OBS thread. Make sure that you do not use any OBS struct from a different thread than the one where the ObsContext was initialized. THIS BUG WILL CAUSE MEMORY CORRUPTION OR DEADLOCKS!"),
        }
    }
}

impl std::error::Error for ObsError {}
