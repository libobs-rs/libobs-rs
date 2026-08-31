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
    let tag_param = if let Some(tag_inner) = tag_str {
        &format!("tags/{}", tag_inner)
    } else {
        "latest"
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
pub fn fetch_latest_compatible_release(
    _repo_id: &str,
    major: u32,
    minor: Option<u32>,
    _cache_dir: &Path,
) -> anyhow::Result<Option<String>> {
    println!("cargo:warning=-- WARNING --");
    println!("cargo:warning=Using mock GitHub responses! This is only for testing purposes.");
    println!("cargo:warning=-- WARNING --");
    let body = include_str!("../mock_responses/obs_studio_release.json");
    let arr: Vec<Value> = serde_json::from_str(&body)?;
    Ok(parse_releases_for_latest_compatible(&arr, major, minor))
}

#[cfg(not(feature = "__mock_github_responses"))]
pub fn fetch_latest_compatible_release(
    repo_id: &str,
    major: u32,
    minor: Option<u32>,
    cache_dir: &Path,
) -> anyhow::Result<Option<String>> {
    let version_key = minor
        .map(|minor| format!("{major}.{minor}"))
        .unwrap_or_else(|| major.to_string());
    let cache_key = format!("{}-releases-{}", repo_id.replace('/', "_"), version_key);
    let cache_dir = cache_dir.join(".api-cache");
    let cache_path = cache_dir.join(format!("{cache_key}.json"));

    if cache_path.exists() {
        if let Ok(content) = fs::read_to_string(&cache_path) {
            if let Ok(arr) = serde_json::from_str::<Vec<Value>>(&content) {
                log::debug!("Using cached releases list for {version_key}");
                return Ok(parse_releases_for_latest_compatible(&arr, major, minor));
            }
        }
    }

    let url = format!("https://api.github.com/repos/{repo_id}/releases");
    let url = Uri::try_from(url.as_str())?;

    let mut body = Vec::new();
    let mut req = Request::new(&url);
    req.header("User-Agent", "cargo-obs-build");
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req.header("Authorization", &format!("Bearer {token}"));
    }

    let res = req.send(&mut body)?;
    if res.status_code() != StatusCode::new(200) {
        bail!(
            "Failed to fetch releases: {} with {}",
            res.status_code(),
            String::from_utf8(body).unwrap_or_else(|_| "Couldn't parse".to_string())
        );
    }

    let body = String::from_utf8(body)?;
    let _ = save_cached_release(&cache_path, &body);
    let arr: Vec<Value> = serde_json::from_str(&body)?;
    Ok(parse_releases_for_latest_compatible(&arr, major, minor))
}

pub fn fetch_latest_patch_release(
    repo_id: &str,
    major: u32,
    minor: u32,
    cache_dir: &Path,
) -> anyhow::Result<Option<String>> {
    fetch_latest_compatible_release(repo_id, major, Some(minor), cache_dir)
}

fn parse_releases_for_latest_compatible(
    arr: &[Value],
    major: u32,
    minor: Option<u32>,
) -> Option<String> {
    let mut best: Option<((u32, u32), String)> = None;

    for rel in arr {
        if rel["draft"].as_bool().unwrap_or(false) || rel["prerelease"].as_bool().unwrap_or(false) {
            continue;
        }

        let Some(tag_name) = rel["tag_name"].as_str() else {
            continue;
        };
        let mut parts = tag_name.trim_start_matches('v').split('.');
        let (Some(r_major), Some(r_minor), Some(r_patch), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let (Ok(r_major), Ok(r_minor), Ok(r_patch)) = (
            r_major.parse::<u32>(),
            r_minor.parse::<u32>(),
            r_patch.parse::<u32>(),
        ) else {
            continue;
        };

        if r_major != major || minor.is_some_and(|wanted| r_minor != wanted) {
            continue;
        }

        let version = (r_minor, r_patch);
        if best.as_ref().is_none_or(|(current, _)| version > *current) {
            best = Some((version, tag_name.to_string()));
        }
    }

    best.map(|(_, tag)| tag)
}

#[cfg(test)]
mod tests {
    use super::parse_releases_for_latest_compatible;
    use serde_json::json;

    #[test]
    fn resolves_latest_release_within_major() {
        let releases = vec![
            json!({"tag_name":"32.1.2","draft":false,"prerelease":false}),
            json!({"tag_name":"32.3.0","draft":false,"prerelease":false}),
            json!({"tag_name":"33.0.0","draft":false,"prerelease":false}),
        ];
        assert_eq!(
            parse_releases_for_latest_compatible(&releases, 32, None).as_deref(),
            Some("32.3.0")
        );
    }

    #[test]
    fn resolves_latest_patch_within_minor() {
        let releases = vec![
            json!({"tag_name":"32.1.2","draft":false,"prerelease":false}),
            json!({"tag_name":"32.1.7","draft":false,"prerelease":false}),
            json!({"tag_name":"32.2.0","draft":false,"prerelease":false}),
            json!({"tag_name":"32.1.9","draft":true,"prerelease":false}),
        ];
        assert_eq!(
            parse_releases_for_latest_compatible(&releases, 32, Some(1)).as_deref(),
            Some("32.1.7")
        );
    }
}
