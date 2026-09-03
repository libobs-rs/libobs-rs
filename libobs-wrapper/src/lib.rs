//! Safe, runtime-affine Rust access to the libobs object model.
//!
//! # Start here
//!
//! [`crate::context::ObsContext`] is the root managed handle. Most applications create it through
//! [`crate::utils::StartupInfo`], then use only the modules needed for their workflow.
//!
//! | What you need | Module |
//! | --- | --- |
//! | Discover installed sources/encoders/outputs/services | [`crate::capabilities`] |
//! | Configure arbitrary plugins from runtime metadata | [`crate::settings`] |
//! | Sources and filters | [`crate::sources`] |
//! | Scenes, groups, native order and transform snapshots | [`crate::scenes`] |
//! | Audio/video encoders | [`crate::encoders`] |
//! | Streaming services | [`crate::services`] |
//! | Validated output graphs and lifecycle | [`crate::data::output`] |
//! | Preview/display surfaces | [`crate::display`] |
//! | OBS event subscriptions | [`crate::signals`] and per-object `signals()` methods |
//! | Raw FFI escape hatch | [`crate::sys`] |
//!
//! If you only need a conventional recorder, RTMP stream, replay buffer, or common capture source,
//! the `libobs-simple` crate is normally a better starting point. It uses this wrapper internally
//! and can be mixed with wrapper calls on the same [`crate::context::ObsContext`].
//!
//! # Plugin-generic workflow
//!
//! Do not assume every machine has the same OBS plugins. The preferred generic path is:
//!
//! 1. call [`crate::context::ObsContext::capabilities`];
//! 2. choose by codec/protocol/capability, or use
//!    [`crate::capabilities::ObsCapabilities::best_output_plan`];
//! 3. use descriptor `settings_schema_for` / `settings_snapshot_for` methods to configure plugin
//!    settings without hard-coded property models;
//! 4. create managed objects from the descriptors;
//! 5. use [`crate::data::output::ObsOutputPipelineBuilder`] for complete output graphs.
//!
//! # Scene composition
//!
//! [`crate::scenes::ObsSceneRef`] owns managed scene items and exposes libobs's actual native order.
//! [`crate::scenes::SceneItemTrait`] covers transform/state behavior, including crop, bounds, scale
//! filters, blending, and restorable snapshots. [`crate::scenes::ObsSceneGroupRef`] represents native
//! OBS groups rather than simulating groups in Rust.
//!
//! One important libobs behavior is exposed explicitly: ungrouping creates replacement parent-scene
//! items. [`crate::scenes::ObsSceneGroupRef::ungroup`] therefore returns replacement managed handles
//! and marks the replaced child handles removed.
//!
//! # Ownership and unsafe access
//!
//! Managed objects retain native references through one OBS runtime. Clones share a lifetime lease,
//! and operations validate runtime affinity before combining objects. Prefer opaque object IDs for
//! identity. [`crate::sys`] is available when raw libobs functionality is genuinely needed, but raw
//! pointers carry the usual libobs thread-affinity and reference-count responsibilities.
//!
//! The repository's `docs/api_orientation.md` contains a longer task-to-module guide and examples.

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
pub mod services;
pub mod settings;
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
