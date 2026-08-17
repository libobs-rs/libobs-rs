# How libobs-rs Works

`libobs-rs` is a workspace of progressively safer layers over the OBS Studio C API. The v10 wrapper treats libobs as process-global, thread-affine native state rather than as ordinary Rust objects.

## Architecture

### `libobs`

The sys crate contains the raw bindgen-generated C bindings. `libobs/OBS_VERSION` is the canonical OBS version represented by the vendored headers and drives Linux pkg-config checks, CI builds, the bootstrapper, and `cargo-obs-build`.

### One process-global OBS actor

With the default `enable_runtime` feature, `ObsContext` reserves the single process-global libobs slot and starts one dedicated OBS actor thread. Normal synchronous and fire-and-forget work uses a bounded queue. Queue saturation is a typed `ObsError::RuntimeQueueFull` error; callers are never silently placed on an unbounded normal-work queue and synchronous submission does not wait indefinitely for queue capacity.

Native destruction has a separate cleanup queue. That queue exists specifically so `Drop` can remain non-blocking: production destructors enqueue cleanup and never wait for the actor or an async runtime. The final runtime guard requests shutdown and detaches the actor thread in production. The actor drains accepted work and native cleanup before calling `obs_shutdown()`.

Actor commands are panic-contained. A panic marks the runtime failed, rejects pending normal work, still attempts deferred native cleanup, and runs shutdown on the actor thread. Initialization has rollback guards so errors or panics after process-global activation cannot strand libobs in a half-started state.

The core wrapper has no Tokio dependency and remains synchronous to downstream callers.

### Native object registry and identity

Owned OBS objects use a self-contained shared native lease plus a runtime-owned identity registry. A managed handle contains:

- an opaque native address whose pointer type is sealed to wrapper internals;
- a runtime-scoped `NativeObjectId` made from a process runtime epoch plus an object sequence;
- a reference to the runtime identity/liveness registry; and
- the native release guard.

An already-live handle never looks its pointer back up in the registry. The lease itself is the source of truth for the native address and lifetime, while the registry tracks only opaque IDs for identity and diagnostics. This removes a formerly panic-based registry-resolution invariant from ordinary FFI calls.

Safe downstream code cannot construct an ID or native handle from an integer or pointer. A stale ID from a previous context cannot compare equal to an object created by a later context, even when their per-runtime sequence numbers are the same.

The final lease unregisters its opaque ID before the release guard schedules native destruction. Every native guard owns an `ObsRuntime` clone, so the actor and registry remain alive until all native cleanup that can still be required has been accepted. `ObsContext` also has an internal high-level object registry for scenes, outputs, filters, and displays; this is ownership bookkeeping, not a global native-pointer map.

### First-class object model and runtime affinity

Normal APIs use high-level Rust types such as `ObsSourceRef`, `ObsSceneRef`, `ObsFilterRef`, `ObsOutputRef`, `ObsAudioEncoder`, `ObsVideoEncoder`, and scene-item references. `ObsObjectTrait` is no longer generic over a public raw pointer type. Native representation is a doc-hidden implementation detail, while `object_id()` exposes only opaque identity.

Operations that combine independently owned objects verify runtime affinity before reaching FFI. Attaching a source/filter/encoder from another runtime therefore returns `ObsError::RuntimeMismatch` rather than operating on unrelated native state.

Raw pointers are restricted to narrow FFI boundaries. The remaining public raw-pointer operations are explicitly `unsafe` and document their lifetime/thread/ownership obligations. `ObsWindowHandle` constructors that accept HWND/Wayland pointers are also unsafe. Ordinary source, scene, output, encoder, data, property, and signal APIs do not require raw pointers.

### Signals and callbacks

Signal managers are owned per OBS object. All callback hubs are allocated first and then connected to C in one actor command, so construction cannot leave half-connected callback userdata behind. Destruction disconnects callbacks on the actor while retaining the hubs through deregistration.

Callback payloads are copied into owned Rust data before publication. Callback-only native object addresses become opaque `SignalObjectId` values; they are identity tokens, not dereferenceable handles and do not extend a C object's lifetime.

Each subscriber has a bounded channel. A slow subscriber drops new events for that subscriber rather than blocking a libobs callback or growing memory without bound. Subscriber removal during/after callbacks is synchronized by the hub.

Display rendering follows the same rule: per-display render state is owned by the display's release guard and remains alive through callback deregistration. Windows WndProc userdata stores only a mutex-protected borrowed display address; the owning handler keeps the managed display lease on the OBS-owning thread, and teardown clears the address under the same mutex before releasing that lease.

### Capability discovery

`ObsContext` can query the running libobs installation instead of relying on hard-coded plugin assumptions:

- `source_types()`, `input_types()`, `filter_types()`, and `transition_types()`;
- `output_types()`, including supported audio/video codecs, protocols, and output flags;
- `encoder_types()`, including codec and capability metadata;
- `service_types()`;
- `protocols()`;
- `loaded_modules()`; and
- `capabilities()` for one aggregate snapshot.

Source/output/encoder/service descriptors can query generic properties and default settings. `default_settings_mut()` creates an owned mutable settings object initialized from plugin defaults, and descriptor-aware `ObsContext::create_source`, `create_filter`, `create_output`, `create_video_encoder`, `create_audio_encoder`, and `create_service` methods turn discovery results directly into runtime-affine typed handles. A discovered source can also be created directly inside a scene with `ObsSceneRef::add_discovered_source`.

`ObsCapabilities` also provides deterministic selectors: `select_video_encoder()`, `select_audio_encoder()`, and `select_output()`. They filter by codec, protocol, native capability flags, and deprecated/internal status. `prefer_hardware()` ranks likely hardware encoders first while preserving same-codec software fallbacks. Selection is metadata-only: no native encoder/output is created until the caller chooses a descriptor.

For complete outputs, `ObsContext::output_pipeline()` is the preferred high-level seam. `ObsOutputPipelineBuilder::validate()` checks runtime affinity, required/forbidden components, audio mixer bounds, encoder codecs, service protocol, and output flags before output creation or native mutation. `build()` creates the output only after validation and applies the complete `ObsOutputComposition`. The lower-level composition and individual attach/detach methods remain available for applications that intentionally manage wiring themselves. Shared output handles expose `current_composition()` plus `attached_video_encoder()`, `attached_audio_encoder()` / `attached_audio_encoders()`, and `attached_service()`.

Property trees are copied recursively into owned `PropertyMetadata`/`PropertyKind` values and destroyed on the OBS actor before the result crosses the thread boundary. Lists, paths, numeric controls, groups, frame-rate metadata, and other common property kinds are represented directly. Unknown future property/category enum values are preserved as `Unknown(...)` rather than panicking.

Discovery descriptors retain the owning runtime so later property/default queries and typed creation can verify runtime affinity before reaching FFI.

### Scene and scene-item composition

`ObsSceneRef` exposes inherent `add`, `add_new_source`, `remove_item`, `items_for_source`, and `clear` operations, so normal callers do not need pointer-oriented extension names. `ObsSceneItemRef<T>` keeps the concrete source type and provides managed operations for position, scale, rotation, visibility, locking, absolute/relative ordering, full transform updates, and fitting to the current canvas.

Scene items take an explicit native reference when they are created. `remove_item(&item)` therefore detaches the item from the scene immediately while outstanding Rust clones remain valid managed references; the final handle releases the native reference exactly once. This avoids the older behavior where apparent removal could be delayed until every Rust clone happened to drop.

See [`examples/capability-discovery`](../examples/capability-discovery) for an end-to-end introspection example.

### `libobs-simple`

The convenience layer provides typed source and output builders on top of `libobs-wrapper`. It does not own a second runtime or lifetime model. Platform helpers return ordinary owned Rust values (`DisplayInfo`, `WindowInfo`, etc.) and reuse the wrapper's runtime affinity/native-handle rules.

### `libobs-bootstrapper` and `cargo-obs-build`

The bootstrapper installs compatible runtime binaries. `cargo-obs-build` resolves the actual Cargo target triple rather than the build host; Windows x64/ARM64 artifacts are selected only for Windows targets, Linux uses system/source libobs, and unsupported macOS prebuilt resolution fails explicitly.

## Lifetime sequence

1. **Resolve OBS** — build/bootstrap tooling uses the canonical supported OBS version and target triple.
2. **Reserve** — `ObsContext` atomically reserves the process-global runtime slot.
3. **Initialize** — the actor performs platform setup and `obs_startup`; rollback owns every partially initialized state.
4. **Create** — native objects are created on the actor and registered under runtime-scoped opaque IDs.
5. **Operate** — safe wrappers dispatch typed work to the bounded actor queue and verify runtime affinity where objects interact.
6. **Observe** — callback glue copies event data into bounded per-object signal hubs.
7. **Destroy** — final leases unregister opaque identities and enqueue actor-thread cleanup.
8. **Shutdown** — after the final runtime owner goes away, accepted work/cleanup is drained and `obs_shutdown()` runs on the actor.
