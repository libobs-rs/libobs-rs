#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
// awaits and streams use unsafe internally, so I'm gonna check for unsafe blocks manually here.
#![allow(unknown_lints, require_safety_comments_on_unsafe)]

use std::{env, path::PathBuf, process};

use async_stream::stream;
use download::DownloadStatus;
use extract::ExtractStatus;
use futures_core::Stream;
use futures_util::{StreamExt, pin_mut};
use lazy_static::lazy_static;
use libobs::{LIBOBS_API_MAJOR_VER, LIBOBS_API_MINOR_VER, LIBOBS_API_PATCH_VER};
use tokio::{fs::File, io::AsyncWriteExt, process::Command};

#[cfg_attr(coverage_nightly, coverage(off))]
mod download;
mod error;
#[cfg_attr(coverage_nightly, coverage(off))]
mod extract;
#[cfg_attr(coverage_nightly, coverage(off))]
mod github_types;
mod options;
pub mod status_handler;
mod version;

#[cfg(test)]
mod options_tests;
#[cfg(test)]
mod version_tests;

pub use error::ObsBootstrapError;

pub use options::ObsBootstrapperOptions;

use crate::status_handler::{ObsBootstrapConsoleHandler, ObsBootstrapStatusHandler};

pub enum BootstrapStatus {
    /// Downloading status (first is progress from 0.0 to 1.0 and second is message)
    Downloading(f32, String),

    /// Extracting status (first is progress from 0.0 to 1.0 and second is message)
    Extracting(f32, String),
    Error(ObsBootstrapError),
    /// The application must be restarted to use the new version of OBS.
    /// This is because the obs.dll file is in use by the application and can not be replaced while running.
    /// Therefore, the "updater" is spawned to watch for the application to exit and rename the "obs_new.dll" file to "obs.dll".
    /// The updater will start the application again with the same arguments as the original application.
    RestartRequired,
}

/// A struct for bootstrapping OBS Studio.
///
/// This struct provides functionality to download, extract, and set up OBS Studio
/// for use with libobs-rs. It also handles updates to OBS when necessary.
///
/// If you want to use this bootstrapper to also install required OBS binaries at runtime,
/// do the following:
/// - Add a `obs.dll` file to your executable directory. This file will be replaced by the obs installer.
///   Recommended to use is the dll dummy (found [here](https://github.com/sshcrack/libobs-builds/releases), make sure you use the correct OBS version)
///   and rename it to `obs.dll`.
/// - Call `ObsBootstrapper::bootstrap()` at the start of your application. Options must be configured. For more documentation look at the [tauri example app](https://github.com/libobs-rs/libobs-rs/tree/main/examples/tauri-app). This will download the latest version of OBS and extract it in the executable directory.
/// - If BootstrapStatus::RestartRequired is returned, you'll need to restart your application. A updater process has been spawned to watch for the application to exit and rename the `obs_new.dll` file to `obs.dll`.
/// - Exit the application. The updater process will wait for the application to exit and rename the `obs_new.dll` file to `obs.dll` and restart your application with the same arguments as before.
///
/// [Example project](https://github.com/libobs-rs/libobs-rs/tree/main/examples/download-at-runtime)
pub struct ObsBootstrapper {}

lazy_static! {
    pub(crate) static ref LIBRARY_OBS_VERSION: String = format!(
        "{}.{}.{}",
        LIBOBS_API_MAJOR_VER, LIBOBS_API_MINOR_VER, LIBOBS_API_PATCH_VER
    );
}

pub const UPDATER_SCRIPT: &str = include_str!("./updater.ps1");

fn get_obs_dll_path() -> Result<PathBuf, ObsBootstrapError> {
    let executable =
        env::current_exe().map_err(|e| ObsBootstrapError::IoError("Getting current exe", e))?;
    let obs_dll = executable
        .parent()
        .ok_or_else(|| {
            ObsBootstrapError::IoError(
                "Failed to get parent directory",
                std::io::Error::from(std::io::ErrorKind::InvalidInput),
            )
        })?
        .join("obs.dll");

    Ok(obs_dll)
}

pub(crate) fn bootstrap(
    options: &ObsBootstrapperOptions,
) -> Result<Option<impl Stream<Item = BootstrapStatus>>, ObsBootstrapError> {
    let repo = options.repository.to_string();

    log::trace!("Checking for update...");
    let update = if options.update {
        ObsBootstrapper::is_update_available()?
    } else {
        ObsBootstrapper::is_valid_installation()?
    };

    if !update {
        log::debug!("No update needed.");
        return Ok(None);
    }

    let options = options.clone();
    Ok(Some(stream! {
        log::debug!("Downloading OBS from {}", repo);
        let download_stream = download::download_obs(&repo).await;
        if let Err(err) = download_stream {
            yield BootstrapStatus::Error(err);
            return;
        }

        let download_stream = download_stream.unwrap();
        pin_mut!(download_stream);

        let mut file = None;
        while let Some(item) = download_stream.next().await {
            match item {
                DownloadStatus::Error(err) => {
                    yield BootstrapStatus::Error(err);
                    return;
                }
                DownloadStatus::Progress(progress, message) => {
                    yield BootstrapStatus::Downloading(progress, message);
                }
                DownloadStatus::Done(path) => {
                    file = Some(path)
                }
            }
        }

        let archive_file = file.ok_or(ObsBootstrapError::InvalidState);
        if let Err(err) = archive_file {
            yield BootstrapStatus::Error(err);
            return;
        }

        log::debug!("Extracting OBS to {:?}", archive_file);
        let archive_file = archive_file.unwrap();
        let extract_stream = extract::extract_obs(&archive_file).await;
        if let Err(err) = extract_stream {
            yield BootstrapStatus::Error(err);
            return;
        }

        let extract_stream = extract_stream.unwrap();
        pin_mut!(extract_stream);

        while let Some(item) = extract_stream.next().await {
            match item {
                ExtractStatus::Error(err) => {
                    yield BootstrapStatus::Error(err);
                    return;
                }
                ExtractStatus::Progress(progress, message) => {
                    yield BootstrapStatus::Extracting(progress, message);
                }
            }
        }

        let r = spawn_updater(options).await;
        if let Err(err) = r {
            yield BootstrapStatus::Error(err);
            return;
        }

        yield BootstrapStatus::RestartRequired;
    }))
}

pub(crate) async fn spawn_updater(
    options: ObsBootstrapperOptions,
) -> Result<(), ObsBootstrapError> {
    let pid = process::id();
    let args = env::args().collect::<Vec<_>>();
    // Skip the first argument which is the executable path
    let args = args.into_iter().skip(1).collect::<Vec<_>>();

    let updater_path = env::temp_dir().join("libobs_updater.ps1");
    let mut updater_file = File::create(&updater_path)
        .await
        .map_err(|e| ObsBootstrapError::IoError("Creating updater script", e))?;

    updater_file
        .write_all(UPDATER_SCRIPT.as_bytes())
        .await
        .map_err(|e| ObsBootstrapError::IoError("Writing updater script", e))?;

    let mut command = Command::new("powershell");
    command
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-NoProfile")
        .arg("-WindowStyle")
        .arg("Hidden")
        .arg("-File")
        .arg(updater_path)
        .arg("-processPid")
        .arg(pid.to_string())
        .arg("-binary")
        .arg(
            env::current_exe()
                .map_err(|e| ObsBootstrapError::IoError("Getting current exe", e))?
                .to_string_lossy()
                .to_string(),
        );

    if options.restart_after_update {
        command.arg("-restart");
    }

    // Encode arguments as hex string (UTF-8, null-separated)
    if !args.is_empty() {
        let joined = args.join("\0");
        let bytes = joined.as_bytes();
        let hex_str = hex::encode(bytes);
        command.arg("-argumentHex");
        command.arg(hex_str);
    }

    command
        .spawn()
        .map_err(|e| ObsBootstrapError::IoError("Spawning updater process", e))?;

    Ok(())
}

pub enum ObsBootstrapperResult {
    /// No action was needed, OBS is already installed and up to date.
    None,
    /// The application must be restarted to complete the installation or update of OBS.
    Restart,
}

/// A convenience type that exposes high-level helpers to detect, update and
/// bootstrap an OBS installation.
///
/// The bootstrapper coordinates version checks and the streaming bootstrap
/// process. It does not itself perform low-level network or extraction work;
/// instead it delegates to internal modules (version checking and the
/// bootstrap stream) and surfaces a simple API for callers.
impl ObsBootstrapper {
    /// Returns true if a valid OBS installation (as determined by locating the
    /// OBS DLL and querying the installed version) is present on the system.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if an installed OBS version could be detected.
    /// - `Ok(false)` if no installed OBS version was found.
    ///
    /// # Errors
    ///
    /// Returns an `Err(ObsBootstrapError)` if there was an error locating the OBS DLL or
    /// reading the installed version information.
    pub fn is_valid_installation() -> Result<bool, ObsBootstrapError> {
        let installed = version::get_installed_version(&get_obs_dll_path()?)?;
        Ok(installed.is_some())
    }

    /// Returns true when an update to OBS should be performed.
    ///
    /// The function first checks whether OBS is installed. If no installation
    /// is found it treats that as an available update (returns `Ok(true)`).
    /// Otherwise it consults the internal version logic to determine whether
    /// the installed version should be updated.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` when an update is recommended or when OBS is not installed.
    /// - `Ok(false)` when the installed version is up-to-date.
    ///
    /// # Errors
    ///
    /// Returns an `Err(ObsBootstrapError)` if there was an error locating the OBS DLL or
    /// determining the currently installed version or update necessity.
    pub fn is_update_available() -> Result<bool, ObsBootstrapError> {
        let installed = version::get_installed_version(&get_obs_dll_path()?)?;
        if installed.is_none() {
            return Ok(true);
        }

        let installed = installed.unwrap();

        version::should_update(&installed)
    }

    /// Bootstraps OBS using the provided options and a default console status
    /// handler.
    ///
    /// This is a convenience wrapper around `bootstrap_with_handler` that
    /// supplies an `ObsBootstrapConsoleHandler` as the status consumer.
    ///
    /// # Returns
    ///
    /// - `Ok(ObsBootstrapperResult::None)` if no action was necessary.
    /// - `Ok(ObsBootstrapperResult::Restart)` if the bootstrap completed and a
    ///   restart is required.
    ///
    /// # Errors
    ///
    /// Returns `Err(ObsBootstrapError)` for any failure that prevents the
    /// bootstrap from completing (download failures, extraction failures,
    /// general errors).
    pub async fn bootstrap(
        options: &options::ObsBootstrapperOptions,
    ) -> Result<ObsBootstrapperResult, ObsBootstrapError> {
        ObsBootstrapper::bootstrap_with_handler(
            options,
            Box::new(ObsBootstrapConsoleHandler::default()),
        )
        .await
    }

    /// Bootstraps OBS using the provided options and a custom status handler.
    ///
    /// The handler will receive progress updates as the bootstrap stream emits
    /// statuses. The method drives the bootstrap stream to completion and maps
    /// stream statuses into handler calls or final results:
    ///
    /// - `BootstrapStatus::Downloading(progress, message)` → calls
    ///   `handler.handle_downloading(progress, message)`. Handler errors are
    ///   mapped to `ObsBootstrapError::DownloadError`.
    /// - `BootstrapStatus::Extracting(progress, message)` → calls
    ///   `handler.handle_extraction(progress, message)`. Handler errors are
    ///   mapped to `ObsBootstrapError::ExtractError`.
    /// - `BootstrapStatus::Error(err)` → returns `Err(ObsBootstrapError::GeneralError(_))`.
    /// - `BootstrapStatus::RestartRequired` → returns `Ok(ObsBootstrapperResult::Restart)`.
    ///
    /// If the underlying `bootstrap(options)` call returns `None` there is
    /// nothing to do and the function returns `Ok(ObsBootstrapperResult::None)`.
    ///
    /// # Parameters
    ///
    /// - `options`: configuration that controls download/extraction behavior.
    /// - `handler`: user-provided boxed trait object that receives progress
    ///   notifications; it is called on each progress update and can fail.
    ///
    /// # Returns
    ///
    /// - `Ok(ObsBootstrapperResult::None)` when no work was required or the
    ///   stream completed without requiring a restart.
    /// - `Ok(ObsBootstrapperResult::Restart)` when the bootstrap succeeded and
    ///   a restart is required.
    ///
    /// # Errors
    ///
    /// Returns `Err(ObsBootstrapError)` when:
    /// - the bootstrap pipeline could not be started,
    /// - the handler returns an error while handling a download or extraction
    ///   update (mapped respectively to `DownloadError` / `ExtractError`),
    /// - or when the bootstrap stream yields a general error.
    pub async fn bootstrap_with_handler<E: Send + Sync + 'static + std::error::Error>(
        options: &options::ObsBootstrapperOptions,
        mut handler: Box<dyn ObsBootstrapStatusHandler<Error = E>>,
    ) -> Result<ObsBootstrapperResult, ObsBootstrapError> {
        let stream = bootstrap(options)?;

        if let Some(stream) = stream {
            pin_mut!(stream);

            log::trace!("Waiting for bootstrapper to finish");
            while let Some(item) = stream.next().await {
                match item {
                    BootstrapStatus::Downloading(progress, message) => {
                        handler
                            .handle_downloading(progress, message)
                            .map_err(|e| ObsBootstrapError::Abort(Box::new(e)))?;
                    }
                    BootstrapStatus::Extracting(progress, message) => {
                        handler
                            .handle_extraction(progress, message)
                            .map_err(|e| ObsBootstrapError::Abort(Box::new(e)))?;
                    }
                    BootstrapStatus::Error(err) => {
                        return Err(err);
                    }
                    BootstrapStatus::RestartRequired => {
                        return Ok(ObsBootstrapperResult::Restart);
                    }
                }
            }
        }

        Ok(ObsBootstrapperResult::None)
    }
}
