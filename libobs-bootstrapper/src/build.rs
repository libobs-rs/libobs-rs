/// Emits the linker flags required to delay-load `obs.dll` on Windows.
///
/// Call this from the *application's* `build.rs` when the executable links
/// `libobs`/`libobs-wrapper` directly and wants to bootstrap OBS before the
/// first OBS symbol is used. Cargo does not propagate `rustc-link-arg` from a
/// dependency's build script to the final executable, so this helper must run
/// in the final application's build script.
///
/// On non-Windows targets this is a no-op so a cross-platform build script can
/// call it unconditionally.
///
/// # Panics
///
/// Panics for Windows toolchains whose delay-load linker convention is not
/// supported by this helper.
pub fn emit_windows_obs_delay_load() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_env == "msvc" {
        println!("cargo:rustc-link-lib=dylib=delayimp");
        println!("cargo:rustc-link-arg=/DELAYLOAD:obs.dll");
    } else {
        panic!("libobs-bootstrapper delay loading currently supports the Windows MSVC target");
    }
}
