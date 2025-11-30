use std::fmt::Display;

use display_info::error::DIError;

/// Error type for libobs-simple operations.
#[derive(Debug)]
pub enum ObsSimpleError {
    /// The underlying libobs-wrapper error
    WrapperError(libobs_wrapper::utils::ObsError),
    /// Feature is not available on this system
    FeatureNotAvailable(&'static str),
    /// Error from display-info crate
    DisplayInfoError(DIError),
    /// Error from window helper
    #[cfg(feature = "window-list")]
    WindowHelperError(libobs_window_helper::WindowHelperError),
}

impl Display for ObsSimpleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObsSimpleError::WrapperError(e) => write!(f, "OBS wrapper error: {}", e),
            ObsSimpleError::FeatureNotAvailable(msg) => write!(f, "Feature not available: {}", msg),
            ObsSimpleError::DisplayInfoError(e) => write!(f, "Display info error: {}", e),
            #[cfg(feature = "window-list")]
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

#[cfg(feature = "window-list")]
impl From<libobs_window_helper::WindowHelperError> for ObsSimpleError {
    fn from(err: libobs_window_helper::WindowHelperError) -> Self {
        ObsSimpleError::WindowHelperError(err)
    }
}
