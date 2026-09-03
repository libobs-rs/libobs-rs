# libobs-wrapper

[![Crates.io](https://img.shields.io/crates/v/libobs-wrapper.svg)](https://crates.io/crates/libobs-wrapper)

A safe, ergonomic Rust wrapper around the OBS (Open Broadcaster Software) Studio library. This crate provides a high-level interface for recording and streaming functionality using OBS's powerful capabilities, without having to deal with unsafe C/C++ code directly.

## Where to start

Use `libobs-wrapper` when you want the full safe OBS object model rather than an opinionated workflow. If you mainly want “record this” or “stream this”, start with [`libobs-simple`](../libobs-simple/README.md) instead.

The most useful entry points are:

| Task | Module / type |
| --- | --- |
| Start and own libobs | `utils::StartupInfo`, `context::ObsContext` |
| Discover installed plugins/capabilities | `capabilities` |
| Configure arbitrary plugins | `settings` + capability descriptors |
| Create sources and filters | `sources` |
| Compose scenes, groups and transforms | `scenes` |
| Choose encoders/outputs/services | `capabilities`, `encoders`, `services` |
| Build/start outputs | `data::output` |
| Preview/render to native windows | `display` |
| Subscribe to OBS events | per-object `signals()` / `signals` |
| Reach raw libobs when necessary | `sys` (unsafe escape hatch) |

For a detailed task-to-module map, ownership explanation, and workflows, read the [API orientation guide](../docs/api_orientation.md).

### Preferred plugin-generic path

When plugin IDs may vary across machines, prefer:

1. `ObsContext::capabilities()` to discover what is installed.
2. `best_output_plan` / selectors to choose by codec, protocol, and capability instead of backend ID.
3. Descriptor `settings_schema_for` / `settings_snapshot_for` to configure arbitrary plugins from their runtime property tree.
4. `ObsContext::output_pipeline()` to validate the complete graph before native output creation.

The older typed property/builders remain useful for known plugins, but they should not be the only route to functionality.

## Features

- **Actor Runtime**: Uses one bounded dedicated-thread actor for libobs calls, with backpressure for fire-and-forget work
- **Resource Safety**: Native objects live in shared lifetime leases; a runtime-owned registry tracks opaque identities without serving as a fallible pointer lookup table
- **Non-blocking Cleanup**: Destructors enqueue native release operations without requiring Tokio or panicking on runtime shutdown
- **Per-object Signals**: Signal subscriptions are owned by each object; callback-only pointers are exposed as opaque identities
- **Runtime Bootstrapping**: Optional automatic download and setup of OBS binaries at runtime (functionality moved to [libobs-bootstrapper](https://crates.io/crates/libobs-bootstrapper))
- **Runtime Discovery & Selection**: Inspect plugin source/output/encoder/service metadata, plan compatible graphs with structured rejection diagnostics, and select by codec/protocol/capability instead of backend ID
- **Generic Plugin Settings**: Build dynamic schemas with ranges/lists/visibility plus current/default typed values, validated through OBS property callbacks
- **Validated Output Pipelines**: Validate codecs, protocols, runtime affinity, required components, and mixer indices before output creation, with lower-level desired-state composition still available
- **Scene Management**: Create typed scene items and native groups; inspect native order; snapshot/restore position, scale, bounds, crop, blend state, visibility, locking, and ordering
- **Video Recording**: Configure and record video with various encoders
- **Audio Support**: Configure audio sources and encoders
- **Display Management**: Create and control OBS preview windows

## Prerequisites

The library needs compatible OBS binaries/libraries for the selected target. Windows can be prepared with `cargo-obs-build`; Linux uses a system/source libobs installation. macOS wrapper support is still incomplete.

If you want to target Linux, you'll need to build and install OBS Studio from source. This can be done on Ubuntu using the `cargo-obs-build` tool (using `cargo obs-build install`), or by following the [official OBS build instructions](https://github.com/obsproject/obs-studio/wiki/Build-Instructions-For-Linux). Users of your application can just install OBS Studio via their package manager directly (tested and working for version 30+ on Ubuntu)

On Linux:
When running the application and saving for example an replay buffer, the underlying `libobs` library will look at the current executables directory and tries to execute `obs-ffmpeg-mux` and `obs-nvenc-test`. Because they don't exist by default, the `libobs-wrapper` will create symlinks to the existing `obs-ffmpeg-mux` and `obs-nvenc-test` binaries which are found by searching the `PATH`.

For Windows and Macos, there are multiple ways to set this up:

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

### Option 2: Using the OBS Bootstrapper
You can also download OBS binaries at runtime using the [libobs-bootstrapper](https://crates.io/crates/libobs-bootstrapper) crate, which provides a convenient API for downloading and setting up OBS without needing to include it in your build process. This is useful if you want to keep your application lightweight.
See the [libobs-bootstrapper documentation](https://docs.rs/libobs-bootstrapper) for detailed setup instructions and examples of implementing custom progress handlers.

## Advanced Usage

For more advanced usage examples, check out:

- Monitor capture example with full configuration: [examples/monitor_capture](../examples/monitor-capture)
- Runtime bootstrapping example: [examples/download-at-runtime](../examples/download-at-runtime)

For even easier handling, consider using the [`libobs-simple`](https://crates.io/crates/libobs-simple) crate which
builds on top of this wrapper.

## Features
- `no_blocking_drops` - Deprecated compatibility feature. Native cleanup is now always deferred to the OBS actor without requiring Tokio.
- `generate_bindings` - When enabled, forces the underlying bindings from `libobs` to generate instead of using the cached ones.
- `color-logger` - Enables coloring for the console. **On by default**.
- `dialog_crash_handler` - Adds a default crash handler, which shows the error and an option to copy the stacktrace to the clipboard. **On by default**. If turned off, OBS crashes will be reported via `stderr`, unless `logging_crash_handler` is enabled, in which case they will be reported via `log::error!`.
- `logging_crash_handler` - Sets the non-`dialog_crash_handler` default crash handler to report crashes via `log::error!`, instead of through `stderr`.

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