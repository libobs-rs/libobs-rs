# libobs-bootstrapper

[![Crates.io](https://img.shields.io/crates/v/libobs-bootstrapper.svg)](https://crates.io/crates/libobs-bootstrapper)
[![Documentation](https://docs.rs/libobs-bootstrapper/badge.svg)](https://docs.rs/libobs-bootstrapper)

`libobs-bootstrapper` explicitly provisions and inspects OBS runtimes for applications built with libobs-rs. Runtime provisioning is **opt-in**: depending on this crate never downloads anything. Network access only begins when the application calls `ObsBootstrapper::bootstrap*`.

The current design deliberately avoids the old dummy-`obs.dll` and PowerShell updater flow. Downloads use official OBS Studio GitHub releases by default, select a compatible architecture/version, require an advertised SHA-256 checksum/digest, verify it before extraction, and prepare the runtime with the same platform-aware machinery used by `cargo-obs-build`.

## Platform model

- **Windows:** same-process first-run provisioning is supported when the final executable delay-loads `obs.dll`. This lets `main()` run before OBS exists and keeps the DLL unlocked until bootstrap has finished.
- **macOS:** provisioning is supported, but an application that directly links `libobs.framework` cannot bootstrap a missing framework from its own `main()` because dyld resolves it before the entry point. Use a small launcher/helper that depends on `libobs-bootstrapper` but not `libobs`, then start the real application.
- **Linux:** use the distribution/system or source-built `libobs`; portable runtime bootstrap is intentionally unsupported.

## Windows: bootstrap before the first OBS call

Microsoft's delay-load mechanism defers `obs.dll` loading until the first imported OBS function is called. Because Cargo cannot propagate a dependency's `rustc-link-arg` into the final application executable, add `libobs-bootstrapper` as a build dependency and call the helper from your application's own build script.

`Cargo.toml`:

```toml
[dependencies]
libobs-bootstrapper = "0.4"
libobs-wrapper = "9"

[build-dependencies]
libobs-bootstrapper = "0.4"
```

`build.rs`:

```rust
fn main() {
    libobs_bootstrapper::build::emit_windows_obs_delay_load();
}
```

Then bootstrap **before any call into `libobs` or `libobs-wrapper`**:

```rust,no_run
use libobs_bootstrapper::{ObsBootstrapper, ObsBootstrapperOptions};

# async fn prepare() -> Result<(), libobs_bootstrapper::ObsBootstrapError> {
let options = ObsBootstrapperOptions::default();
ObsBootstrapper::bootstrap(&options).await?;

// It is now safe to make the first OBS call / create ObsContext.
# Ok(())
# }
```

The default install directory is the executable directory, which also makes it discoverable by the Windows delay-load helper. If you use `set_install_dir` with a different directory, your application is responsible for adding that directory to the Windows DLL search path before the first OBS symbol is used.

If OBS was already loaded before `bootstrap()`, the bootstrapper returns `ObsBootstrapError::RuntimeAlreadyLoaded` rather than attempting to overwrite in-use DLLs.

## macOS launcher pattern

Use a tiny launcher executable that depends on `libobs-bootstrapper` only:

```rust,no_run
use libobs_bootstrapper::{ObsBootstrapper, ObsBootstrapperOptions};

# async fn prepare() -> Result<(), libobs_bootstrapper::ObsBootstrapError> {
ObsBootstrapper::bootstrap(&ObsBootstrapperOptions::default()).await?;
// Start the real libobs-linked application after provisioning succeeds.
# Ok(())
# }
```

Because the launcher does not depend on the native `libobs` crate, it can start when the framework is absent and can update the framework without it being mapped into the process.

## Update and trust policy

The secure default is `UpdateTargetMode::Exact`: provision the OBS ABI version this libobs-rs release targets. Callers can explicitly choose `LatestCompatibleSameMajorMinor` or `LatestCompatibleSameMajor` if they want a broader update line.

The default repository is `obsproject/obs-studio`. `set_repository` intentionally allows a different GitHub repository, but doing so is a caller-controlled trust decision. Runtime provisioning requires SHA-256 integrity metadata; unlike normal build-time preparation, it refuses downloads without a checksum/digest.

No bootstrap path creates a dummy DLL, launches PowerShell, replaces files after OBS has been loaded, or automatically restarts the application.

## Local inspection

Inspection APIs do not access the network:

```rust,no_run
use libobs_bootstrapper::{ObsBootstrapper, ObsBootstrapperOptions};

# fn inspect() -> Result<(), libobs_bootstrapper::ObsBootstrapError> {
let options = ObsBootstrapperOptions::new().set_install_dir("./runtime");
let valid = ObsBootstrapper::is_valid_installation_with_options(&options)?;
let needs_update = ObsBootstrapper::is_update_available_with_options(&options)?;
println!("valid={valid}, needs_update={needs_update}");
# Ok(())
# }
```

`is_update_available_with_options` compares the local runtime with the configured base target. Release-server resolution only occurs during the explicit bootstrap operation when a non-exact update mode is selected.

See [`examples/download-at-runtime`](../examples/download-at-runtime) for a complete example.
