# libobs-wrapper v10 migration

Version 10 is an intentional architecture-breaking release focused on native lifetime and threading safety.

## Runtime and cleanup

The default runtime is now a bounded actor and no longer depends on Tokio. `no_blocking_drops` remains as a compatibility feature name but cleanup is always deferred to the OBS actor. `run_with_obs!` now preserves structured `ObsError` values instead of converting them to `InvocationError(String)`.

## Native handles

`Sendable` and `SmartPointerSendable` can no longer be constructed by downstream safe code. Owned native handles are backed by `NativeObjectId` entries in the runtime registry. Code that only compared raw pointers should compare `native_id()` instead.

Signal payloads that previously exposed a sendable callback raw pointer now expose `SignalObjectId`. It is an identity token, not a dereferenceable pointer.

## Signals

Signal subscription methods return `SignalReceiver<T>` rather than `tokio::sync::broadcast::Receiver<T>`. Synchronous code can use `recv`, `blocking_recv`, or `try_recv`. Each subscriber queue is bounded; if a subscriber falls behind, new events for that subscriber are dropped rather than blocking OBS callbacks or growing memory without bound. Async applications can bridge the receiver at their application boundary instead of making Tokio part of the wrapper core.

## `ObsData`

`ObsData` no longer implements `Clone`, because cloning requires a fallible native/JSON round-trip. Use `data.try_clone()?` when an independent mutable copy is required. `ImmutableObsData` remains cheaply cloneable.

## Context and scene lookups

Read-only context lookups now take `&self`. Scene source storage uses `Arc<dyn ObsSourceTrait>` rather than `Arc<Box<dyn ObsSourceTrait>>`. `get_source_mut` is deprecated because it never returned a mutable reference; use `get_source`.

## Windows source helpers

`MonitorCaptureSource::get_monitors()` returns `Vec<DisplayInfo>` and `WindowCaptureSource::get_windows()` returns `Vec<WindowInfo>`. Their setters accept references to those ordinary Rust types; the old generic sendability wrapper is gone.

## Native Unix display handles

Use the explicit unsafe constructors `NixDisplay::x11(raw)` and `NixDisplay::wayland(raw)`. The caller must guarantee the native display outlives the OBS context.

## Build tooling

`cargo-obs-build` v3 resolves Cargo `TARGET` and scopes caches by target triple. `ObsBuildConfig` has an optional `target` field; build scripts normally leave it `None` so Cargo's `TARGET` is used automatically. macOS targets are rejected explicitly until macOS prebuilt support exists.
