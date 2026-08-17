//! This crate contains a high level API wrapper around the OBs C API.
//! To get started, have a look at the [examples](https://github.com/libobs-rs/libobs-rs/tree/main/examples) folder.
//!
//! For documentation, have a look at the [docs](https://libobs-rs.github.io/libobs_wrapper/).
//!
//! Also have a look at the [libobs-simple](https://crates.io/crates/libobs-simple) crate, which has a lot of
//! source builders for easier source creation.
//! You can also create outputs easily with it.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod capabilities;
pub mod context;
pub mod crash_handler;
pub mod data;
pub mod display;
pub mod encoders;
pub mod enums;
pub mod logger;
pub mod runtime;
pub mod scenes;
pub mod signals;
pub mod sources;
#[doc(hidden)]
pub mod unsafe_send;
pub mod utils;

pub use libobs as sys;

// Add the macros module to the public exports
pub mod graphics;
#[cfg_attr(coverage_nightly, coverage(off))]
mod macros;
