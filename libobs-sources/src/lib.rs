//! A library for creating OBS sources without having to figure out what properties are used by sources.
//!
//! This crate provides convenient builders for OBS sources across different platforms:
//! - **Windows**: Window capture, monitor capture, game capture
//! - **Linux**: X11 screen capture, XComposite window capture, V4L2 camera, ALSA/PulseAudio/JACK audio, PipeWire
//!
//! # Windows Example
//! ```no_run
//! #[cfg(target_family = "windows")]
//! use libobs_sources::windows::WindowCaptureSourceBuilder;
//! use libobs_wrapper::{context::ObsContext, sources::ObsSourceBuilder, utils::{StartupInfo}};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let startup_info = StartupInfo::default();
//! # let mut context = ObsContext::new(startup_info)?;
//! # let mut scene = context.scene("Main Scene")?;
//!
//! #[cfg(target_family = "windows")]
//! {
//!     # #[cfg(feature = "window-list")]
//!     use libobs_window_helper::WindowSearchMode;
//!     
//!     # #[cfg(feature = "window-list")]
//!     let windows = WindowCaptureSourceBuilder::get_windows(WindowSearchMode::IncludeMinimized)?;
//!     # #[cfg(feature = "window-list")]
//!     let example_window = windows.get(0).ok_or("No windows found")?;
//!
//!     # #[cfg(feature = "window-list")]
//!     let source = context
//!         .source_builder::<WindowCaptureSourceBuilder, _>("Test Window Capture")?
//!         .set_window(example_window)
//!         .add_to_scene(&mut scene)?;
//! }
//! # Ok(())
//! # }
//! ```

//!
//! # Linux Example
//! ```no_run
//! #[cfg(target_os = "linux")]
//! use libobs_sources::linux::{
//!     LinuxGeneralScreenCapture, LinuxGeneralWindowCapture,
//!     X11CaptureSourceBuilder, V4L2InputSourceBuilder, AlsaInputSourceBuilder
//! };
//! use libobs_wrapper::{context::ObsContext, sources::ObsSourceBuilder, utils::StartupInfo};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let startup_info = StartupInfo::default();
//! # let mut context = ObsContext::new(startup_info)?;
//! # let mut scene = context.scene("Main Scene")?;
//!
//! #[cfg(target_os = "linux")]
//! {
//!     // General screen capture (auto-detects X11 vs Wayland/PipeWire)
//!     let screen_capture = LinuxGeneralScreenCapture::auto_detect(
//!         context.runtime().clone(),
//!         "Screen Capture"
//!     )?;
//!     scene.add(&screen_capture)?;
//!
//!     // General window capture (auto-detects X11 vs Wayland/PipeWire)
//!     let window_capture = LinuxGeneralWindowCapture::auto_detect(
//!         context.runtime().clone(),
//!         "Window Capture"
//!     )?;
//!     scene.add(&window_capture)?;
//!
//!     // Or use specific X11 screen capture
//!     let screen_source = context
//!         .source_builder::<X11CaptureSourceBuilder, _>("X11 Screen")?
//!         .set_screen(0)
//!         .set_show_cursor(true)
//!         .add_to_scene(&mut scene)?;
//!
//!     // Camera capture
//!     let camera_source = context
//!         .source_builder::<V4L2InputSourceBuilder, _>("Webcam")?
//!         .set_device_id("/dev/video0".to_string())
//!         .set_resolution(1920 << 16 | 1080) // 1920x1080
//!         .set_framerate(30 << 16 | 1)        // 30 fps
//!         .add_to_scene(&mut scene)?;
//!
//!     // Audio input
//!     let audio_source = context
//!         .source_builder::<AlsaInputSourceBuilder, _>("Microphone")?
//!         .set_alsa_device("default")
//!         .set_rate(44100)
//!         .add_to_scene(&mut scene)?;
//! }
//! # Ok(())
//! # }
//! ```

#[cfg(target_family = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

pub mod encoders;
mod macro_helper;
pub mod output;

pub use libobs_wrapper::{data::ObsObjectUpdater, sources::ObsSourceBuilder};
