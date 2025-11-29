use std::fmt::Debug;

use crate::ObsBootstrapError;

//NOTE: Maybe do not require to implement Debug here?
pub trait ObsBootstrapStatusHandler: Debug + Send + Sync {
    fn handle_downloading(
        &mut self,
        progress: f32,
        message: String,
    ) -> Result<(), ObsBootstrapError>;
    fn handle_extraction(
        &mut self,
        progress: f32,
        message: String,
    ) -> Result<(), ObsBootstrapError>;
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
    fn handle_downloading(
        &mut self,
        progress: f32,
        message: String,
    ) -> Result<(), ObsBootstrapError> {
        if progress - self.last_download_percentage >= 0.05 || progress == 1.0 {
            self.last_download_percentage = progress;
            println!("Downloading: {}% - {}", progress * 100.0, message);
        }
        Ok(())
    }

    fn handle_extraction(
        &mut self,
        progress: f32,
        message: String,
    ) -> Result<(), ObsBootstrapError> {
        if progress - self.last_extract_percentage >= 0.05 || progress == 1.0 {
            self.last_extract_percentage = progress;
            println!("Extracting: {}% - {}", progress * 100.0, message);
        }
        Ok(())
    }
}
