# OBS Runtime Setup and Bootstrapping

libobs-rs supports two complementary ways to provide OBS binaries. Packaging at build/install time remains the simplest choice for conventional desktop applications, while `libobs-bootstrapper` provides an explicit first-run/update flow for applications that want to provision OBS themselves.

## Build/package-time setup

`cargo-obs-build` downloads the matching official OBS release, prepares the platform-specific runtime layout, and copies it into the requested output directory.

```bash
cargo install cargo-obs-build
cargo obs-build build --out-dir target/debug
```

On macOS this uses the official architecture-specific DMG and therefore requires a native macOS host for extraction. Linux uses a system/source installation instead of treating a distribution package as a portable runtime.

## Explicit runtime bootstrap

`ObsBootstrapper::bootstrap()` is the opt-in network boundary. Merely linking `libobs-bootstrapper`, calling installation inspection helpers, or using `libobs-wrapper` does not initiate a download.

The runtime bootstrapper defaults to:

- the official `obsproject/obs-studio` release repository,
- the exact OBS version targeted by this libobs-rs release,
- architecture-specific official assets,
- mandatory advertised SHA-256 checksum/digest verification,
- no process restart and no external updater script.

```rust,no_run
use libobs_bootstrapper::{ObsBootstrapper, ObsBootstrapperOptions};

# async fn bootstrap() -> Result<(), libobs_bootstrapper::ObsBootstrapError> {
ObsBootstrapper::bootstrap(&ObsBootstrapperOptions::default()).await?;
# Ok(())
# }
```

### Update policy

`UpdateTargetMode::Exact` is the default and most predictable policy. `LatestCompatibleSameMajorMinor` and `LatestCompatibleSameMajor` are explicit opt-ins for applications that want newer releases within a broader OBS compatibility line.

`set_update(false)` means an existing runtime will not be replaced. A missing runtime may still be installed. `set_repository(...)` is supported, but changing the official repository is an explicit trust decision by the caller.

## Windows: why delay loading is required

A normally linked Windows executable imports `obs.dll` before `main()` runs. If the DLL is missing, the OS loader terminates startup before Rust code can call the bootstrapper. The previous workaround used a dummy DLL plus a PowerShell updater because a loaded DLL also cannot be replaced safely.

The current design instead uses the MSVC `/DELAYLOAD:obs.dll` mechanism. The executable can enter `main()` without OBS, bootstrap/update the verified runtime, and only then trigger the real DLL load on its first OBS call.

Add the bootstrapper as a build dependency:

```toml
[build-dependencies]
libobs-bootstrapper = "0.4"
```

and in the final application's `build.rs`:

```rust
fn main() {
    libobs_bootstrapper::build::emit_windows_obs_delay_load();
}
```

This helper must be called by the final executable crate because Cargo does not forward a dependency crate's `rustc-link-arg` to a downstream binary's linker invocation.

Bootstrap must complete before any `libobs`/`libobs-wrapper` function is called. If `obs.dll` is already loaded, the bootstrapper returns `RuntimeAlreadyLoaded` rather than trying to update locked files.

## macOS: use a launcher for missing-runtime bootstrap

A libobs-linked macOS executable has the analogous startup problem at the Mach-O/dyld layer: a missing required `libobs.framework` prevents the application entry point from running. The recommended runtime-bootstrap architecture is therefore a small launcher/helper that depends on `libobs-bootstrapper` but **not** on `libobs` or `libobs-wrapper`. It provisions the runtime and then launches the real application.

Build/package-time bundling with `cargo-obs-build` is still preferred when a separate launcher is undesirable.

## Linux

Runtime downloading is intentionally not offered on Linux. Install a compatible `libobs` with the distribution/package manager or build OBS from source. On supported Debian/Ubuntu setups, `cargo obs-build install` can perform the source/system installation flow.

## Local inspection

`is_valid_installation*` and `is_update_available*` are local-only helpers. `ObsBootstrapperOptions::set_install_dir` selects the runtime to inspect; `set_cache_dir` selects where verified release preparation is cached.

For the implementation details and platform data flow, see [How libobs-rs Works](./how_it_works.md).
