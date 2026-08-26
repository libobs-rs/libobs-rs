use libobs_bootstrapper::{ObsBootstrapper, ObsBootstrapperOptions};

#[tokio::main]
async fn main() {
    println!("bootstrap fixture reached main");

    if std::env::var_os("BOOTSTRAP_OBS").is_some() {
        ObsBootstrapper::bootstrap(&ObsBootstrapperOptions::default())
            .await
            .expect("runtime bootstrap failed");
    }

    if std::env::var_os("CALL_OBS").is_some() {
        let version = unsafe { libobs::obs_get_version() };
        println!("obs version word: {version:#x}");
    }
}
