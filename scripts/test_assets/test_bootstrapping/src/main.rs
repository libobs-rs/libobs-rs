use std::{ffi::CString, ptr};

use libobs_bootstrapper::{ObsBootstrapper, ObsBootstrapperOptions, ObsBootstrapperResult};

#[tokio::main]
async fn main() {
    let res = ObsBootstrapper::bootstrap(&ObsBootstrapperOptions::default().set_no_restart())
        .await
        .unwrap();

    if matches!(res, ObsBootstrapperResult::Restart) {
        println!(
            "OBS has been downloaded and extracted. The application will now exit. You'll have to restart it yourself"
        );
        return;
    }

    let locale = CString::new("en-US").unwrap();
    println!("Locale pointer: {:?}", locale.as_ptr());

    let startup_result =
        unsafe { libobs::obs_startup(locale.as_ptr(), ptr::null(), ptr::null_mut()) };
    if !startup_result {
        panic!("error on libobs startup");
    }
    println!("OBS startup successful");

    unsafe {
        libobs::obs_shutdown();
        assert_eq!(libobs::bnum_allocs(), 0, "Memory leak detected");
    };
}
