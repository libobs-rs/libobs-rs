#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![allow(
    unknown_lints,
    non_camel_case_types,
    non_upper_case_globals,
    unnecessary_transmutes,
    non_snake_case,
    no_unqualified_libobs_uses,
    ensure_obs_call_in_runtime,
    require_safety_comments_on_unsafe,
    clippy::all
)]

//! # LibOBS bindings (and wrapper) for rust
//! This crate provides bindings to the [LibOBS](https://obsproject.com/) library for rust.
//! Furthermore, this crate provides a safe wrapper around the unsafe functions, which can be found in the [`libobs-wrapper`](https://crates.io/crates/libobs-wrapper) crate.

#[cfg_attr(coverage_nightly, coverage(off))]
// Bindgen emits libc declarations whose ABI-equivalent C integer types are not
// written as Rust's `usize`; newer Rust versions lint those names as potential
// runtime symbol overrides. These declarations are generated, not exports.
#[allow(suspicious_runtime_symbol_definitions)]
mod bindings {
    #[cfg(any(feature = "generate_bindings", target_family = "unix"))]
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

    #[cfg(all(not(feature = "generate_bindings"), target_family = "windows"))]
    include!("bindings_win.rs");
}

pub use bindings::*;

/// Exact OBS Studio/libobs version represented by the vendored headers.
pub const SUPPORTED_OBS_VERSION: &str = include_str!("../OBS_VERSION");
