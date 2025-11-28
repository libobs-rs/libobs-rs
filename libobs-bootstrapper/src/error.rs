#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObsBootstrapError {
    GeneralError(String),
    DownloadError(String),
    ExtractError(String),
    IoError(String),
    VersionError(String),
}

impl std::fmt::Display for ObsBootstrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObsBootstrapError::GeneralError(e) => write!(f, "Bootstrapper error: {:?}", e),
            ObsBootstrapError::DownloadError(e) => {
                write!(f, "Bootstrapper download error: {:?}", e)
            }
            ObsBootstrapError::ExtractError(e) => write!(f, "Bootstrapper extract error: {:?}", e),
            ObsBootstrapError::IoError(e) => write!(f, "Bootstrapper I/O error: {:?}", e),
            ObsBootstrapError::VersionError(e) => write!(f, "Version error: {:?}", e),
        }
    }
}
impl std::error::Error for ObsBootstrapError {}
