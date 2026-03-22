use std::{path::PathBuf, process::Command};

fn main() {
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        println!("cargo:rustc-link-lib=X11");
        println!("cargo:rustc-link-lib=wayland-client");
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "linux" {
        setup_symlinks();
    }
}

fn setup_symlinks() {
    // Cargo target directory (one level up from OUT_DIR)
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let target_dir = out_dir
        .ancestors()
        .nth(3) // up from target/<profile>/build/<crate>/out
        .unwrap();

    let executables = ["obs-nvenc-test", "obs-ffmpeg-mux"];
    for exe in &executables {
        let link_name = target_dir.join(exe);
        let target = which::which(exe).unwrap_or_else(|_| {
            panic!(
                "Executable {} not found in PATH. Please ensure it is built and available.",
                exe
            )
        });

        Command::new("ln")
            .args(["-s", target.to_str().unwrap(), link_name.to_str().unwrap()])
            .status()
            .expect("Failed to create symlink");
    }
}
