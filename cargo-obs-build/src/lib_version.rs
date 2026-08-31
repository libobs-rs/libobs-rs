use std::path::PathBuf;

use anyhow::Context;
use cargo_metadata::{MetadataCommand, Package};
use log::{debug, info, warn};
use regex::Regex;

pub fn get_lib_obs_version() -> anyhow::Result<(u32, u32, u32)> {
    let re = Regex::new(r"^(#define LIBOBS_API_(MAJOR|MINOR|PATCH)_VER\s*)(\d+)$")
        .context("Extracting OBS version from bindings.rs")?;

    info!("Getting libobs version from bindings...");
    let meta = MetadataCommand::new()
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .exec()?;

    let pkgs = meta
        .packages
        .iter()
        .filter(|p| p.name == "libobs")
        .collect::<Vec<&Package>>();

    if pkgs.is_empty() {
        anyhow::bail!("could not find libobs package in metadata");
    }

    // Check if every package has the same version
    let mut pkg = &pkgs[0];
    if pkgs.len() > 1 {
        for p in &pkgs[1..] {
            if p.version > pkg.version {
                pkg = p;
            }
        }

        warn!(
            "multiple libobs packages found in metadata, using the one with the highest version: {}",
            pkg.version
        );
    }

    let manifest = PathBuf::from(pkg.manifest_path.clone());
    let dir = manifest
        .parent()
        .context("manifest path has no parent directory")?;

    let bindings_file = dir.join("headers").join("obs").join("obs-config.h");
    let bindings = std::fs::read_to_string(&bindings_file)
        .with_context(|| format!("failed to read bindings file: {}", bindings_file.display()))?;

    debug!("bindings file: {}", bindings_file.display());
    let version_parts = bindings
        .lines()
        .filter_map(|line| {
            // use the Option result directly and propagate missing groups via `?`
            re.captures(line).and_then(|captures| {
                let name = captures.get(2)?.as_str();
                let version = captures.get(3)?.as_str();
                Some((name, version))
            })
        })
        .collect::<Vec<_>>();

    let major_version: Option<u32> = version_parts.iter().find_map(|(name, version)| {
        if name.contains("MAJOR") {
            version.parse().ok()
        } else {
            None
        }
    });
    let minor_version: Option<u32> = version_parts.iter().find_map(|(name, version)| {
        if name.contains("MINOR") {
            version.parse().ok()
        } else {
            None
        }
    });

    let patch_version: Option<u32> = version_parts.iter().find_map(|(name, version)| {
        if name.contains("PATCH") {
            version.parse().ok()
        } else {
            None
        }
    });

    if major_version.is_none() || minor_version.is_none() || patch_version.is_none() {
        anyhow::bail!("failed to find version parts in bindings");
    }

    let obs_version = (
        major_version.unwrap(),
        minor_version.unwrap(),
        patch_version.unwrap(),
    );

    Ok(obs_version)
}
