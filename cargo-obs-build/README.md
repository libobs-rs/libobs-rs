# cargo-obs-build

A library and CLI tool for building and installing libOBS binaries. It automatically downloads the correct version of OBS Studio binaries based on your `libobs` crate version, handling caching and version compatibility.

Note: On Linux, you must build OBS Studio from source and install it manually. This can be automatically be done by `cargo-obs-build` on `Ubuntu`, just run `cargo-obs-build install`. For other platforms refer to these [Build Instructions For Linux](https://github.com/obsproject/obs-studio/wiki/Build-Instructions-For-Linux).

For Windows and macOS, this tool will download prebuilt binaries.

## Usage

### As a CLI Tool

The CLI tool automatically prepares the environment by putting the required OBS libraries in the target directory.

```bash
cargo install cargo-obs-build
cargo obs-build build --out-dir ./target/debug # or ./target/release, this should be the directory where your binary will be built
```

Run `cargo obs-build --help` for all available options.

### As a Library in Build Scripts

You can use `cargo-obs-build` as a library in your `build.rs` to automatically download and install OBS binaries during the build process.

Add to your `Cargo.toml`:

```toml
[build-dependencies]
cargo-obs-build = { version = "1.2.4", default-features = false }
```

**Simple Usage (Recommended)**:

```rust
fn main() {
    cargo_obs_build::install().expect("Failed to install OBS binaries");
}
```

**Advanced Usage** with custom configuration:

```rust
use cargo_obs_build::{build_obs_binaries, ObsBuildConfig};
use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let target_dir = PathBuf::from(&out_dir)
        .parent().unwrap()
        .parent().unwrap()
        .parent().unwrap()
        .join("obs-binaries");

    let config = ObsBuildConfig {
        out_dir: target_dir,
        browser: true, // Include browser support
        ..Default::default()
    };

    build_obs_binaries(config).expect("Failed to build OBS binaries");
}
```

See [BUILD_SCRIPT_EXAMPLE.md](BUILD_SCRIPT_EXAMPLE.md) for more examples and detailed explanations.

## Configuration

### Cargo.toml Metadata

You can configure the tool using workspace or package metadata:

```toml
[package.metadata] # Can also be [workspace.metadata]
libobs-version = "30.2.2"
libobs-cache-dir = "../obs-build" # Optional, defaults to "obs-build", relative to the Cargo.toml file
```

### Environment Variables

- `GITHUB_TOKEN`: Provide a GitHub token to increase the API rate limit. This is especially useful for CI environments.
- `RUST_LOG`: Set the logging level (e.g., `RUST_LOG=debug`).

## Features

- **Automatic Version Detection**: Automatically selects the correct OBS version based on your `libobs` crate version
- **Smart Caching**:
  - Downloads are cached to avoid re-downloading binaries
  - GitHub API responses are cached to prevent rate limiting
  - Respects CI environment and warns if caching is not configured
- **Locking**: Prevents concurrent builds from interfering with each other
- **Version Compatibility**: Checks version compatibility between libobs crate and binaries
- **CI-Aware**: Detects CI environments and provides helpful warnings about GITHUB_TOKEN and caching setup
- **Flexible**: Can be used as both a CLI tool and a library

## Caching for CI

Cache both the `obs-build` directory and the API cache to avoid re-downloading binaries and reduce API calls:

**The library automatically:**
- Caches GitHub API responses in `obs-build/.api-cache/` (1 day expiration)
- Warns you in CI if GITHUB_TOKEN is not set
- Warns you in CI if the cache directory doesn't exist

**GitHub Actions Example:**
```yaml
- uses: actions/cache@v3
  with:
    path: |
      obs-build
    key: obs-build-${{ runner.os }}-${{ hashFiles('**/Cargo.lock') }}
```

For detailed CI setup instructions, see [CI_SETUP.md](CI_SETUP.md).

## License

MIT