use libobs_bootstrapper::{
    ObsBootstrapError, ObsBootstrapper, ObsBootstrapperOptions, ObsBootstrapperResult,
};

#[tokio::main]
async fn main() {
    let options = ObsBootstrapperOptions::default();
    // SAFETY: this provisioning-only call does not invoke libobs; on Windows
    // it prepares the delay-loaded runtime before the first direct OBS call.
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
        #[allow(ensure_obs_call_in_runtime)]
        let version = {
            // SAFETY: bootstrap completed above, so the delay-loaded OBS
            // runtime is available before this first direct FFI call.
            unsafe { libobs::obs_get_version() }
        };
        println!("Loaded OBS version word: {version:#x}");
    }

    #[cfg(target_os = "macos")]
    println!(
        "On macOS use this bootstrapper from a launcher/helper that does not itself link libobs, then start the real application."
    );
}
