#[derive(Debug)]
pub enum ObsBootstrapError {
    GeneralError(String),
    InvalidFormatError(String),
    /// Contains context and specific reqwest error
    DownloadError(&'static str, reqwest::Error),
    ExtractError(String),
    /// Contains context and specific io error
    IoError(&'static str, std::io::Error),
    LibLoadingError(&'static str, libloading::Error),
    VersionError(String),
    /// This error indicates that the downloaded file's hash did not match the expected hash
    HashMismatchError,
    /// This error should never happen, report to maintainers
    InvalidState,
    /// This error is emitted when a status handler returns an error instead of an Ok(()). This is the Error type that your handler uses.
    Abort(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ObsBootstrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObsBootstrapError::GeneralError(e) => write!(f, "Bootstrapper error: {:?}", e),
            ObsBootstrapError::DownloadError(context, e) => {
                write!(f, "Bootstrapper download error: {:?} ({:?})", context, e)
            }
            ObsBootstrapError::ExtractError(e) => write!(f, "Bootstrapper extract error: {:?}", e),
            ObsBootstrapError::IoError(context, error) => write!(f, "{}: {:?}", context, error),
            ObsBootstrapError::VersionError(e) => write!(f, "Version error: {:?}", e),
            ObsBootstrapError::InvalidFormatError(e) => write!(f, "Invalid format error: {:?}", e),
            ObsBootstrapError::HashMismatchError => write!(
                f,
                "Hash mismatch error: The downloaded file's hash did not match the expected hash"
            ),
            ObsBootstrapError::InvalidState => write!(
                f,
                "Invalid state error: This error should never happen, please report to maintainers"
            ),
            ObsBootstrapError::Abort(e) => {
                write!(f, "Operation aborted by status handler: {:?}", e)
            }
            ObsBootstrapError::LibLoadingError(context, e) => {
                write!(f, "Library loading error: {}: {:?}", context, e)
            }
        }
    }
}
impl std::error::Error for ObsBootstrapError {}
