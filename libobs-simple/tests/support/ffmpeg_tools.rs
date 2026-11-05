//! FFmpeg/FFprobe commands for recording verification.
//!
//! Uses an `ffmpeg`/`ffprobe` already on `PATH` (as installed in CI) and otherwise
//! lets `ffmpeg-sidecar` download a static build, so the tests also work on
//! machines without a system FFmpeg (e.g. a plain Windows runner).
//!
//! Include with `#[path = "support/ffmpeg_tools.rs"] mod ffmpeg_tools;`.

#![allow(dead_code)]

use std::process::Command;

fn ensure_available() {
    ffmpeg_sidecar::download::auto_download()
        .expect("FFmpeg should be available (on PATH or downloadable) for recording verification");
}

/// A `Command` running FFmpeg.
pub fn ffmpeg() -> Command {
    ensure_available();
    Command::new(ffmpeg_sidecar::paths::ffmpeg_path())
}

/// A `Command` running FFprobe.
pub fn ffprobe() -> Command {
    ensure_available();
    Command::new(ffmpeg_sidecar::ffprobe::ffprobe_path())
}
