use std::fmt::Display;

/// Error type for window helper operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WindowHelperError {
    /// Error from Windows API
    WindowsApiError(String),
    /// Failed to get file name from path
    FileNameError,
    /// Failed to convert to string
    StringConversionError,
    /// Microsoft internal executable (filtered out)
    MicrosoftInternalExe,
    /// OBS executable (filtered out)
    ObsExe,
    /// Invalid state encountered
    InvalidState(String),
    /// No window found
    NoWindowFound,
    /// Integer conversion error
    IntConversionError(String),
}

impl Display for WindowHelperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WindowHelperError::WindowsApiError(e) => write!(f, "Windows API error: {}", e),
            WindowHelperError::FileNameError => write!(f, "Failed to get file name"),
            WindowHelperError::StringConversionError => write!(f, "Failed to convert to string"),
            WindowHelperError::MicrosoftInternalExe => {
                write!(f, "Handle is a Microsoft internal exe")
            }
            WindowHelperError::ObsExe => write!(f, "Handle is obs64.exe"),
            WindowHelperError::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            WindowHelperError::NoWindowFound => write!(f, "No window found"),
            WindowHelperError::IntConversionError(e) => {
                write!(f, "Integer conversion error: {}", e)
            }
        }
    }
}

impl std::error::Error for WindowHelperError {}

#[cfg(windows)]
impl From<windows::core::Error> for WindowHelperError {
    fn from(err: windows::core::Error) -> Self {
        WindowHelperError::WindowsApiError(err.to_string())
    }
}

impl From<std::num::TryFromIntError> for WindowHelperError {
    fn from(err: std::num::TryFromIntError) -> Self {
        WindowHelperError::IntConversionError(err.to_string())
    }
}

impl From<std::convert::Infallible> for WindowHelperError {
    fn from(_: std::convert::Infallible) -> Self {
        // Infallible can never actually be constructed, so this is unreachable
        unreachable!()
    }
}
