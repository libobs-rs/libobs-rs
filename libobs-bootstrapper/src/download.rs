use std::{env::temp_dir, path::PathBuf};

use async_stream::stream;
use futures_core::Stream;
use futures_util::StreamExt;
use libobs::{LIBOBS_API_MAJOR_VER, LIBOBS_API_MINOR_VER};
use semver::Version;
use sha2::{Digest, Sha256};
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;

use super::{LIBRARY_OBS_VERSION, github_types};
use crate::{error::ObsBootstrapError, options::UpdateTargetMode};

#[derive(Debug, Clone)]
pub(crate) struct ResolvedRelease {
    pub(crate) version: Version,
    pub(crate) archive_url: String,
    pub(crate) hash_url: String,
}

pub enum DownloadStatus {
    Error(ObsBootstrapError),
    Progress(f32, String),
    Done(PathBuf),
}

fn parse_release_version(release: &github_types::Root2) -> Result<Version, ObsBootstrapError> {
    let tag = release.tag_name.replace("obs-build-", "");
    Version::parse(&tag)
        .map_err(|e| ObsBootstrapError::VersionError(format!("Parsing version: {}", e)))
}

fn is_release_compatible(version: &Version, mode: UpdateTargetMode) -> bool {
    if version.major != LIBOBS_API_MAJOR_VER as u64 {
        return false;
    }

    match mode {
        UpdateTargetMode::LatestCompatibleSameMajor => true,
        UpdateTargetMode::LatestCompatibleSameMajorMinor => {
            version.minor == LIBOBS_API_MINOR_VER as u64
        }
    }
}

pub(crate) fn select_latest_compatible_release(
    releases: &[github_types::Root2],
    mode: UpdateTargetMode,
) -> Result<(&github_types::Root2, Version), ObsBootstrapError> {
    let mut latest: Option<(&github_types::Root2, Version)> = None;

    for release in releases {
        if release.draft || release.prerelease {
            continue;
        }

        let version = parse_release_version(release)?;
        if !is_release_compatible(&version, mode) {
            continue;
        }

        if let Some((_, latest_version)) = &latest {
            if version > *latest_version {
                latest = Some((release, version));
            }
        } else {
            latest = Some((release, version));
        }
    }

    latest.ok_or_else(|| {
        ObsBootstrapError::InvalidFormatError(format!(
            "Finding a matching obs version for {}",
            *LIBRARY_OBS_VERSION
        ))
    })
}

fn release_to_resolved(
    release: &github_types::Root2,
    version: Version,
) -> Result<ResolvedRelease, ObsBootstrapError> {
    let archive_url = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".7z"))
        .ok_or_else(|| ObsBootstrapError::InvalidFormatError("Finding 7z asset".to_string()))?
        .browser_download_url
        .clone();

    let hash_url = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".sha256"))
        .ok_or_else(|| ObsBootstrapError::InvalidFormatError("Finding sha256 asset".to_string()))?
        .browser_download_url
        .clone();

    Ok(ResolvedRelease {
        version,
        archive_url,
        hash_url,
    })
}

pub(crate) async fn resolve_latest_compatible_release(
    repo: &str,
    mode: UpdateTargetMode,
) -> Result<ResolvedRelease, ObsBootstrapError> {
    #[cfg(feature = "__mock_github_responses")]
    {
        let _ = repo;
        println!("-- WARNING --");
        println!("Using mock GitHub responses! This is only for testing purposes.");
        println!("-- WARNING --");
        let releases: github_types::Root =
            serde_json::from_str(include_str!("../mock_responses/libobs_builds_release.json"))
                .expect("Parsing mock response");
        let (release, version) = select_latest_compatible_release(&releases, mode)?;
        return release_to_resolved(release, version);
    }

    #[cfg(not(feature = "__mock_github_responses"))]
    {
        let client = reqwest::ClientBuilder::new()
            .user_agent("libobs-rs")
            .build()
            .map_err(|e| ObsBootstrapError::DownloadError("Building the reqwest client", e))?;

        let latest_release_url = format!("https://api.github.com/repos/{}/releases/latest", repo);
        let latest_release: github_types::Root2 = client
            .get(&latest_release_url)
            .send()
            .await
            .map_err(|e| ObsBootstrapError::DownloadError("Sending Github API request", e))?
            .json()
            .await
            .map_err(|e| {
                ObsBootstrapError::DownloadError("Converting Github API request to JSON", e)
            })?;

        if !latest_release.draft && !latest_release.prerelease {
            let latest_version = parse_release_version(&latest_release)?;
            if is_release_compatible(&latest_version, mode) {
                return release_to_resolved(&latest_release, latest_version);
            }
        }

        let releases_url = format!("https://api.github.com/repos/{}/releases", repo);
        let releases: github_types::Root = client
            .get(&releases_url)
            .send()
            .await
            .map_err(|e| ObsBootstrapError::DownloadError("Sending Github API request", e))?
            .json()
            .await
            .map_err(|e| {
                ObsBootstrapError::DownloadError("Converting Github API request to JSON", e)
            })?;

        let (release, version) = select_latest_compatible_release(&releases, mode)?;
        release_to_resolved(release, version)
    }
}

pub(crate) async fn download_obs(
    resolved_release: &ResolvedRelease,
) -> Result<impl Stream<Item = DownloadStatus>, ObsBootstrapError> {
    let archive_url = resolved_release.archive_url.clone();
    let hash_url = resolved_release.hash_url.clone();

    let client = reqwest::ClientBuilder::new()
        .user_agent("libobs-rs")
        .build()
        .map_err(|e| ObsBootstrapError::DownloadError("Building the reqwest client", e))?;

    let res = client
        .get(archive_url)
        .send()
        .await
        .map_err(|e| ObsBootstrapError::DownloadError("Sending archive request", e))?;
    let length = res.content_length().unwrap_or(0);

    let mut bytes_stream = res.bytes_stream();

    let path = PathBuf::new()
        .join(temp_dir())
        .join(format!("{}.7z", Uuid::new_v4()));
    let mut tmp_file = File::create_new(&path)
        .await
        .map_err(|e| ObsBootstrapError::IoError("Creating temporary file", e))?;

    let mut curr_len = 0;
    let mut hasher = Sha256::new();
    Ok(stream! {
        yield DownloadStatus::Progress(0.0, "Downloading OBS".to_string());
        while let Some(chunk) = bytes_stream.next().await {
            let chunk = chunk.map_err(|e| ObsBootstrapError::DownloadError("Receiving chunk of archive data", e));
            if let Err(e) = chunk {
                yield DownloadStatus::Error(e);
                return;
            }

            let chunk = chunk.unwrap();
            hasher.update(&chunk);
            let r = tmp_file.write_all(&chunk).await.map_err(|e| ObsBootstrapError::IoError("Writing to temporary file", e));
            if let Err(e) = r {
                yield DownloadStatus::Error(e);
                return;
            }

            curr_len = std::cmp::min(curr_len + chunk.len() as u64, length);
            let progress = if length == 0 {
                0.0
            } else {
                curr_len as f32 / length as f32
            };

            yield DownloadStatus::Progress(progress, "Downloading OBS".to_string());
        }

        // Getting remote hash
        let remote_hash = client.get(hash_url).send().await.map_err(|e| ObsBootstrapError::DownloadError("Fetching hash", e));
        if let Err(e) = remote_hash {
            yield DownloadStatus::Error(e);
            return;
        }

        let remote_hash = remote_hash.unwrap().text().await.map_err(|e| ObsBootstrapError::DownloadError("Reading hash", e));
        if let Err(e) = remote_hash {
            yield DownloadStatus::Error(e);
            return;
        }

        let remote_hash = remote_hash.unwrap();
        let remote_hash = hex::decode(remote_hash.trim()).map_err(|e| ObsBootstrapError::InvalidFormatError(e.to_string()));
        if let Err(e) = remote_hash {
            yield DownloadStatus::Error(e);
            return;
        }

        let remote_hash = remote_hash.unwrap();

        // Calculating local hash
        let local_hash = hasher.finalize();
        if local_hash.to_vec() != remote_hash {
            yield DownloadStatus::Error(ObsBootstrapError::HashMismatchError);
            return;
        }

        log::info!("Hashes match");
        yield DownloadStatus::Done(path);
    })
}
