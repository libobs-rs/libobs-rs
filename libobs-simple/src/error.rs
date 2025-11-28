use std::fmt::Display;

/// Error type for libobs-simple operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObsSimpleError {
    /// The underlying libobs-wrapper error
    WrapperError(libobs_wrapper::utils::ObsError),
    /// Feature is not available on this system
    FeatureNotAvailable(String),
    /// Error from display-info crate
    DisplayInfoError(String),
    /// Error from window helper
    WindowHelperError(String),
}

impl Display for ObsSimpleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObsSimpleError::WrapperError(e) => write!(f, "OBS wrapper error: {}", e),
            ObsSimpleError::FeatureNotAvailable(msg) => write!(f, "Feature not available: {}", msg),
            ObsSimpleError::DisplayInfoError(e) => write!(f, "Display info error: {}", e),
            ObsSimpleError::WindowHelperError(e) => write!(f, "Window helper error: {}", e),
        }
    }
}

impl std::error::Error for ObsSimpleError {}

impl From<libobs_wrapper::utils::ObsError> for ObsSimpleError {
    fn from(err: libobs_wrapper::utils::ObsError) -> Self {
        ObsSimpleError::WrapperError(err)
    }
}
