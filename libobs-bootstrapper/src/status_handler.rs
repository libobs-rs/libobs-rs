use std::{convert::Infallible, fmt::Debug};

//NOTE: Maybe do not require to implement Debug here?
pub trait ObsBootstrapStatusHandler: Debug + Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    fn handle_downloading(
        &mut self,
        progress: f32,
        message: String,
    ) -> Result<(), Self::Error>;
    fn handle_extraction(
        &mut self,
        progress: f32,
        message: String,
    ) -> Result<(), Self::Error>;
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

    fn handle_downloading(
        &mut self,
        progress: f32,
        message: String,
    ) -> Result<(), Infallible> {
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
    ) -> Result<(), Infallible> {
        if progress - self.last_extract_percentage >= 0.05 || progress == 1.0 {
            self.last_extract_percentage = progress;
            println!("Extracting: {}% - {}", progress * 100.0, message);
        }
        Ok(())
    }
}
