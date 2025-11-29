use std::{convert::Infallible, fmt::Debug};

//NOTE: Maybe do not require to implement Debug here?
pub trait ObsBootstrapStatusHandler: Debug + Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Used to report in some way or another the download progress to the user (this is between 0.0 and 1.0)
    /// # Errors
    /// This should return an error if the download process should be aborted. This error will be mapped to `ObsBootstrapError::Abort`. This WILL NOT clean up any files or similar, that is the responsibility of the caller.
    fn handle_downloading(&mut self, progress: f32, message: String) -> Result<(), Self::Error>;

    /// Used to report in some way another the extraction progress to the user (this is between 0.0 and 1.0)
    /// # Errors
    /// This should return an error if the extraction process should be aborted. This error will be mapped to `ObsBootstrapError::Abort`. This WILL NOT clean up any files or similar, that
    fn handle_extraction(&mut self, progress: f32, message: String) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub struct ObsBootstrapConsoleHandler {
    last_download_percentage: f32,
    last_extract_percentage: f32,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for ObsBootstrapConsoleHandler {
    fn default() -> Self {
        Self {
            last_download_percentage: 0.0,
            last_extract_percentage: 0.0,
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl ObsBootstrapStatusHandler for ObsBootstrapConsoleHandler {
    type Error = Infallible;

    fn handle_downloading(&mut self, progress: f32, message: String) -> Result<(), Infallible> {
        if progress - self.last_download_percentage >= 0.05 || progress == 1.0 {
            self.last_download_percentage = progress;
            println!("Downloading: {}% - {}", progress * 100.0, message);
        }
        Ok(())
    }

    fn handle_extraction(&mut self, progress: f32, message: String) -> Result<(), Infallible> {
        if progress - self.last_extract_percentage >= 0.05 || progress == 1.0 {
            self.last_extract_percentage = progress;
            println!("Extracting: {}% - {}", progress * 100.0, message);
        }
        Ok(())
    }
}
