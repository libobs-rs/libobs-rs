# OBS Runtime Setup

`libobs-rs` requires the OBS runtime to be installed and authenticated before the application starts. Runtime network bootstrapping is intentionally disabled: downloading a mutable "latest compatible" release from inside the process cannot establish trustworthy provenance before native OBS libraries are loaded.

## Windows and macOS: build/package-time setup

Use `cargo-obs-build` during development, CI, or packaging. It selects the requested target architecture, downloads the matching OBS asset, verifies an advertised release checksum/digest when available, and prepares the runtime layout.

```bash
cargo install cargo-obs-build
cargo obs-build build --out-dir target/debug
```

For test binaries, prepare `target/debug/deps` as well when needed. On macOS the helper uses the official architecture-specific OBS DMG and must run on a native macOS host for DMG extraction.

## Linux: system/source integration

Linux intentionally does not unpack an arbitrary distribution `.deb` as a portable runtime. Install a compatible `libobs` through your distribution/package manager or build OBS from source; on supported Ubuntu setups `cargo obs-build install` can perform the source/system installation flow.

## Inspecting a packaged runtime

`libobs-bootstrapper` retains local installation/version inspection helpers. `ObsBootstrapperOptions::set_install_dir` is honored by the corresponding `*_with_options` helpers. Its `bootstrap` and `bootstrap_with_handler` methods always return `RuntimeBootstrapDisabled` and perform no network/extraction work.

This model makes updates an explicit packaging/deployment decision instead of executable code fetched implicitly during application startup.
