# libobs-wrapper v10 migration

Version 10 is an intentional architecture-breaking release focused on native lifetime, threading, and API safety. The post-refactor audit tightened several v10 boundaries further; code written against early v10 snapshots may need the changes below.

## Runtime and cleanup

The default runtime is a bounded actor and no longer depends on Tokio. `no_blocking_drops` remains as a compatibility feature name, but cleanup is always deferred to the OBS actor.

Normal actor submissions are bounded for both synchronous and fire-and-forget calls. Saturation returns `ObsError::RuntimeQueueFull` instead of blocking for queue capacity. Runtime shutdown/failure is distinguished by structured errors such as `RuntimePanicked`, `RuntimeMismatch`, and `RuntimeReentrantBlocking` rather than stringifying actor failures into `InvocationError`.

Initialization and actor execution are panic-contained. Native cleanup is attempted before `obs_shutdown()` even after an actor command panic, and partial startup rolls back process-global OBS state.

## Native handles and `ObsObjectTrait`

`ObsObjectTrait<K>` is now `ObsObjectTrait`. The native pointer type is a hidden implementation detail. Safe downstream code should use the concrete high-level object types and `object_id()` when stable identity/hash comparison is needed.

`NativeObjectId` is runtime-scoped. Do not persist its diagnostic sequence value or treat it as a pointer/address.

Public safe `as_ptr()`/`get_ptr()` escape hatches have been removed from ordinary wrapper objects. `Sendable` is crate-private, and managed native pointers can only be extracted through an explicitly unsafe, doc-hidden integration seam. This intentionally breaks code that used wrapper internals as a general way to make arbitrary pointer-shaped values `Send` or `Sync`.

If an advanced integration genuinely needs the process-global audio/video output pointer, `ObsContext::get_audio_ptr()` and `get_video_ptr()` remain explicit `unsafe` APIs and return raw pointers. The caller is responsible for the native lifetime, reference-counting, and threading rules.

## Runtime affinity

Objects remember the runtime that owns them. Operations that combine objects now reject cross-runtime values with `ObsError::RuntimeMismatch`. This applies to scene/source/filter relationships and output/encoder attachment paths.

## Signals

Signal subscription methods return `SignalReceiver<T>` rather than `tokio::sync::broadcast::Receiver<T>`. Synchronous code can use `recv`, `blocking_recv`, or `try_recv`. Each subscriber queue is bounded; if a subscriber falls behind, new events for that subscriber are dropped rather than blocking OBS callbacks or growing memory without bound.

Signal pointer fields are exposed only as `SignalObjectId`, which is an opaque callback identity and never a dereferenceable native pointer. Callback connection is atomic at the actor-command boundary so failed construction cannot leave stale userdata registered in C.

## `ObsData`, strings, and properties

`ObsData` no longer implements `Clone`, because cloning requires a fallible native/JSON round-trip. Use `data.try_clone()?` when an independent mutable copy is required. `ImmutableObsData` remains cheaply cloneable.

`ObsString::as_ptr()` is no longer part of the safe public API. Use `as_c_str()` for ordinary C-string interop.

Legacy typed property access remains available, but generic discovery should prefer the owned `PropertyMetadata` model described below. Property-type mismatch is reported as `ObsError::PropertyTypeMismatch`.

## Scene and display handles

Scene and scene-item pointer accessors are internal. Use `object_id()` for identity. Display render callbacks now own per-display userdata rather than relying on a process-global position map.

Raw native window constructors are unsafe:

```rust,ignore
let hwnd = unsafe { ObsWindowHandle::new_from_handle(raw_hwnd) };
let wayland = unsafe { ObsWindowHandle::new_from_wayland(raw_wl_surface) };
```

The supplied native handle must remain valid for every OBS display created from it and must obey the GUI toolkit's thread-affinity rules.

`ObsTransformInfo::get_bounds_type()` now returns `Option<ObsBoundsType>` so an enum value added by a newer libobs does not panic older wrapper code.

## Generic capability discovery

Applications no longer need to hard-code installed OBS plugins/features. `ObsContext` provides:

```rust,ignore
let capabilities = obs.capabilities()?;
for source in capabilities.source_types() {
    println!("{}: {:?}", source.id(), source.kind());
}

if let Some(source) = capabilities.source_types().first() {
    for property in source.properties()? {
        println!("{}: {:?}", property.name, property.kind);
    }
}
```

Separate query methods cover inputs, filters, transitions, outputs, encoders, services, output protocols, and loaded modules. Output/encoder metadata includes codecs where libobs exposes them. Source/output/encoder/service descriptors can return owned property metadata and default `ImmutableObsData` settings. They also expose `default_settings_mut()` for an editable copy of plugin defaults.

Discovery is now directly actionable. Descriptor-aware creation methods keep type IDs and runtime affinity together:

```rust,ignore
let color = obs.source_type("color_source_v3")?.expect("plugin available");
let mut settings = color.default_settings_mut()?;
settings.set_int("width", 1280)?.set_int("height", 720)?;
let source = obs.create_source(&color, "Background", Some(settings))?;

let capabilities = obs.capabilities()?;
let video_ty = capabilities
    .select_video_encoder()
    .codec("h264")
    .prefer_hardware()
    .best_available()
    .expect("H.264 encoder available");
let audio_ty = capabilities
    .select_audio_encoder()
    .codec("aac")
    .best_available()
    .expect("AAC encoder available");
let output_ty = capabilities
    .select_output()
    .protocol("RTMP")
    .video_codec("h264")
    .audio_codec("aac")
    .best_available()
    .expect("compatible RTMP output available");

let service_ty = obs.service_type("rtmp_custom")?.expect("plugin available");
let service = obs.create_service(&service_ty, "Stream", None)?;
let video_encoder = obs.create_video_encoder(video_ty, "Video", None)?;
let audio_encoder = obs.create_audio_encoder(audio_ty, "Audio", None, 0)?;
let pipeline = obs
    .output_pipeline(output_ty, "RTMP", None)
    .video_encoder(video_encoder)
    .audio_encoder(0, audio_encoder)
    .service(service)
    .build()?;
```

Equivalent typed creation methods exist for filters and audio/video encoders. Passing a descriptor from another runtime, or using an encoder/source descriptor with the wrong category, returns a structured error before native creation.

Unknown property/list/category values are represented with `Unknown(...)` variants for forward compatibility. Discovery results never expose temporary libobs strings or property pointers.

`ObsOutputPipelineBuilder` is now the preferred complete-output path. It validates output flags, required components, runtime affinity, mixer indices, encoder codecs, and service protocol before creating or mutating the output. `ObsOutputComposition` remains the lower-level desired-state wiring mechanism underneath it.

Legacy `ObsContextEncoders::{best,available}_{video,audio}_encoder(s)` discovery is deprecated; use `ObsContext::capabilities()` with `select_video_encoder()` / `select_audio_encoder()` instead. Output attachment getters now use `attached_*` names; the old `get_current_*` methods remain deprecated compatibility shims.

Scenes also expose inherent `add`, `remove_item`, `items_for_source`, and `clear` methods. `remove_item(&item)` now detaches immediately; a scene-item handle owns a native reference so cloned handles cannot become dangling merely because the item was removed from the scene. Position/scale shorthand plus rotation, explicit `ObsSceneItemCrop` edge cropping, visibility, locking, and ordering operations are available on `SceneItemTrait`.

See [`examples/capability-discovery`](../examples/capability-discovery) for a complete executable example.

## Context and scene lookups

Read-only context lookups take `&self`. Scene source storage uses `Arc<dyn ObsSourceTrait>` rather than `Arc<Box<dyn ObsSourceTrait>>`. `get_source_mut` is deprecated because it never returned a mutable reference; use `get_source`.

## Windows source helpers

`MonitorCaptureSource::get_monitors()` returns `Vec<DisplayInfo>` and `WindowCaptureSource::get_windows()` returns `Vec<WindowInfo>`. Their setters accept references to those ordinary Rust types; the old generic sendability wrapper is gone.

## Native Unix display handles

Use the explicit unsafe constructors `NixDisplay::x11(raw)` and `NixDisplay::wayland(raw)`. The caller must guarantee the native display outlives the OBS context.

## Build tooling

`cargo-obs-build` v3 resolves Cargo `TARGET` and scopes caches by target triple. `ObsBuildConfig` has an optional `target` field; build scripts normally leave it `None` so Cargo's `TARGET` is used automatically. macOS targets are rejected explicitly until macOS prebuilt support exists.
