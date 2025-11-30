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
use crate::error::ObsBootstrapError;

pub enum DownloadStatus {
    Error(ObsBootstrapError),
    Progress(f32, String),
    Done(PathBuf),
}

pub(crate) async fn download_obs(
    repo: &str,
) -> Result<impl Stream<Item = DownloadStatus>, ObsBootstrapError> {
    // Fetch latest OBS release
    let client = reqwest::ClientBuilder::new()
        .user_agent("libobs-rs")
        .build()
        .map_err(|e| ObsBootstrapError::DownloadError("Building the reqwest client", e))?;

    let releases_url = format!("https://api.github.com/repos/{}/releases", repo);
    let releases: github_types::Root = client
        .get(&releases_url)
        .send()
        .await
        .map_err(|e| ObsBootstrapError::DownloadError("Sending Github API request", e))?
        .json()
        .await
        .map_err(|e| ObsBootstrapError::DownloadError("Converting Github API requet to JSON", e))?;

    let mut possible_versions = vec![];
    for release in releases {
        let tag = release.tag_name.replace("obs-build-", "");
        let version = Version::parse(&tag)
            .map_err(|e| ObsBootstrapError::VersionError(format!("Parsing version: {}", e)))?;

        // The minor and major version must be the same, patches shouldn't have braking changes
        if version.major == LIBOBS_API_MAJOR_VER as u64
            && version.minor == LIBOBS_API_MINOR_VER as u64
        {
            possible_versions.push(release);
        }
    }

    let latest_version = possible_versions
        .iter()
        .max_by_key(|r| &r.published_at)
        .ok_or_else(|| {
            ObsBootstrapError::InvalidFormatError(format!(
                "Finding a matching obs version for {}",
                *LIBRARY_OBS_VERSION
            ))
        })?;

    let archive_url = latest_version
        .assets
        .iter()
        .find(|a| a.name.ends_with(".7z"))
        .ok_or_else(|| ObsBootstrapError::InvalidFormatError("Finding 7z asset".to_string()))?
        .browser_download_url
        .clone();

    let hash_url = latest_version
        .assets
        .iter()
        .find(|a| a.name.ends_with(".sha256"))
        .ok_or_else(|| ObsBootstrapError::InvalidFormatError("Finding sha256 asset".to_string()))?
        .browser_download_url
        .clone();

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
            yield DownloadStatus::Progress(curr_len as  f32 / length as f32, "Downloading OBS".to_string());
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
