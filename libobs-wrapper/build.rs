fn main() {
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        println!("cargo:rustc-link-lib=X11");
        println!("cargo:rustc-link-lib=wayland-client");
    }
}
