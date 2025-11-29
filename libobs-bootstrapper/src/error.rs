#[derive(Debug)]
pub enum ObsBootstrapError {
    GeneralError(String),
    InvalidFormatError(String),
    /// Contains context and specific reqwest error
    DownloadError(&'static str, reqwest::Error),
    ExtractError(String),
    IoError(String),
    VersionError(String),
    /// This error indicates that the downloaded file's hash did not match the expected hash
    HashMismatchError,
    /// This error should never happen, report to maintainers
    InvalidState,
    /// This should be emitted in the ObsBootstrapperHandler to abort the download/extraction process (this does not clean up files or similar)
    Abort,
}

impl std::fmt::Display for ObsBootstrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObsBootstrapError::GeneralError(e) => write!(f, "Bootstrapper error: {:?}", e),
            ObsBootstrapError::DownloadError(context, e) => {
                write!(f, "Bootstrapper download error: {:?} ({:?})", context, e)
            }
            ObsBootstrapError::ExtractError(e) => write!(f, "Bootstrapper extract error: {:?}", e),
            ObsBootstrapError::IoError(e) => write!(f, "Bootstrapper I/O error: {:?}", e),
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
            ObsBootstrapError::Abort => write!(f, "Operation aborted by status handler"),
        }
    }
}
impl std::error::Error for ObsBootstrapError {}
