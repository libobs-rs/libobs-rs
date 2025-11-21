#[cfg(any(not(feature = "install_dummy_dll"), not(target_os = "windows")))]
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}

#[cfg(all(feature = "install_dummy_dll", target_os = "windows"))]
fn main() {
    use std::path::PathBuf;
    println!("cargo:rerun-if-changed=build.rs");
    let dll = include_bytes!("./assets/obs-dummy.dll");

    // Cargo target directory (one level up from OUT_DIR)
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let target_dir = out_dir
        .ancestors()
        .nth(3) // up from target/<profile>/build/<crate>/out
        .unwrap();

    let obs_dll_file = target_dir.join("obs.dll");
    if !obs_dll_file.exists() {
        std::fs::write(obs_dll_file, dll).unwrap();
    }
}
