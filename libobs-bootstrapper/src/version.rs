use std::path::Path;

use libloading::Library;
use semver::Version;

use crate::error::ObsBootstrapError;

pub type GetVersionFunc = unsafe extern "C" fn() -> u32;

pub fn parse_version(version_str: &str) -> Result<Version, ObsBootstrapError> {
    let parse_error =
        || ObsBootstrapError::VersionError(format!("Invalid version string: {version_str}"));

    let version = Version::parse(version_str).map_err(|_| parse_error())?;
    if !version.pre.is_empty() || !version.build.is_empty() {
        return Err(parse_error());
    }

    Ok(version)
}

pub fn get_installed_version(obs_library: &Path) -> Result<Option<String>, ObsBootstrapError> {
    if !obs_library.is_file() {
        log::trace!("OBS library does not exist at {}", obs_library.display());
        return Ok(None);
    }

    log::trace!("Reading OBS version from {}", obs_library.display());
    // Safety: the caller selected this OBS runtime for inspection. We only
    // resolve/call the stable `obs_get_version` entry point and close the
    // temporary library handle before returning.
    unsafe {
        let lib = Library::new(obs_library)
            .map_err(|e| ObsBootstrapError::LibLoadingError("Opening library", e))?;
        let get_version: libloading::Symbol<GetVersionFunc> = lib
            .get(b"obs_get_version")
            .map_err(|e| ObsBootstrapError::LibLoadingError("Getting version", e))?;
        let version = get_version();
        if version == 0 {
            lib.close()
                .map_err(|e| ObsBootstrapError::LibLoadingError("Closing library", e))?;
            return Ok(None);
        }

        lib.close()
            .map_err(|e| ObsBootstrapError::LibLoadingError("Closing library", e))?;
        Ok(Some(format!(
            "{}.{}.{}",
            (version >> 24) & 0xFF,
            (version >> 16) & 0xFF,
            version & 0xFFFF
        )))
    }
}

pub fn should_update(
    installed_version_str: &str,
    target_version: &Version,
) -> Result<bool, ObsBootstrapError> {
    let installed_version = parse_version(installed_version_str)?;
    if installed_version.major != target_version.major {
        return Ok(true);
    }
    Ok(installed_version < *target_version)
}
