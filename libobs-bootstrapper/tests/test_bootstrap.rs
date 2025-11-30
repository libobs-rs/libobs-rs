#![cfg(not(target_os = "linux"))]

use std::{
    thread::sleep,
    time::{Duration, Instant},
};

#[test]
fn test_build_script_project() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let project_dir = manifest_dir
        .join("..")
        .join("scripts")
        .join("test_assets")
        .join("test_bootstrapping");

    let obs_new = project_dir.join("target").join("debug").join("obs_new");

    // Clean previous build artifacts
    std::process::Command::new("cargo")
        .args(["clean", "--manifest-path"])
        .arg(project_dir.join("Cargo.toml"))
        .status()
        .expect("Failed to clean previous build artifacts");

    let status = std::process::Command::new("cargo")
        .args(["run", "--manifest-path"])
        .arg(project_dir.join("Cargo.toml"))
        .status()
        .expect("Failed to run cargo build on test project");

    assert!(status.success(), "Cargo run failed for test project");

    // Wait until the "obs_new" directory is deleted...

    let timeout = Duration::from_secs(30);
    let interval = Duration::from_millis(200);
    let start = Instant::now();
    while obs_new.exists() {
        if start.elapsed() >= timeout {
            panic!("Timeout waiting for deletion of {:?}", obs_new);
        }
        sleep(interval);
    }

    // Running again should be instant as libobs is already bootstrapped
    let status = std::process::Command::new("cargo")
        .args(["run", "--manifest-path"])
        .arg(project_dir.join("Cargo.toml"))
        .status()
        .expect("Failed to run cargo build on test project");
    assert!(status.success(), "Cargo run failed for test project");
}
