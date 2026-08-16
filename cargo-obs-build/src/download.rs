use std::{
    fs::File,
    io::{stdout, BufReader, Write},
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

use crate::{git::ReleaseInfo, target::ObsTarget};

const DEFAULT_REQ_TIMEOUT: u64 = 60 * 60;

pub fn download_binaries(
    build_dir: &Path,
    info: &ReleaseInfo,
    target: &ObsTarget,
) -> anyhow::Result<PathBuf> {
    let architecture = target.windows_asset_arch()?;
    let to_download = &info.assets.iter().find(|e| {
        let name = e["name"].as_str().unwrap_or("").to_lowercase();

        // OBS-Studio-30.2.1-Windows.zip
        name.contains("obs-studio")
            && (name.contains("windows") || name.contains("full"))
            && name.contains(".zip")
            && !name.contains("pdb")
            && name.contains(architecture)
    });

    if to_download.is_none() {
        bail!("No OBS Studio binaries found");
    }

    let to_download = to_download.unwrap();
    let url = to_download["browser_download_url"]
        .as_str()
        .ok_or(anyhow!("No download url found"))?;

    let download_path = build_dir.join(format!("obs-prebuilt-{}.zip", target.cache_key()));

    #[cfg(feature = "colored")]
    println!("Downloading OBS from {}", url.green());
    let hash = download_file(url, &download_path)?;

    let name = to_download["name"].as_str().unwrap_or("");
    let checksum = &info.checksums.get(&name.to_lowercase());

    if let Some(checksum) = checksum {
        if checksum.to_lowercase() != hash.to_lowercase() {
            bail!("Checksums do not match");
        } else {
            #[cfg(feature = "colored")]
            info!("{}", "Checksums match".on_green());
        }
    } else {
        error!("No checksum found for {}", name);
    }

    Ok(download_path)
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
