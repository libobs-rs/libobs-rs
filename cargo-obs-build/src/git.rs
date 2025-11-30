use anyhow::{anyhow, bail};
#[cfg(not(feature = "__mock_github_responses"))]
use http_req::{request::Request, response::StatusCode, uri::Uri};
use serde_json::Value;
use std::collections::HashMap;
#[cfg(not(feature = "__mock_github_responses"))]
use std::fs;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct ReleaseInfo {
    pub tag: String,
    #[allow(dead_code)]
    pub assets: Vec<Value>,
    #[allow(dead_code)]
    pub checksums: HashMap<String, String>,
}

/// Try to load cached release info from disk
#[cfg(not(feature = "__mock_github_responses"))]
fn load_cached_release(cache_path: &Path) -> Option<ReleaseInfo> {
    if !cache_path.exists() {
        return None;
    }

    if let Ok(metadata) = fs::metadata(cache_path) {
        if metadata.modified().ok()?.elapsed().ok()?.as_secs() > 86400 {
            // Cache is older than 1 day
            return None;
        }
    };

    let content = fs::read_to_string(cache_path).ok()?;
    let data: Value = serde_json::from_str(&content).ok()?;

    let tag = data["tag_name"].as_str()?.to_string();
    let assets = data["assets"].as_array()?.clone();

    let mut checksums = HashMap::new();
    let note = data["body"].as_str().unwrap_or("");
    let split = note.replace("\r", "");
    let split = split.split("\n");

    let mut is_checksums = false;
    for line in split {
        if line.to_lowercase().contains("checksums") {
            is_checksums = true;
            continue;
        }

        if !is_checksums {
            continue;
        }

        let split: Vec<&str> = line.trim().split(":").collect();
        if split.len() != 2 {
            continue;
        }

        checksums.insert(
            split[0].trim().to_lowercase().to_string(),
            split[1].trim().to_string(),
        );
    }

    Some(ReleaseInfo {
        tag,
        assets,
        checksums,
    })
}

/// Save release info to cache
#[cfg(not(feature = "__mock_github_responses"))]
fn save_cached_release(cache_path: &Path, data: &str) -> anyhow::Result<()> {
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(cache_path, data)?;
    Ok(())
}

#[cfg(feature = "__mock_github_responses")]
pub fn fetch_release(
    _repo_id: &str,
    _tag: &Option<String>,
    _cache_dir: &Path,
) -> anyhow::Result<ReleaseInfo> {
    println!("cargo:warning=-- WARNING --");
    println!("cargo:warning=Using mock GitHub responses! This is only for testing purposes.");
    println!("cargo:warning=-- WARNING --");
    let body = include_str!("../mock_responses/obs_studio_release_latest.json");
    let body: Value = serde_json::from_str(&body)?;
    parse_release_info(&body)
}

#[cfg(not(feature = "__mock_github_responses"))]
pub fn fetch_release(
    repo_id: &str,
    tag: &Option<String>,
    cache_dir: &Path,
) -> anyhow::Result<ReleaseInfo> {
    let tag_str = tag.clone();
    let tag_param = if tag_str.is_none() {
        "latest"
    } else {
        &format!("tags/{}", tag_str.unwrap())
    };

    // Create cache key based on repo and tag
    let cache_key = format!(
        "{}-{}",
        repo_id.replace('/', "_"),
        tag_param.replace('/', "_")
    );
    let cache_dir = cache_dir.join(".api-cache");
    let cache_path = cache_dir.join(format!("{}.json", cache_key));

    // Try to load from cache first
    if let Some(cached) = load_cached_release(&cache_path) {
        log::debug!("Using cached release info for {}", tag_param);
        return Ok(cached);
    }

    let url = format!(
        "https://api.github.com/repos/{}/releases/{}",
        repo_id, tag_param
    );
    let url = Uri::try_from(url.as_str())?;

    let mut body = Vec::new(); //Container for body of a response.
    let mut req = Request::new(&url);
    req.header("User-Agent", "cargo-obs-build");

    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req.header("Authorization", &format!("Bearer {}", token));
    }

    let res = req.send(&mut body)?;
    if res.status_code() != StatusCode::new(200) {
        bail!(
            "Failed to fetch latest release: {} with {}",
            res.status_code(),
            String::from_utf8(body).unwrap_or("Couldn't parse".to_string())
        );
    }

    let body = String::from_utf8(body)?;

    // Save to cache for future use
    let _ = save_cached_release(&cache_path, &body);

    let body: Value = serde_json::from_str(&body)?;
    parse_release_info(&body)
}

pub fn parse_release_info(body: &Value) -> anyhow::Result<ReleaseInfo> {
    let tag_name = body["tag_name"].as_str();

    if tag_name.is_none() {
        bail!("Tag name in release is none");
    }

    let tag = tag_name.unwrap();
    let assets = body["assets"]
        .as_array()
        .ok_or(anyhow!("No assets found"))?;

    let mut checksums = HashMap::new();
    let note = body["body"].as_str().unwrap_or("");

    let split = note.replace("\r", "");
    let split = split.split("\n");

    let mut is_checksums = false;
    for line in split {
        if line.to_lowercase().contains("checksums") {
            is_checksums = true;
            continue;
        }

        if !is_checksums {
            continue;
        }

        let split: Vec<&str> = line.trim().split(":").collect();
        if split.len() != 2 {
            continue;
        }

        checksums.insert(
            split[0].trim().to_lowercase().to_string(),
            split[1].trim().to_string(),
        );
    }

    Ok(ReleaseInfo {
        tag: tag.to_string(),
        assets: assets.clone(),
        checksums,
    })
}

#[cfg(feature = "__mock_github_responses")]
pub fn fetch_latest_patch_release(
    _repo_id: &str,
    major: u32,
    minor: u32,
    _cache_dir: &Path,
) -> anyhow::Result<Option<String>> {
    println!("cargo:warning=-- WARNING --");
    println!("cargo:warning=Using mock GitHub responses! This is only for testing purposes.");
    println!("cargo:warning=-- WARNING --");
    let body = include_str!("../mock_responses/obs_studio_release.json");
    let arr: Vec<Value> = serde_json::from_str(&body)?;
    parse_releases_for_latest_patch(&arr, major, minor)
}

#[cfg(not(feature = "__mock_github_responses"))]
pub fn fetch_latest_patch_release(
    repo_id: &str,
    major: u32,
    minor: u32,
    cache_dir: &Path,
) -> anyhow::Result<Option<String>> {
    // Create cache key based on repo and version
    let cache_key = format!("{}-releases-{}.{}", repo_id.replace('/', "_"), major, minor);
    let cache_dir = cache_dir.join(".api-cache");
    let cache_path = cache_dir.join(format!("{}.json", cache_key));

    // Try to load from cache first
    if cache_path.exists() {
        if let Ok(content) = fs::read_to_string(&cache_path) {
            if let Ok(arr) = serde_json::from_str::<Vec<Value>>(&content) {
                log::debug!("Using cached releases list for {}.{}", major, minor);
                return parse_releases_for_latest_patch(&arr, major, minor);
            }
        }
    }

    let url = format!("https://api.github.com/repos/{}/releases", repo_id);
    let url = Uri::try_from(url.as_str())?;

    let mut body = Vec::new();
    let mut req = Request::new(&url);

    req.header("User-Agent", "cargo-obs-build");
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req.header("Authorization", &format!("Bearer {}", token));
    }

    let res = req.send(&mut body)?;

    if res.status_code() != StatusCode::new(200) {
        bail!(
            "Failed to fetch releases: {} with {}",
            res.status_code(),
            String::from_utf8(body).unwrap_or("Couldn't parse".to_string())
        );
    }

    let body = String::from_utf8(body)?;

    // Save to cache for future use
    let _ = save_cached_release(&cache_path, &body);

    let arr: Vec<Value> = serde_json::from_str(&body)?;
    parse_releases_for_latest_patch(&arr, major, minor)
}

fn parse_releases_for_latest_patch(
    arr: &[Value],
    major: u32,
    minor: u32,
) -> anyhow::Result<Option<String>> {
    let mut best_patch: Option<u32> = None;
    let mut best_tag: Option<String> = None;

    for rel in arr.iter() {
        // skip drafts and prereleases
        if rel["draft"].as_bool().unwrap_or(false) || rel["prerelease"].as_bool().unwrap_or(false) {
            continue;
        }
        let tag_name = rel["tag_name"].as_str().unwrap_or("").to_string();
        if tag_name.is_empty() {
            continue;
        }
        let parts: Vec<&str> = tag_name.trim_start_matches('v').split('.').collect();
        if parts.len() < 3 {
            continue;
        }
        let r_major = parts[0].parse::<u32>().unwrap_or(0);
        let r_minor = parts[1].parse::<u32>().unwrap_or(0);
        let r_patch = parts[2].parse::<u32>().unwrap_or(0);

        if r_major == major
            && r_minor == minor
            && (best_patch.is_none() || r_patch > best_patch.unwrap())
        {
            best_patch = Some(r_patch);
            best_tag = Some(tag_name);
        }
    }

    Ok(best_tag)
}
