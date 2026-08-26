use libobs_bootstrapper::{ObsBootstrapError, ObsBootstrapper, ObsBootstrapperOptions};

#[tokio::test]
async fn public_runtime_bootstrap_api_is_disabled() {
    let options = ObsBootstrapperOptions::default()
        .set_repository("example.invalid/should-never-be-contacted")
        .set_no_restart();

    assert!(matches!(
        ObsBootstrapper::bootstrap(&options).await,
        Err(ObsBootstrapError::RuntimeBootstrapDisabled)
    ));
}
