#![cfg(not(target_os = "linux"))]

#[test]
fn test_build_script_project() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let project_dir = manifest_dir
        .join("..")
        .join("scripts")
        .join("test_assets")
        .join("test_cargo_obs_build");

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
}
