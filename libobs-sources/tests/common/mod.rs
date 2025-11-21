mod initialize;

use std::{path::Path, process::Command};

use anyhow::bail;
use ffmpeg_sidecar::{ffprobe::ffprobe_path, paths::ffmpeg_path};
#[allow(unused_imports)]
pub use initialize::*;

#[cfg(target_family = "windows")]
use libobs_sources::windows::WindowCaptureSourceBuilder;
#[cfg(target_family = "windows")]
use libobs_window_helper::{WindowInfo, WindowSearchMode};

#[cfg(target_family = "windows")]
use libobs_wrapper::unsafe_send::Sendable;

#[allow(dead_code)]
fn parse_ffmpeg_duration(duration: &str) -> anyhow::Result<f64> {
    let parts: Vec<&str> = duration.split(':').collect();
    if parts.len() != 3 {
        bail!("Invalid duration format");
    }

    let hours: f64 = parts[0].parse()?;
    let minutes: f64 = parts[1].parse()?;
    let seconds: f64 = parts[2].parse()?;

    let total_seconds = hours * 3600.0 + minutes * 60.0 + seconds;
    Ok(total_seconds)
}

#[allow(dead_code)]
pub fn assert_not_black(vid_path: &Path, divider: f64) {
    ffmpeg_sidecar::download::auto_download().unwrap();

    let cmd = Command::new(ffmpeg_path())
        .arg("-i")
        .arg(vid_path)
        .arg("-vf")
        .arg("blackdetect=d=0:pic_th=0.98")
        .arg("-an")
        .arg("-f")
        .arg("null")
        .arg("-")
        .output()
        .expect("Failed to execute ffmpeg command");

    let stdout = format!(
        "{}\n{}",
        String::from_utf8_lossy(&cmd.stdout),
        String::from_utf8_lossy(&cmd.stderr)
    );
    let stdout = stdout.replace("\r", "");

    let duration = stdout
        .split("\n")
        .find(|l| l.contains("Duration"))
        .expect("Couldn't find duration")
        .trim();

    let duration = duration.split(" ").collect::<Vec<_>>()[1];
    let duration = duration.replace(",", "");
    let duration = parse_ffmpeg_duration(&duration).expect("Couldn't parse duration");

    let split = stdout.split("\n").find(|l| l.contains("black_start"));
    if split.is_none() {
        // No black frames detected
        return;
    }

    let split = split
        .unwrap()
        .trim()
        .split("]")
        .nth(1)
        .expect("Couldn't find black_start");

    println!("Split {:?}", split);
    let comps = split.split(" ").collect::<Vec<_>>();

    let black_duration = comps
        .get(2)
        .expect("Couldn't find black_duration")
        .split(":")
        .nth(1)
        .expect("Couldn't find black duration");

    let black_duration = black_duration
        .parse::<f64>()
        .expect("Couldn't parse black duration");

    let max_no_black = 0.7 / divider;

    if black_duration / duration > max_no_black {
        panic!(
            "Video is too black: black duration {}s / total duration {}s",
            black_duration, duration
        );
    }
}

/// Returns ffprobe JSON output for a given file
#[allow(dead_code)]
pub fn ffprobe_json(path: &str) -> serde_json::Value {
    ffmpeg_sidecar::download::auto_download().unwrap();

    let output = Command::new(ffprobe_path())
        .args([
            "-v",
            "error",
            "-show_streams",
            "-show_frames",
            "-of",
            "json",
            path,
        ])
        .output()
        .expect("Failed to run ffprobe");
    serde_json::from_slice(&output.stdout).expect("Failed to parse ffprobe output")
}

/// Checks that the video stream exists and duration is reasonable
#[allow(dead_code)]
pub async fn assert_valid_video(path: &str) {
    ffmpeg_sidecar::download::auto_download().unwrap();

    let json = ffprobe_json(path);
    let streams = json["streams"].as_array().expect("No streams");
    let video_stream = streams
        .iter()
        .find(|s| s["codec_type"] == "video")
        .expect("No video stream");
    let duration: f64 = video_stream["duration"]
        .as_str()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0.0);
    assert!(duration > 1.0, "Video duration too short: {}", duration);
}

/// Checks that the video has motion (frame variance above threshold)
#[allow(dead_code)]
pub async fn assert_motion(path: &str, min_variance: f64) {
    ffmpeg_sidecar::download::auto_download().unwrap();

    let output = Command::new(ffmpeg_path())
        .args(["-i", path, "-vf", "signalstats", "-f", "null", "-"])
        .output()
        .expect("Failed to run ffmpeg");
    let stdout = String::from_utf8_lossy(&output.stderr);
    let mut found = false;
    for line in stdout.lines() {
        if let Some(idx) = line.find("VAVG:") {
            let val = &line[idx + 5..].split_whitespace().next().unwrap_or("");
            if let Ok(var) = val.parse::<f64>() {
                found = true;
                assert!(var > min_variance, "Video has low motion: variance {}", var);
            }
        }
    }
    assert!(found, "No motion info found");
}

#[allow(dead_code)]
#[cfg(target_family = "windows")]
pub fn find_notepad() -> Option<Sendable<WindowInfo>> {
    let windows =
        WindowCaptureSourceBuilder::get_windows(WindowSearchMode::ExcludeMinimized).unwrap();
    println!("{:?}", windows);
    windows.into_iter().find(|w| {
        w.0.class
            .as_ref()
            .is_some_and(|e| e.to_lowercase().contains("notepad"))
    })
}
