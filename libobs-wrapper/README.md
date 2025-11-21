# libobs-wrapper

[![Crates.io](https://img.shields.io/crates/v/libobs-wrapper.svg)](https://crates.io/crates/libobs-wrapper)
[![Documentation](https://docs.rs/libobs-wrapper/badge.svg)](https://docs.rs/libobs-wrapper)

A safe, ergonomic Rust wrapper around the OBS (Open Broadcaster Software) Studio library. This crate provides a high-level interface for recording and streaming functionality using OBS's powerful capabilities, without having to deal with unsafe C/C++ code directly.

## Features

- **Thread Safety**: Uses a dedicated thread to communicate with OBS, allowing safe cross-thread usage
- **Async API**: Full async support with optional blocking API (via `blocking` feature)
- **Resource Safety**: RAII-based resource management for OBS objects
- **Runtime Bootstrapping**: Optional automatic download and setup of OBS binaries at runtime
- **Scene Management**: Create and manipulate scenes, sources, and outputs
- **Video Recording**: Configure and record video with various encoders
- **Audio Support**: Configure audio sources and encoders
- **Display Management**: Create and control OBS displays

## Prerequisites

The library needs OBS binaries in your target directory for Windows and MacOS.
If you want to target Linux, users (you included) must [build and install](https://github.com/obsproject/obs-studio/wiki/Build-Instructions-For-Linux) OBS Studio manually from source.
For Windows and Macos, there are multiple ways to set this up:

### Option 1: Using cargo-obs-build (Recommended for development)

Install the `cargo-obs-build` tool:

```bash
cargo install cargo-obs-build
```

Add the following to your `Cargo.toml`:

```toml
[package.metadata]
# The libobs version to use (can either be a specific version or "latest")
# If not specified, the version will be selected based on the libobs crate version.
# libobs-version = "31.0.3"
# Optional: The directory to store the OBS build
# libobs-cache-dir = "../obs-build"
```

Install OBS in your target directory:

```bash
# For debug builds
cargo obs-build --out-dir target/debug

# For release builds
cargo obs-build --out-dir target/release

# For testing
cargo obs-build --out-dir target/(debug|release)/deps
```

More details can be found in the [cargo-obs-build documentation](./cargo-obs-build/README.md).

### Option 2: Using the OBS Bootstrapper (Recommended for distribution)

For applications that need to bundle OBS binaries or handle runtime installation, we recommend using the [libobs-bootstrapper](https://crates.io/crates/libobs-bootstrapper) crate.

This separate crate provides functionality to download and install OBS binaries at runtime, which is particularly useful for distributing applications without requiring users to install OBS separately.

See the [libobs-bootstrapper documentation](https://docs.rs/libobs-bootstrapper) for detailed setup instructions and examples of implementing custom progress handlers.

## Advanced Usage

For more advanced usage examples, check out:
- Monitor capture example with full configuration: [examples/monitor_capture.rs](../examples/monitor_capture.rs)
- Tauri integration example: [examples/tauri-app](../examples/tauri-app)
- Runtime bootstrapping example: [examples/download-at-runtime](../examples/download-at-runtime)

For even easier source creation and management, consider using the [`libobs-sources`](https://crates.io/crates/libobs-sources) crate which builds on top of this wrapper.

## Features
- `no_blocking_drops` - Spawns a tokio thread using `tokio::task::spawn_blocking`, so drops don't block your Application (experimental, make sure you have a tokio runtime running)
- `generate_bindings` - When enabled, forces the underlying bindings from `libobs` to generate instead of using the cached ones.
- `color-logger` - Enables coloring for the console
- `dialog_crash_handler` - Adds a default crash handler, which shows the error and an option to copy the stacktrace to the clipboard

## Common Issues

### Missing DLLs or Crashes on Startup

If you're experiencing crashes or missing DLL errors:
1. Make sure OBS binaries are correctly installed using either cargo-obs-build or the bootstrapper
2. Check that you're using the correct OBS version compatible with this wrapper
3. Verify that all required DLLs are in your executable directory

### Memory Leaks

The library handles most memory management automatically, but you should avoid resetting the OBS context repeatedly as this can cause small memory leaks (due to an OBS limitation). There is `1` memory leak caused by `obs_add_data_path` (which is called internally from this lib). Unfortunately, this memory leak can not be fixed because of how OBS internally works.

## License

This project is licensed under the GPL-3.0 License - see the LICENSE file for details.

## Acknowledgments

- The OBS Project for the amazing OBS Studio software
- Contributors to the libobs-rs ecosystem