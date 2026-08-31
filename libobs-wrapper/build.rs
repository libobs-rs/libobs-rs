fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "linux" {
        println!("cargo:rustc-link-lib=X11");
        println!("cargo:rustc-link-lib=wayland-client");
    }
}
