# libobs-wrapper

[![Crates.io](https://img.shields.io/crates/v/libobs-wrapper.svg)](https://crates.io/crates/libobs-wrapper)

A safe, ergonomic Rust wrapper around the OBS (Open Broadcaster Software) Studio library. This crate provides a high-level interface for recording and streaming functionality using OBS's powerful capabilities, without having to deal with unsafe C/C++ code directly.

## Features

- **Thread Safety**: Uses a dedicated thread to communicate with OBS, allowing safe cross-thread usage
- **Resource Safety**: RAII-based resource management for OBS objects
- **Packaged Runtime Support**: Use build/package-time OBS preparation on Windows/macOS and system/source integration on Linux
- **Scene Management**: Create and manipulate scenes, sources, and outputs
- **Video Recording**: Configure and record video with various encoders
- **Audio Support**: Configure audio sources and encoders
- **Display Management**: Create and control OBS preview windows

## Prerequisites

The library needs OBS binaries in your target directory for Windows and MacOS.

If you want to target Linux, you'll need to build and install OBS Studio from source. This can be done on Ubuntu using the `cargo-obs-build` tool (using `cargo obs-build install`), or by following the [official OBS build instructions](https://github.com/obsproject/obs-studio/wiki/Build-Instructions-For-Linux). Users of your application can just install OBS Studio via their package manager directly (tested and working for version 30+ on Ubuntu)

On Linux:
When running the application and saving for example an replay buffer, the underlying `libobs` library will look at the current executables directory and tries to execute `obs-ffmpeg-mux` and `obs-nvenc-test`. Because they don't exist by default, the `libobs-wrapper` will create symlinks to the existing `obs-ffmpeg-mux` and `obs-nvenc-test` binaries which are found by searching the `PATH`.

For Windows and macOS, there are multiple ways to set this up. On macOS, install `simde` first (`brew install simde`) so the libobs headers can be processed:

### Option 1: Using cargo-obs-build

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
cargo obs-build build --out-dir target/debug

# For release builds
cargo obs-build build --out-dir target/release

# For testing
cargo obs-build build --out-dir target/(debug|release)/deps
```

More details can be found in the [cargo-obs-build documentation](../cargo-obs-build/README.md).

### Runtime bootstrapper

`libobs-bootstrapper` offers an explicit verified first-run/update flow in addition to build-time packaging with `cargo-obs-build`. On Windows, the final executable must delay-load `obs.dll` and call the bootstrapper before its first wrapper/libobs operation; the bootstrapper exposes `build::emit_windows_obs_delay_load()` for the application's `build.rs`. On macOS, use a separate launcher/helper when the framework may be absent, because dyld resolves a directly linked framework before `main()`. Linux continues to use the system/source `libobs`.

## Advanced Usage

For more advanced usage examples, check out:

- Monitor capture example with full configuration: [examples/monitor_capture](../examples/monitor-capture)

For even easier handling, consider using the [`libobs-simple`](https://crates.io/crates/libobs-simple) crate which
builds on top of this wrapper.

## Features
- `no_blocking_drops` - Spawns a tokio thread using `tokio::task::spawn_blocking`, so drops don't block your Application (experimental, make sure you have a tokio runtime running)
- `generate_bindings` - When enabled, forces the underlying bindings from `libobs` to generate instead of using the cached ones.
- `color-logger` - Enables coloring for the console. **On by default**.
- `dialog_crash_handler` - Adds a default crash handler, which shows the error and an option to copy the stacktrace to the clipboard. **On by default**. If turned off, OBS crashes will be reported via `stderr`, unless `logging_crash_handler` is enabled, in which case they will be reported via `log::error!`.
- `logging_crash_handler` - Sets the non-`dialog_crash_handler` default crash handler to report crashes via `log::error!`, instead of through `stderr`.

## Common Issues

### Missing DLLs or Crashes on Startup

If you're experiencing crashes or missing DLL errors:
1. Make sure OBS binaries are correctly installed using cargo-obs-build, your package/installer, or the Linux system integration
2. Check that you're using the correct OBS version compatible with this wrapper
3. Verify that all required DLLs are in your executable directory

### Memory Leaks

The library handles most memory management automatically, but you should avoid resetting the OBS context repeatedly as this can cause small memory leaks (due to an OBS limitation). There is `1` memory leak caused by `obs_add_data_path` (which is called internally from this lib). Unfortunately, this memory leak can not be fixed because of how OBS internally works.

## License

This project is licensed under the GPL-3.0 License - see the LICENSE file for details.

## Acknowledgments

- The OBS Project for the amazing OBS Studio software
- Contributors to the libobs-rs ecosystem