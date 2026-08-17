use std::path::PathBuf;

use anyhow::Context;
use cargo_metadata::{MetadataCommand, Package};
use log::{info, warn};

pub fn get_lib_obs_version() -> anyhow::Result<(u32, u32, u32)> {
    info!("Getting canonical libobs version...");
    let meta = MetadataCommand::new().exec()?;
    let pkgs = meta
        .packages
        .iter()
        .filter(|p| p.name == "libobs")
        .collect::<Vec<&Package>>();

    if pkgs.is_empty() {
        anyhow::bail!("could not find libobs package in metadata");
    }

    let mut pkg = pkgs[0];
    if pkgs.len() > 1 {
        for candidate in &pkgs[1..] {
            if candidate.version > pkg.version {
                pkg = candidate;
            }
        }
        warn!(
            "multiple libobs packages found in metadata, using the highest version: {}",
            pkg.version
        );
    }

    let manifest = PathBuf::from(pkg.manifest_path.clone());
    let dir = manifest
        .parent()
        .context("manifest path has no parent directory")?;
    let version_file = dir.join("OBS_VERSION");
    let version = std::fs::read_to_string(&version_file)
        .with_context(|| format!("failed to read {}", version_file.display()))?;
    let parts = version
        .trim()
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .context("OBS_VERSION must contain a semantic x.y.z version")?;
    if parts.len() != 3 {
        anyhow::bail!("OBS_VERSION must contain exactly three numeric components");
    }
    Ok((parts[0], parts[1], parts[2]))
}
