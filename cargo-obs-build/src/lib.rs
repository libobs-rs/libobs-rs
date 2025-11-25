use git::{fetch_latest_patch_release, fetch_release, ReleaseInfo};
use lock::{acquire_lock, wait_for_lock};
use log::{debug, info, warn};
use metadata::fetch_latest_release_tag;
use std::{
    env,
    fs::{self, File},
    path::{Path, PathBuf},
};
use util::{copy_to_dir, delete_all_except};
use walkdir::WalkDir;

use lib_version::get_lib_obs_version;

use download::download_binaries;
use zip::ZipArchive;

pub use metadata::get_meta_info;

mod download;
mod git;
mod lib_version;
mod lock;
mod metadata;
mod util;

/// Check if we're running in a CI environment
fn is_ci_environment() -> bool {
    env::var("CI").is_ok()
        || env::var("GITHUB_ACTIONS").is_ok()
        || env::var("GITLAB_CI").is_ok()
        || env::var("CIRCLECI").is_ok()
        || env::var("TRAVIS").is_ok()
        || env::var("JENKINS_URL").is_ok()
        || env::var("BUILDKITE").is_ok()
}

/// Check and warn about CI environment configuration issues
fn check_ci_environment(cache_dir: &Path) {
    if !is_ci_environment() {
        return;
    }

    let mut warnings = Vec::new();

    // Check if GitHub token is set
    if env::var("GITHUB_TOKEN").is_err() {
        warnings.push(
            "GITHUB_TOKEN environment variable not set in CI. \
This may cause GitHub API rate limiting issues.",
        );
    }

    // Check if cache directory exists
    if !cache_dir.exists() {
        warnings.push(
            "OBS build cache directory does not exist. \
Consider caching this directory in your CI configuration to speed up builds. \
Ignore if this is the first run.",
        );
    }

    if !warnings.is_empty() {
        println!("cargo:warning=");
        println!("cargo:warning=⚠️  CI Environment Configuration Issues Detected:");
        for warning in warnings {
            println!("cargo:warning=  - {}", warning);
        }
        println!("cargo:warning=");
        println!("cargo:warning=For detailed setup instructions, see:");
        println!("cargo:warning=https://github.com/joshprk/libobs-rs/blob/main/cargo-obs-build/CI_SETUP.md");
        println!("cargo:warning=");
    }
}

/// Configuration options for building OBS binaries
#[derive(Debug, Clone)]
pub struct ObsBuildConfig {
    /// The directory the libobs binaries should be installed to (this is typically your `target/debug` or `target/release` directory)
    pub out_dir: PathBuf,

    /// The location where the OBS Studio binaries should be downloaded to. If this is set to None, it defaults to reading the `Cargo.toml` metadata. If no metadata is set, it defaults to `obs-build`.
    pub cache_dir: Option<PathBuf>,

    /// The GitHub repository to clone OBS Studio from, if not specified it defaults to `obsproject/obs-studio`
    pub repo_id: Option<String>,

    /// If this is specified, the specified zip file will be used instead of downloading the latest release
    /// This is useful for testing purposes, but it is not recommended to use this in production
    pub override_zip: Option<PathBuf>,

    /// When this flag is active, the cache will be cleared and a new build will be started
    pub rebuild: bool,

    /// If the browser should be included in the build
    pub browser: bool,

    /// The tag of the OBS Studio release to build.
    /// If none is specified, first the `Cargo.toml` metadata will be checked, if the version is not set it'll find the matching release for the libobs crate will be used.
    /// Use `latest` for the latest obs release.
    pub tag: Option<String>,

    /// If the compatibility check should be skipped
    pub skip_compatibility_check: bool,

    /// If set, PDBs will be deleted after extraction to save space, saving disk space.
    pub remove_pdbs: bool,
}

impl Default for ObsBuildConfig {
    fn default() -> Self {
        Self {
            out_dir: PathBuf::from("obs-out"),
            cache_dir: None,
            repo_id: None,
            override_zip: None,
            rebuild: false,
            browser: false,
            tag: None,
            skip_compatibility_check: false,
            remove_pdbs: false,
        }
    }
}

/// Simple installation method for use in build scripts.
///
/// This automatically:
/// - Determines the target directory from the OUT_DIR environment variable
/// - Uses default cache directory ("obs-build") if none is specified in metadata
/// - Auto-detects the OBS version from the libobs crate
/// - Handles all caching and locking
///
/// # Example
///
/// ```rust,no_run
/// cargo_obs_build::install().expect("Failed to install OBS binaries");
/// ```
///
/// This is equivalent to calling `build_obs_binaries()` with default configuration
/// and the out_dir set to `$OUT_DIR/../../obs-binaries`.
pub fn install() -> anyhow::Result<()> {
    use std::env;

    let out_dir = env::var("OUT_DIR")
        .map_err(|_| anyhow::anyhow!("OUT_DIR environment variable not set. This function should only be called from a build script."))?;

    let target_dir = PathBuf::from(&out_dir);
    let target_dir = target_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .ok_or_else(|| anyhow::anyhow!("Failed to determine target directory from OUT_DIR"))?;

    let config = ObsBuildConfig {
        out_dir: target_dir.to_path_buf(),
        ..Default::default()
    };

    build_obs_binaries(config)
}

/// Build and install OBS binaries according to the provided configuration
///
/// This is the main entry point for the library. It handles:
/// - Version detection from the libobs crate
/// - Downloading and extracting OBS binaries
/// - Caching to avoid re-downloads
/// - Locking to prevent concurrent builds
/// - Copying binaries to the target directory
pub fn build_obs_binaries(config: ObsBuildConfig) -> anyhow::Result<()> {
    //TODO For build scripts, we should actually check the TARGET env var instead of just erroring out on linux, but I don't think anyone will be cross-compiling

    if cfg!(target_os = "linux") {
        // The case for the "install" subcommand is handled before calling this function
        return Err(anyhow::anyhow!("Building OBS Studio from source is required on Linux. You can install binaries by running `cargo-obs-build install` separately before building your project."));
    }

    let ObsBuildConfig {
        mut cache_dir,
        repo_id,
        out_dir,
        rebuild,
        browser,
        mut tag,
        override_zip,
        skip_compatibility_check,
        remove_pdbs,
    } = config;

    // Get metadata which may update cache_dir and tag
    metadata::get_meta_info(&mut cache_dir, &mut tag)?;
    let cache_dir = cache_dir.unwrap_or_else(|| PathBuf::from("obs-build"));

    let mut obs_ver = None;
    let repo_id = repo_id.unwrap_or_else(|| "obsproject/obs-studio".to_string());
    if tag.is_none() {
        obs_ver = Some(get_lib_obs_version()?);
        let (major, minor, patch) = obs_ver.as_ref().unwrap();
        let lib_tag = format!("{}.{}.{}", major, minor, patch);

        // Check if a newer version of libobs (same major/minor, higher patch) exists in releases.
        // If found, use that tag; otherwise fall back to the crate version tag.
        match fetch_latest_patch_release(&repo_id, *major, *minor, &cache_dir) {
            Ok(Some(found_tag)) => {
                let parts: Vec<&str> = found_tag.trim_start_matches('v').split('.').collect();
                let found_patch = parts
                    .get(2)
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                if found_patch > *patch {
                    info!(
                        "Found newer libobs binaries release {} (crate: {}). Using {}",
                        found_tag, lib_tag, found_tag
                    );
                    tag = Some(found_tag);
                } else {
                    // no newer patch found -> use crate version
                    tag = Some(lib_tag);
                }
            }
            Ok(None) => {
                // none found -> use crate version
                tag = Some(lib_tag);
            }
            Err(e) => {
                // On error, log debug and fall back to crate version
                warn!("Failed to check for newer compatible libobs release: {}", e);
                tag = Some(lib_tag);
            }
        }
    }

    let tag = tag.unwrap();
    let target_out_dir = PathBuf::new().join(&out_dir);

    // Check CI environment configuration AFTER we have the final cache_dir
    check_ci_environment(&cache_dir);

    let tag = if tag.trim() == "latest" {
        fetch_latest_release_tag(&repo_id, &cache_dir)?
    } else {
        tag
    };

    if !skip_compatibility_check {
        let (major, minor, patch) = if let Some(v) = obs_ver {
            v
        } else {
            get_lib_obs_version()?
        };

        info!(
            "Detected libobs crate version: {}.{}.{}",
            major, minor, patch
        );
        let tag_parts: Vec<&str> = tag.trim_start_matches('v').split('.').collect();
        let tag_parts = tag_parts
            .iter()
            .map(|e| e.parse::<u32>().unwrap_or(0))
            .collect::<Vec<u32>>();

        if tag_parts.len() < 3 {
            info!("Warning: Could not determine libobs compatibility, tag does not have 3 parts");
        } else {
            let (tag_major, tag_minor, tag_patch) = (tag_parts[0], tag_parts[1], tag_parts[2]);
            if major != tag_major || minor != tag_minor {
                use log::warn;

                warn!(
                    "libobs (crate) version {}.{}.{} may not be compatible with libobs (binaries) {}.{}.{}",
                    major, minor, patch, tag_major, tag_minor, tag_patch
                );
                warn!(
                    "Set the `libobs-version` in `[workspace.metadata]` to {}.{}.{} to avoid runtime issues",
                    major, minor, patch
                );
            } else {
                info!(
                    "libobs (crate) version {}.{}.{} should be compatible with libobs (binaries) {}.{}.{}",
                    major, minor, patch, tag_major, tag_minor, tag_patch
                );
            }
        }
    }

    let repo_dir = cache_dir.join(&tag);
    let repo_exists = repo_dir.is_dir();

    if !repo_exists {
        fs::create_dir_all(&repo_dir)?;
    }

    let build_out = repo_dir.join("build_out");
    let lock_file = cache_dir.join(format!("{}.lock", tag));
    let success_file = repo_dir.join(".success");

    wait_for_lock(&lock_file)?;

    if !success_file.is_file() || rebuild {
        let lock = acquire_lock(&lock_file)?;
        if repo_exists || rebuild {
            debug!("Cleaning up old build...");
            delete_all_except(&repo_dir, None)?;
        }

        debug!("Fetching {} version of OBS Studio...", tag);

        let release = fetch_release(&repo_id, &Some(tag.clone()), &cache_dir)?;
        build_obs(release, &build_out, browser, remove_pdbs, override_zip)?;

        File::create(&success_file)?;
        drop(lock);
    }

    info!(
        "Copying files from {} to {}",
        build_out.display(),
        target_out_dir.display()
    );
    copy_to_dir(&build_out, &target_out_dir, None)?;

    info!("Done!");

    Ok(())
}

fn build_obs(
    release: ReleaseInfo,
    build_out: &Path,
    include_browser: bool,
    remove_pdbs: bool,
    override_zip: Option<PathBuf>,
) -> anyhow::Result<()> {
    fs::create_dir_all(build_out)?;

    let obs_path = if let Some(e) = override_zip {
        e
    } else {
        download_binaries(build_out, &release)?
    };

    let obs_archive = File::open(&obs_path)?;
    let mut archive = ZipArchive::new(&obs_archive)?;

    info!("Extracting OBS Studio binaries...");
    archive.extract(build_out)?;
    let bin_path = build_out.join("bin").join("64bit");
    copy_to_dir(&bin_path, build_out, None)?;
    fs::remove_dir_all(build_out.join("bin"))?;

    clean_up_files(build_out, remove_pdbs, include_browser)?;

    fs::remove_file(&obs_path)?;

    Ok(())
}

fn clean_up_files(
    build_out: &Path,
    remove_pdbs: bool,
    include_browser: bool,
) -> anyhow::Result<()> {
    let mut to_exclude = vec![
        "obs64",
        "frontend",
        "obs-webrtc",
        "obs-websocket",
        "decklink",
        "obs-scripting",
        "qt6",
        "qminimal",
        "qwindows",
        "imageformats",
        "obs-studio",
        "aja-output-ui",
        "obs-vst",
    ];

    if remove_pdbs {
        to_exclude.push(".pdb");
    }

    if !include_browser {
        to_exclude.append(&mut vec![
            "obs-browser",
            "obs-browser-page",
            "chrome_",
            "resources",
            "cef",
            "snapshot",
            "locales",
        ]);
    }

    info!("Cleaning up unnecessary files...");
    for entry in WalkDir::new(build_out).into_iter().flatten() {
        let path = entry.path();
        if to_exclude.iter().any(|e| {
            path.file_name().is_some_and(|x| {
                let x_l = x.to_string_lossy().to_lowercase();
                x_l.contains(e) || x_l == *e
            })
        }) {
            debug!("Deleting: {}", path.display());
            if path.is_dir() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
        }
    }

    Ok(())
}
