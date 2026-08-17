# How libobs-rs Works

`libobs-rs` is a workspace of progressively safer layers over the OBS Studio C API.

## Architecture

### `libobs`

The sys crate contains raw bindgen-generated C bindings. `libobs/OBS_VERSION` is the canonical OBS version represented by the vendored headers and drives Linux pkg-config checks, CI builds, the bootstrapper, and `cargo-obs-build`.

### `libobs-wrapper`

The wrapper treats process-global libobs state as a single actor. With the default runtime enabled, application threads submit work to one bounded command queue; all native calls execute on the dedicated OBS thread. A separate cleanup queue lets Rust destructors schedule native release operations without blocking or requiring Tokio.

Native objects are registered in a runtime-owned registry. Public wrapper clones carry an opaque `NativeObjectId` and a lease rather than copying raw OBS pointers. The final lease removes the registry entry and schedules the corresponding native release before runtime shutdown. Context-level scenes, outputs, filters, and displays are additionally collected by an internal `ObjectRegistry`, so correctness does not depend on `ObsContext` field declaration order.

Signals are per-object. Each signal manager owns its subscription hubs and passes stable hub addresses to libobs callbacks. Callback-only native pointers are converted to opaque `SignalObjectId` values instead of being sent through channels as dereferenceable raw pointers. The wrapper core has no Tokio dependency.

### `libobs-simple`

The convenience layer provides typed source and output builders on top of `libobs-wrapper`. It does not own a second runtime or lifetime model. Platform helpers return ordinary Rust types (`DisplayInfo`, `WindowInfo`, etc.) instead of wrapping them in unsafe generic sendability adapters.

### `libobs-bootstrapper` and `cargo-obs-build`

The bootstrapper installs compatible runtime binaries. `cargo-obs-build` resolves the actual Cargo target triple rather than the build host; Windows x64/ARM64 artifacts are selected only for Windows targets, Linux uses system/source libobs, and unsupported macOS prebuilt resolution fails explicitly.

## Data Flow

1. **Resolve OBS** — build/bootstrap tooling uses the canonical supported OBS version and target triple.
2. **Initialize** — `ObsContext` reserves the process-global runtime slot and starts the OBS actor.
3. **Create objects** — native objects are created on the actor and registered under opaque runtime IDs.
4. **Operate** — safe wrappers dispatch typed closures to the bounded actor queue.
5. **Observe** — per-object signal hubs fan out owned event data to subscribers.
6. **Destroy** — final handle leases unregister objects and enqueue native cleanup; shutdown drains queued work before `obs_shutdown()`.
