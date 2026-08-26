use libobs_bootstrapper::{ObsBootstrapError, ObsBootstrapper, ObsBootstrapperOptions};

#[tokio::main]
async fn main() {
    let result = ObsBootstrapper::bootstrap(&ObsBootstrapperOptions::default().set_no_restart()).await;
    assert!(matches!(
        result,
        Err(ObsBootstrapError::RuntimeBootstrapDisabled)
    ));
}
