//! Legacy runtime-bootstrap example.
//!
//! Network bootstrap is intentionally disabled. Keep this example as an explicit
//! migration check for applications that previously called the API.

use libobs_bootstrapper::{ObsBootstrapError, ObsBootstrapper, ObsBootstrapperOptions};

#[tokio::main]
async fn main() {
    let result = ObsBootstrapper::bootstrap(&ObsBootstrapperOptions::default()).await;
    match result {
        Err(ObsBootstrapError::RuntimeBootstrapDisabled) => {
            eprintln!(
                "Runtime OBS bootstrap is disabled. Prepare OBS before startup with cargo-obs-build, a signed package, or the Linux system integration."
            );
        }
        Err(error) => panic!("unexpected bootstrap error: {error}"),
        Ok(_) => panic!("runtime bootstrap unexpectedly became active"),
    }
}
