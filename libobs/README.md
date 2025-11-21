# libobs-rs

This is currently just a wrapper around the C API of OBS Studio.
The stable API is available in the [`libobs-wrapper`](https://crates.io/crates/libobs-wrapper) crate.

## Building on Linux
The library needs OBS binaries in your target directory for Windows and MacOS.
If you want to target Linux, users (you included) must [build and install](https://github.com/obsproject/obs-studio/wiki/Build-Instructions-For-Linux) OBS Studio manually from source.
For Windows and Macos, there are multiple ways to set this up. Instructions can be viewed [here](https://github.com/libobs-rs/libobs-rs/blob/feat/flake/libobs-wrapper/README.md#prerequisites)

### Troubleshooting

If you encounter build errors:

1. Make sure all system dependencies are installed
2. Verify that clang is in your PATH: `clang --version`
3. If you have a custom OBS installation, set the `LIBOBS_PATH` environment variable to point to your OBS library directory
