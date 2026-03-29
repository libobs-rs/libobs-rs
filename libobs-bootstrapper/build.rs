#[cfg(any(not(feature = "install_dummy_dll"), not(target_os = "windows")))]
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}

// The dummy size will less than 250kb (right now its at 60kb), so we are just checking if the file is smaller than 250kb to determine if it's a dummy or not. This is to prevent accidentally overwriting a real obs.dll with a dummy one.
pub const DUMMY_DLL_SIZE_THRESHOLD: u64 = 250 * 1024; // 250 KB

#[cfg(all(feature = "install_dummy_dll", target_os = "windows"))]
fn main() {
    use std::path::PathBuf;
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=./assets/obs-dummy.dll");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");

    if std::env::var("TOP_SECRET_NO_DUMMY_DLL").is_ok() {
        return;
    }

    let dll = include_bytes!("./assets/obs-dummy.dll");

    // Cargo target directory (one level up from OUT_DIR)
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let target_dir = out_dir
        .ancestors()
        .nth(3) // up from target/<profile>/build/<crate>/out
        .unwrap();

    let obs_dll_file = target_dir.join("obs.dll");
    let is_dummy_dll_file =
        obs_dll_file.metadata().map(|m| m.len()).unwrap_or(0) <= DUMMY_DLL_SIZE_THRESHOLD;

        if obs_dll_file.exists() && !is_dummy_dll_file {
        println!(
            "cargo:warning=obs.dll already exists at {:?}, skipping dummy DLL installation.",
            obs_dll_file
        );
        return;
    }

    std::fs::write(obs_dll_file, dll).unwrap();
}
