# Example build.rs for using cargo-obs-build as a library

**This crate supports Windows and MacOS for build only!**

This demonstrates how to use the `cargo-obs-build` library in a build script to automatically download and install OBS binaries.

## Simple Usage (Recommended)

The simplest way to use the library is with the `install()` function:

```rust
fn main() {
    cargo_obs_build::install().expect("Failed to install OBS binaries");
}
```

That's it! This will automatically:
- Install OBS binaries to the target directory
- Auto-detect the correct OBS version from your `libobs` dependency
- Handle caching to avoid re-downloads
- Set up proper locking for concurrent builds

## Advanced Usage

For more control, you can use the `build_obs_binaries()` function with custom configuration:

```rust
use cargo_obs_build::{build_obs_binaries, ObsBuildConfig};
use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = PathBuf::from(&out_dir)
        .parent().unwrap()
        .parent().unwrap()
        .parent().unwrap();

    let config = ObsBuildConfig {
        out_dir: target_dir,
        cache_dir: PathBuf::from("obs-build"),
        browser: true, // Include browser support
        ..Default::default()
    };

    build_obs_binaries(config).expect("Failed to build OBS binaries");
}
```

## Adding to your Cargo.toml

Add this to your `[build-dependencies]`:

```toml
[build-dependencies]
cargo-obs-build = { version = "1.2.4", default-features = false }
```

Note: We use `default-features = false` to avoid pulling in CLI-specific dependencies (clap, colored, fern) that aren't needed in a build script.

## Customization

You can customize the build by modifying the `ObsBuildConfig`:

- `out_dir`: Where to copy the final binaries (this should be the same directory as your executable)
- `cache_dir`: Where to cache downloaded files (useful for CI caching and to prevent re-downloads)
- `repo_id`: Use a different OBS repository
- `browser`: Include browser support in the binaries
- `tag`: Specify a specific OBS version (or "latest"). If set to none, it will auto-detect from your `libobs` dependency.
- `rebuild`: Force a rebuild even if cached (it's called `rebuild` but it actually just re-downloads and reinstalls)
- `skip_compatibility_check`: Skip version compatibility warnings
- `remove_pdbs`: Remove .pdb files from the final output to reduce size

## Workspace Metadata

You can also configure the library version and cache directory in your `Cargo.toml`:

```toml
[workspace.metadata]
libobs-version = "30.2.2"  # Specific version to use
libobs-cache-dir = "../obs-build"  # this is relative to your Cargo.toml
```

These settings will be automatically picked up by the library if the default install function is used.
