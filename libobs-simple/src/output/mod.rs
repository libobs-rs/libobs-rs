//! Common outputs with opinionated defaults.
//!
//! - [`simple`] builds file recording outputs and automatically selects a compatible encoder.
//! - [`streaming`] builds an H.264/AAC custom RTMP graph.
//! - [`replay`] provides replay-buffer convenience behavior.
//!
//! For custom protocols, services, track layouts, or exact encoder selection, use
//! [`libobs_wrapper::data::output`] and [`libobs_wrapper::capabilities`] directly.

mod configure;

pub mod replay;
pub mod simple;
pub mod streaming;
