#![cfg_attr(all(doc, not(doctest)), feature(doc_cfg))]
//! Opinionated convenience workflows on top of `libobs-wrapper`.
//!
//! # Start here for common applications
//!
//! `libobs-simple` is the layer that **chooses sensible OBS implementations for you** while the
//! underlying `libobs-wrapper` remains available for full control. Both layers use the same
//! [`wrapper::context::ObsContext`].
//!
//! | Goal | Start with |
//! | --- | --- |
//! | Record to a file | [`output::simple`] |
//! | Stream H.264/AAC to RTMP | [`output::streaming`] |
//! | Replay buffer | [`output::replay`] |
//! | Common window/monitor/platform sources | [`sources`] |
//! | Anything more custom | [`wrapper`] |
//!
//! Recording and streaming builders discover the loaded OBS capabilities rather than assuming a
//! particular NVENC/QSV/AMF/x264 backend. The simple layer delegates native lifetimes, dynamic
//! settings validation, and output graph validation to [`wrapper`].
//!
//! Drop down to [`wrapper::capabilities`] and [`wrapper::settings`] for arbitrary plugins, or
//! [`wrapper::scenes`] for groups/native ordering/full transform state. You do not need to create a
//! second context to mix the two layers.

pub mod error;
pub mod output;
pub mod sources;

pub use error::ObsSimpleError;
pub use libobs_wrapper as wrapper;
