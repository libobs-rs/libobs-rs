use std::path::Path;

use libloading::Library;
use libobs::LIBOBS_API_MAJOR_VER;
use semver::Version;

use crate::error::ObsBootstrapError;

pub type GetVersionFunc = unsafe extern "C" fn() -> u32;

pub fn parse_version(version_str: &str) -> Result<Version, ObsBootstrapError> {
    let parse_error =
        || ObsBootstrapError::VersionError(format!("Invalid version string: {}", version_str));

    let version = Version::parse(version_str).map_err(|_| parse_error())?;
    if !version.pre.is_empty() || !version.build.is_empty() {
        return Err(parse_error());
    }

    Ok(version)
}

pub fn is_compatible_major(version_str: &str) -> Result<bool, ObsBootstrapError> {
    let version = parse_version(version_str)?;
    Ok(version.major == LIBOBS_API_MAJOR_VER as u64)
}

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

        log::trace!("obs.dll version string: {}", version_str);
        Ok(Some(version_str))
    }
}

pub fn should_update(
    installed_version_str: &str,
    target_version: &Version,
) -> Result<bool, ObsBootstrapError> {
    let installed_version = parse_version(installed_version_str)?;
    if installed_version.major != LIBOBS_API_MAJOR_VER as u64 {
        return Ok(false);
    }

    Ok(installed_version < *target_version)
}
