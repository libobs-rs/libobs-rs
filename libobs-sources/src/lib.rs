//! A library for creating OBS sources without having to figure out what properties are used by sources.
//!
//! This crate provides convenient builders for OBS sources across different platforms:
//! - **Windows**: Window capture, monitor capture, game capture
//! - **Linux**: X11 screen capture, XComposite window capture, V4L2 camera, ALSA/PulseAudio/JACK audio, PipeWire
//!
//! You can find examples [here](https://github.com/libobs-rs/libobs-rs/tree/main/examples).

#[cfg(target_family = "windows")]
pub mod windows;

pub use libobs_wrapper as wrapper;

#[cfg(target_os = "linux")]
pub mod linux;

pub mod encoders;
mod macro_helper;
pub mod output;

pub use libobs_wrapper::{data::ObsObjectUpdater, sources::ObsSourceBuilder};
