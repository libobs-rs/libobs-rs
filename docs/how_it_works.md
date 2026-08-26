# How libobs-rs Works

`libobs-rs` is a set of crates that provide Rust bindings and progressively higher-level APIs around the OBS Studio C API (`libobs`).

## Architecture

### 1. `libobs`
- Raw FFI bindings to the native OBS library.
- Links `obs.dll` on Windows, `libobs.framework` on macOS, and system `libobs` on Linux.
- Generated from the OBS headers with `bindgen` when requested; checked bindings are used where supported.

### 2. `libobs-wrapper`
- Safe wrapper around `libobs`.
- Owns OBS runtime/threading, memory/lifecycle management, modules, scenes, sources, displays, encoders, and outputs.

### 3. `libobs-simple`
- Higher-level recording/streaming and source builders.
- Chooses platform encoders and capture sources while still allowing access to wrapper/libobs APIs.

### 4. `cargo-obs-build`
- Build/package-time preparation for Windows and macOS OBS runtimes.
- Selects architecture-specific official release assets and preserves the required bundle/plugin layout.
- Linux uses the system/source integration instead.

### 5. `libobs-bootstrapper`
- Optional, explicit runtime provisioning and local-runtime inspection.
- Does not depend on the native `libobs` crate, so a bootstrap helper can execute before OBS exists.
- Reuses `cargo-obs-build`'s platform preparation but requires checksum/digest verification for runtime downloads.
- Never injects a dummy DLL, starts PowerShell, or updates a DLL/framework after OBS is already loaded.

## Startup data flow

### Packaged application

1. `cargo-obs-build` or the application's installer prepares OBS before launch.
2. The operating-system loader resolves the native OBS library.
3. `libobs-wrapper` initializes OBS and loads modules.
4. `libobs-simple`/`libobs-wrapper` configure scenes, sources, encoders, and outputs.

### Windows runtime-bootstrap application

1. The final application is linked with `obs.dll` as a delay-loaded dependency.
2. `main()` starts without loading OBS.
3. The application explicitly calls `ObsBootstrapper::bootstrap()`.
4. A verified OBS runtime is prepared while no OBS DLL is mapped/locked.
5. The first subsequent `libobs` call causes Windows to load the real `obs.dll`.
6. Normal wrapper initialization continues.

### macOS runtime bootstrap

A small launcher that does not link libobs performs steps 3-4, then starts the real application. A directly linked application cannot bootstrap a completely missing required framework from its own `main()` because dyld resolves that dependency first.

### Linux

The process starts against the system/source `libobs`; the bootstrapper is not part of Linux runtime setup.
