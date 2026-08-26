use std::{
    fs::File,
    io::{BufReader, Write, stdout},
    path::{Path, PathBuf},
    sync::mpsc::{self},
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail};
#[cfg(feature = "cli")]
use colored::Colorize;
use http_req::{
    chunked::ChunkReader,
    request::RequestMessage,
    response::Response,
    stream::{Stream, ThreadReceive, ThreadSend},
    uri::Uri,
};
#[cfg(feature = "cli")]
use indicatif::{ProgressBar, ProgressStyle};
#[cfg(feature = "cli")]
use log::{debug, info};
use log::{error, trace};
use sha2::{Digest, Sha256};

use crate::{
    git::ReleaseInfo,
    target::{ObsBuildTarget, ObsTargetArch, ObsTargetOs},
};

const DEFAULT_REQ_TIMEOUT: u64 = 60 * 60;

pub fn download_binaries(
    build_dir: &Path,
    info: &ReleaseInfo,
    target: ObsBuildTarget,
) -> anyhow::Result<PathBuf> {
    if target.os == ObsTargetOs::Linux {
        bail!(
            "Linux uses a system/source OBS installation. Run `cargo obs-build install` on \
             Debian/Ubuntu or install a compatible libobs development package for your distro."
        );
    }

    let asset = info
        .assets
        .iter()
        .find(|asset| asset_matches_target(asset, target))
        .ok_or_else(|| anyhow!("No OBS Studio binaries found for {}", target.display_name()))?;
    let url = asset["browser_download_url"]
        .as_str()
        .ok_or_else(|| anyhow!("No download url found"))?;

    let output_name = match target.os {
        ObsTargetOs::Windows => "obs-prebuilt-windows.zip",
        ObsTargetOs::Macos => "obs-prebuilt-macos.dmg",
        ObsTargetOs::Linux => unreachable!(),
    };
    let download_path = build_dir.join(output_name);

    #[cfg(feature = "colored")]
    println!("Downloading OBS from {}", url.green());
    let hash = download_file(url, &download_path)?;

    let name = asset["name"].as_str().unwrap_or("");
    let expected = info
        .checksums
        .get(&name.to_lowercase())
        .map(String::as_str)
        .or_else(|| {
            asset["digest"]
                .as_str()
                .and_then(|d| d.strip_prefix("sha256:"))
        });

    if let Some(expected) = expected {
        if expected.eq_ignore_ascii_case(&hash) {
            #[cfg(feature = "colored")]
            info!("{}", "Checksums match".on_green());
        } else {
            bail!("Checksums do not match for {name}");
        }
    } else {
        error!("No checksum found for {name}");
    }

    Ok(download_path)
}

fn asset_matches_target(asset: &serde_json::Value, target: ObsBuildTarget) -> bool {
    let name = asset["name"].as_str().unwrap_or("").to_lowercase();
    if !name.contains("obs-studio") || name.contains("dsym") || name.contains("pdb") {
        return false;
    }

    match target.os {
        ObsTargetOs::Windows => {
            let arch = match target.arch {
                ObsTargetArch::X86_64 => "x64",
                ObsTargetArch::Aarch64 => "arm64",
            };
            (name.contains("windows") || name.contains("full"))
                && name.ends_with(".zip")
                && name.contains(arch)
        }
        ObsTargetOs::Macos => {
            let arch = match target.arch {
                ObsTargetArch::X86_64 => "intel",
                ObsTargetArch::Aarch64 => "apple",
            };
            name.contains("macos") && name.ends_with(".dmg") && name.contains(arch)
        }
        ObsTargetOs::Linux => false,
    }
}

/// Returns hash
pub fn download_file(url: &str, path: &Path) -> anyhow::Result<String> {
    let timeout = Duration::from_secs(60);
    #[cfg(feature = "colored")]
    debug!("Downloading OBS binaries from {}", url.green());

    let uri = Uri::try_from(url)?;
    let mut stream = Stream::connect(&uri, Some(timeout))?;

    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    stream = Stream::try_to_https(stream, &uri, None)?;

    let res = RequestMessage::new(&uri)
        .header("Connection", "Close")
        .header("User-Agent", "cargo-obs-build")
        .parse();
    stream.write_all(&res)?;

    // Set up variables
    let (sender, receiver) = mpsc::channel();
    let (sender_supp, receiver_supp) = mpsc::channel();
    let mut raw_response_head: Vec<u8> = Vec::new();
    let mut buf_reader = BufReader::new(stream);

    // Read from the stream and send over data via `sender`.
    thread::spawn(move || {
        buf_reader.send_head(&sender);

        let params = receiver_supp.recv();
        if params.is_err() {
            return;
        }

        let params: Vec<&str> = params.unwrap();
        //TODO this never exists
        if params.contains(&"chunked") {
            let mut buf_reader = ChunkReader::from(buf_reader);
            buf_reader.send_all(&sender);
        } else {
            buf_reader.send_all(&sender);
        }
    });

    let deadline = Instant::now() + Duration::from_secs(DEFAULT_REQ_TIMEOUT);

    // Receive and process `head` of the response.
    raw_response_head.receive(&receiver, deadline)?;

    let response = Response::from_head(&raw_response_head)?;
    let content_len = response.content_len().unwrap_or(1) as u64;
    let encoding = response.headers().get("Transfer-Encoding");
    let mut params = Vec::with_capacity(4);

    if response.status_code().is_redirect() {
        let location = response.headers().get("Location");
        if location.is_none() {
            bail!("No location header found");
        }

        let location = location.unwrap();
        return download_file(location, path);
    }

    if let Some(encode) = encoding {
        if encode == "chunked" {
            params.push("chunked");
        }
    }

    sender_supp.send(params)?;

    if content_len == 0 {
        bail!("Content length is 0");
    }

    #[cfg(feature = "cli")]
    let pb = ProgressBar::new(content_len);
    #[cfg(feature = "cli")]
    {
        let style = ProgressStyle::default_bar()
            .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .map_err(|e| anyhow!("Couldn't create style {:#?}", e))?
            .progress_chars("#>-");
        pb.set_style(style);
        pb.set_message("Downloading OBS binaries".to_string());
    }

    let mut file =
        File::create(path).or(Err(anyhow!("Failed to create file '{}'", path.display())))?;
    let mut downloaded: u64 = 0;

    let mut hasher = Sha256::new();
    loop {
        let now = Instant::now();
        let remaining_time = deadline - now;

        let item = receiver.recv_timeout(remaining_time);
        if let Err(_e) = item {
            break;
        }

        let chunk = item?;

        hasher.write_all(&chunk)?;
        file.write_all(&chunk)
            .or(Err(anyhow!("Error while writing to file")))?;

        let new = std::cmp::min(downloaded + (chunk.len() as u64), content_len);
        downloaded = new;
        #[cfg(feature = "cli")]
        pb.set_position(new);
    }

    #[cfg(feature = "cli")]
    pb.finish_with_message(format!("Downloaded OBS to {}", path.display()));
    trace!("Hashing...");
    let _ = stdout().flush();
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn selects_official_macos_assets_by_architecture() {
        let apple = json!({"name": "OBS-Studio-32.1.0-macOS-Apple.dmg"});
        let intel = json!({"name": "OBS-Studio-32.1.0-macOS-Intel.dmg"});
        let dsym = json!({"name": "OBS-Studio-32.1.0-macOS-Apple-dSYMs.tar.xz"});
        let target = ObsBuildTarget::parse("macos", "aarch64").unwrap();
        assert!(asset_matches_target(&apple, target));
        assert!(!asset_matches_target(&intel, target));
        assert!(!asset_matches_target(&dsym, target));
    }

    #[test]
    fn selects_windows_assets_without_debug_symbols() {
        let binary = json!({"name": "OBS-Studio-32.1.0-Windows-x64.zip"});
        let pdb = json!({"name": "OBS-Studio-32.1.0-Windows-x64-PDBs.zip"});
        let target = ObsBuildTarget::parse("windows", "x86_64").unwrap();
        assert!(asset_matches_target(&binary, target));
        assert!(!asset_matches_target(&pdb, target));
    }
}
