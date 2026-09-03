use libobs_bootstrapper::{ObsBootstrapper, ObsBootstrapperOptions};

#[cfg(target_os = "linux")]
use libobs_bootstrapper::ObsBootstrapError;

#[cfg(target_os = "linux")]
#[tokio::test]
async fn linux_runtime_bootstrap_is_explicitly_unsupported() {
    let result = ObsBootstrapper::bootstrap(&ObsBootstrapperOptions::default()).await;
    assert!(matches!(
        result,
        Err(ObsBootstrapError::UnsupportedPlatform(_))
    ));
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
#[test]
fn missing_runtime_is_reported_without_network_io() {
    let install_dir = std::env::temp_dir().join(format!(
        "libobs-bootstrapper-missing-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let options = ObsBootstrapperOptions::default().set_install_dir(install_dir);
    assert!(!ObsBootstrapper::is_valid_installation_with_options(&options).unwrap());
}
