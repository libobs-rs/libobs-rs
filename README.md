# libobs-rs
![Build](https://img.shields.io/github/actions/workflow/status/libobs-rs/libobs-rs/validation.yml?branch=main&label=build&style=for-the-badge)
[![Docs](https://img.shields.io/badge/docs-passing-brightgreen?style=for-the-badge)](https://libobs-rs.github.io/libobs-docs/libobs_wrapper/)
![Coverage](https://img.shields.io/badge/coverage-55%25-orange?style=for-the-badge)

Documentation is available [here](https://libobs-rs.github.io/libobs-docs/libobs/)

> [!NOTE]
> Need help? [Join our discord server!](https://discord.gg/rsTffTMPMF) 


Simple and safe video recording through libobs.

Windows and Linux (Ubuntu Wayland / X11) are supported, and macOS support now includes native framework/plugin loading, official OBS DMG preparation, Cocoa previews, ScreenCaptureKit-backed capture sources, and dynamic VideoToolbox hardware encoding. A dedicated native macOS CI job validates the platform after changes are pushed.
The API is currently unstable and will definitely have breaking revisions in the future.

> [!NOTE]
> The libobs-wrapper async functionality has been removed because of all kinds of issues ([#32](https://github.com/libobs-rs/libobs-rs/issues/32))


## Prerequisites

### macOS

Install SIMDe so the bundled libobs headers can be processed, then let `cargo-obs-build` prepare the official OBS DMG into your target directory:

```bash
brew install simde
cargo obs-build build --out-dir target/debug/deps
```

`libobs-simple` exposes the native OBS `screen_capture` source on macOS for display, window, and application capture.

### Linux

Linux keeps using a system/source OBS installation rather than unpacking a portable runtime. On Ubuntu, `cargo obs-build install` builds and installs a compatible OBS; on other distributions use the distro packages or the official OBS build instructions.

### Build helper

Make sure that the OBS binaries are in your target directory on Windows/macOS. The helper can prepare them for you. <br>
Install the tool
```bash
cargo install cargo-obs-build
```

> [!NOTE]
> `libobs-bootstrapper` can explicitly provision a verified OBS runtime at first run on Windows/macOS. On Windows, same-process bootstrap requires delay-loading `obs.dll`; on macOS, use a small launcher/helper if the framework may be missing. Nothing downloads implicitly. Linux continues to use the system/source `libobs`. See [OBS Runtime Setup](./docs/bootstrap_options.md).

Add the following to your `Cargo.toml`
```toml
[package.metadata]
# The libobs version to use (can either be a specific version or "latest")
# This is optional; if not specified, the version will be selected based on the libobs crate version.
# libobs-version="31.0.3"
# The directory in which to store the OBS build (optional)
# libobs-cache-dir="../obs-build"

```

Install OBS in your target directory. This uses the original signed OBS binaries.
```bash
# for debugging
cargo obs-build build --out-dir target/debug
# for release
cargo obs-build build --out-dir target/release
# for testing
cargo obs-build build --out-dir target/(debug|release)/deps
```

More details can be found in the [cargo-obs-build documentation](./cargo-obs-build/README.md).

> [!NOTE]
> You can specify a `GITHUB_TOKEN` environment variable to increase the rate limit when downloading releases from GitHub. This is especially useful for CI environments.


## Quick Start

Below is an example that will record video-only footage of an exclusive fullscreen application. Note that the API is extremely limited right now, but you can already record both video and audio with full control over the output already. If you need more, libobs is exposed.

Examples are located in the [examples](./examples) directory.
Documentation is also available for [libobs-simple](libobs-simple/README.md)
or [libobs-wrapper](./libobs-wrapper/README.md).

## Documentation
- [Bootstrap Options](./docs/bootstrap_options.md)
- [How it Works](./docs/how_it_works.md)

## Disclaimer

This project is **not affiliated with**, **endorsed by**, or **associated with** the OBS Project or OBS Studio.  
**OBS** and **OBS Studio** are trademarks of their respective owners.  
The developers of this project are independent and **not part of the OBS Studio team** in any capacity.

