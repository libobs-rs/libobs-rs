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

#[cfg(windows)]
// I don't know why windows doesn't generate these impls, just keeping them here for now
mod manual_impls {
    use super::bindings::*;
    use std::fmt;

    impl fmt::Debug for obs_video_info {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("obs_video_info")
                .field("graphics_module", &self.graphics_module)
                .field("fps_num", &self.fps_num)
                .field("fps_den", &self.fps_den)
                .field("base_width", &self.base_width)
                .field("base_height", &self.base_height)
                .field("output_width", &self.output_width)
                .field("output_height", &self.output_height)
                .field("output_format", &self.output_format)
                .field("adapter", &self.adapter)
                .field("gpu_conversion", &self.gpu_conversion)
                .field("colorspace", &self.colorspace)
                .field("range", &self.range)
                .field("scale_type", &self.scale_type)
                .finish()
        }
    }

    impl Clone for gs_init_data {
        fn clone(&self) -> Self {
            *self
        }
    }
    impl Copy for gs_init_data {}
    impl fmt::Debug for gs_init_data {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("gs_init_data")
                .field("window", &self.window)
                .field("cx", &self.cx)
                .field("cy", &self.cy)
                .field("num_backbuffers", &self.num_backbuffers)
                .field("format", &self.format)
                .field("zsformat", &self.zsformat)
                .field("adapter", &self.adapter)
                .finish()
        }
    }

    impl fmt::Debug for obs_module_failure_info {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("obs_module_failure_info")
                .field("failed_modules", &self.failed_modules)
                .field("count", &self.count)
                .finish()
        }
    }
}
