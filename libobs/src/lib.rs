#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    unnecessary_transmutes,
    non_snake_case,
    clippy::all
)]

//! # LibOBS bindings (and wrapper) for rust
//! This crate provides bindings to the [LibOBS](https://obsproject.com/) library for rust.
//! Furthermore, this crate provides a safe wrapper around the unsafe functions, which can be found in the [`libobs-wrapper`](https://crates.io/crates/libobs-wrapper) crate.

#[cfg_attr(coverage_nightly, coverage(off))]
mod bindings {
    #[cfg(any(feature = "generate_bindings", target_family = "unix"))]
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

    #[cfg(all(not(feature = "generate_bindings"), target_family = "windows"))]
    include!("bindings_win.rs");
}

pub use bindings::*;
