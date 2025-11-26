# libobs-bootstrapper

[![Crates.io](https://img.shields.io/crates/v/libobs-bootstrapper.svg)](https://crates.io/crates/libobs-bootstrapper)
[![Documentation](https://docs.rs/libobs-bootstrapper/badge.svg)](https://docs.rs/libobs-bootstrapper)

A utility crate for automatically downloading and installing OBS (Open Broadcaster Software) Studio binaries at runtime. This crate is part of the libobs-rs ecosystem and is designed to make distributing OBS-based applications easier by handling the setup of OBS binaries.

Note: This crate currently supports Windows and MacOS platforms. Refer to the libobs-wrapper documentation for Linux setup instructions [here](../libobs-wrapper/README.md).

## Features

- **Automatic OBS Download**: Downloads appropriate OBS binaries at runtime
- **Progress Tracking**: Built-in progress reporting for downloads and extraction
- **Version Management**: Handles OBS version checking and updates
- **Custom Status Handlers**: Flexible progress reporting via custom handlers
- **Async Support**: Built on Tokio for async operations
- **Error Handling**: Comprehensive error types for reliable error handling

## Usage

Add the crate to your dependencies:

```toml
[dependencies]
libobs-bootstrapper = "0.1.0"
async-trait = "0.1"  # For implementing the bootstrap status handler
```

### Basic Example

Here's a simple example using the default console handler:

```rust
use libobs_bootstrapper::{
    ObsBootstrapper,
    ObsBootstrapperOptions,
    ObsBootstrapConsoleHandler,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure bootstrapper options
    let options = ObsBootstrapperOptions::default();

    // Run bootstrap with default console handler
    match ObsBootstrapper::bootstrap(&options).await? {
        ObsBootstrapperResult::None => {
            println!("OBS is already installed and up to date!");
        }
        ObsBootstrapperResult::Restart => {
            println!("OBS has been updated. Restarting application...");
            ObsBootstrapper::spawn_updater(options).await?;
            std::process::exit(0);
        }
    }

    Ok(())
}
```

### Custom Progress Handler

You can implement your own progress handler for custom UI integration:

```rust
use indicatif::{ProgressBar, ProgressStyle};
use libobs_bootstrapper::status_handler::ObsBootstrapStatusHandler;
use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone)]
struct CustomProgressHandler(Arc<ProgressBar>);

impl CustomProgressHandler {
    pub fn new() -> Self {
        let bar = ProgressBar::new(200).with_style(
            ProgressStyle::default_bar()
                .template("{msg}\n{wide_bar} {pos}/{len}")
                .unwrap(),
        );
        
        bar.set_message("Initializing bootstrapper...");
        Self(Arc::new(bar))
    }
}

#[async_trait::async_trait]
impl ObsBootstrapStatusHandler for CustomProgressHandler {
    async fn handle_downloading(&mut self, prog: f32, msg: String) -> anyhow::Result<()> {
        self.0.set_message(msg);
        self.0.set_position((prog * 100.0) as u64);
        Ok(())
    }
    
    async fn handle_extraction(&mut self, prog: f32, msg: String) -> anyhow::Result<()> {
        self.0.set_message(msg);
        self.0.set_position(100 + (prog * 100.0) as u64);
        Ok(())
    }
}
```

### Setup Steps

1. You can either: <br>
   **a) RECOMMENDED** enable the `install_dummy_dll` feature for this crate <br>
   **b)** Add a placeholder `obs.dll` file to your executable directory:
     - Download a dummy DLL from [libobs-builds releases](https://github.com/sshcrack/libobs-builds/releases)
     - Use the version matching your target OBS version
     - Rename the downloaded file to `obs.dll`

2. Call `ObsBootstrapper::bootstrap()` at application startup

3. If `ObsBootstrapperResult::Restart` is returned:
   - Exit the application
   - The updater will restart your application automatically

### Advanced Options

The `ObsBootstrapperOptions` struct allows you to customize the bootstrapper:

```rust
let options = ObsBootstrapperOptions::default()
    .with_repository("sshcrack/libobs-builds")  // Custom repo
    .with_update(true)                          // Force update check
    .with_restart_after_update(true);           // Auto restart
```

## Error Handling

The crate provides the `ObsBootstrapError` enum for error handling:

- `GeneralError`: Generic bootstrapper errors
- `DownloadError`: Issues during OBS binary download
- `ExtractError`: Problems extracting downloaded files

## License

This project is licensed under the MIT License - see the LICENSE file for details.
