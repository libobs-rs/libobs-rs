//!
//! A library for creating OBS sources without having to figure out what properties are used by sources.
//!
//! This crate provides convenient builders for OBS sources across different platforms:
//! - **Windows**: Window capture, monitor capture, game capture
//! - **Linux**: X11 screen capture, XComposite window capture, V4L2 camera, ALSA/PulseAudio/JACK audio, PipeWire
//!
//! You can find examples [here](https://github.com/libobs-rs/libobs-rs/tree/main/examples).

#[cfg(any(windows, doc))]
#[cfg_attr(doc, doc(cfg(windows)))]
pub mod windows;

#[cfg(any(target_os = "linux", doc))]
#[cfg_attr(doc, doc(cfg(target_os = "linux")))]
pub mod linux;

mod either;
mod macro_helper;

pub use either::*;
pub use libobs_wrapper::{data::ObsObjectUpdater, sources::ObsSourceBuilder};
