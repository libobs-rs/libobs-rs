use libobs_bootstrapper::{
    ObsBootstrapError, ObsBootstrapper, ObsBootstrapperOptions, ObsBootstrapperResult,
};

#[tokio::main]
async fn main() {
    let options = ObsBootstrapperOptions::default();
    match ObsBootstrapper::bootstrap(&options).await {
        Ok(ObsBootstrapperResult::None) => println!("OBS runtime is already ready"),
        Ok(ObsBootstrapperResult::Provisioned) => println!("OBS runtime was provisioned"),
        #[allow(deprecated)]
        Ok(ObsBootstrapperResult::Restart) => {
            unreachable!("the current bootstrapper never restarts")
        }
        Err(ObsBootstrapError::UnsupportedPlatform(message)) => {
            eprintln!("Runtime bootstrap is not used on this platform: {message}");
            return;
        }
        Err(error) => panic!("OBS bootstrap failed: {error}"),
    }

    #[cfg(target_os = "windows")]
    {
        // This is intentionally the first direct OBS call in the process. The
        // linker delay-load thunk resolves obs.dll only now, after bootstrap.
        let version = unsafe { libobs::obs_get_version() };
        println!("Loaded OBS version word: {version:#x}");
    }

    #[cfg(target_os = "macos")]
    println!(
        "On macOS use this bootstrapper from a launcher/helper that does not itself link libobs, then start the real application."
    );
}
