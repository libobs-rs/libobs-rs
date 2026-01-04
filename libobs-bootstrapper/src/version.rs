use std::path::Path;

use libloading::Library;
use libobs::{LIBOBS_API_MAJOR_VER, LIBOBS_API_MINOR_VER, LIBOBS_API_PATCH_VER};

use crate::error::ObsBootstrapError;

pub type GetVersionFunc = unsafe extern "C" fn() -> u32;

pub fn get_installed_version(obs_dll: &Path) -> Result<Option<String>, ObsBootstrapError> {
    // The obs.dll should always exist
    let dll_exists = obs_dll.exists() && obs_dll.is_file();
    if !dll_exists {
        log::trace!("obs.dll does not exist at {}", obs_dll.display());
        return Ok(None);
    }

    log::trace!("Getting obs.dll version string");
    // Safety: No coroutines are used here, and the obs.dll is expected to have the obs_get_version function
    unsafe {
        let lib = Library::new(obs_dll)
            .map_err(|e| ObsBootstrapError::LibLoadingError("Opening library", e))?;
        let get_version: libloading::Symbol<GetVersionFunc> = lib
            .get(b"obs_get_version")
            .map_err(|e| ObsBootstrapError::LibLoadingError("Getting version string", e))?;
        let version = get_version();

        if version == 0 {
            lib.close()
                .map_err(|e| ObsBootstrapError::LibLoadingError("Closing lib", e))?;
            log::trace!("obs.dll does not have a version string");
            return Ok(None);
        }

        lib.close()
            .map_err(|e| ObsBootstrapError::LibLoadingError("Closing lib", e))?;

        let version_str = format!(
            "{}.{}.{}",
            (version >> 24) & 0xFF,
            (version >> 16) & 0xFF,
            version & 0xFFFF
        );

        Ok(Some(version_str))
    }
}

pub fn should_update(version_str: &str) -> Result<bool, ObsBootstrapError> {
    let version = version_str.split('.').collect::<Vec<_>>();
    if version.len() != 3 {
        return Err(ObsBootstrapError::VersionError(format!(
            "Invalid version string: {}",
            version_str
        )));
    }

    let parse_error =
        || ObsBootstrapError::VersionError(format!("Invalid version string: {}", version_str));

    let major = version[0].parse::<u64>().map_err(|_| parse_error())?;
    let minor = version[1].parse::<u64>().map_err(|_| parse_error())?;
    let patch = version[2].parse::<u64>().map_err(|_| parse_error())?;

    Ok(major != LIBOBS_API_MAJOR_VER as u64
        || minor != LIBOBS_API_MINOR_VER as u64
        || patch < LIBOBS_API_PATCH_VER as u64)
}
