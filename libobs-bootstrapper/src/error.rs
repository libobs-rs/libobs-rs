#[derive(Debug)]
pub enum ObsBootstrapError {
    /// Legacy variant retained for source compatibility. New bootstrap calls do
    /// not return it.
    RuntimeBootstrapDisabled,
    /// OBS is already mapped into this process, so replacing its runtime files
    /// would be unsafe. On Windows, enable delay loading and bootstrap first.
    RuntimeAlreadyLoaded,
    GeneralError(String),
    UnsupportedPlatform(String),
    InvalidFormatError(String),
    ExtractError(String),
    /// Contains context and specific I/O error.
    IoError(&'static str, std::io::Error),
    LibLoadingError(&'static str, libloading::Error),
    VersionError(String),
    HashMismatchError,
    InvalidState,
    Abort(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for ObsBootstrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RuntimeBootstrapDisabled => write!(f, "Runtime OBS bootstrap is disabled"),
            Self::RuntimeAlreadyLoaded => write!(
                f,
                "OBS is already loaded in this process; bootstrap/update must run before the first OBS call"
            ),
            Self::GeneralError(e) => write!(f, "Bootstrapper error: {e}"),
            Self::UnsupportedPlatform(e) => write!(f, "Unsupported platform: {e}"),
            Self::ExtractError(e) => write!(f, "Bootstrapper extract error: {e}"),
            Self::IoError(context, error) => write!(f, "{context}: {error}"),
            Self::VersionError(e) => write!(f, "Version error: {e}"),
            Self::InvalidFormatError(e) => write!(f, "Invalid format error: {e}"),
            Self::HashMismatchError => write!(
                f,
                "Hash mismatch error: the downloaded file did not match its expected SHA-256"
            ),
            Self::InvalidState => write!(f, "Invalid bootstrapper state"),
            Self::Abort(e) => write!(f, "Operation aborted by status handler: {e}"),
            Self::LibLoadingError(context, e) => {
                write!(f, "Library loading error: {context}: {e}")
            }
        }
    }
}

impl std::error::Error for ObsBootstrapError {}
