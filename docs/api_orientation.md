# libobs-rs API orientation

The repository intentionally has several layers because applications need very different levels of control. Start as high as possible and move downward only when the higher layer does not expose the behavior you need.

```text
Your application
      |
      | common recording / streaming / capture workflows
      v
libobs-simple
      |
      | safe OBS object model, runtime discovery, generic settings,
      | scenes, groups, outputs, services, signals, displays
      v
libobs-wrapper
      |
      | generated FFI declarations
      v
libobs
      |
      v
native libobs / OBS plugins
```

## Which crate should I use?

| Goal | Start here | Why |
| --- | --- | --- |
| Record or stream with sensible defaults | `libobs-simple` | It chooses compatible OBS implementations and wires the graph for you. |
| Capture a window/monitor with convenience builders | `libobs-simple::sources` | Platform-specific source setup is hidden behind Rust builders. |
| Build an OBS-like editor, remote controller, or custom media application | `libobs-wrapper` | It exposes the safe runtime-affine OBS object model and plugin discovery. |
| Configure arbitrary third-party OBS plugins at runtime | `libobs-wrapper::capabilities` + `libobs-wrapper::settings` | Property metadata and values are discovered instead of hard-coded. |
| Compose scenes, groups, transforms, filters, and ordering | `libobs-wrapper::scenes` + `libobs-wrapper::sources` | These mirror libobs concepts while owning native lifetimes safely. |
| Build a custom encoder/output/service graph | `libobs-wrapper::capabilities` + `libobs-wrapper::data::output` | Compatibility planning and pipeline validation happen before native mutation. |
| Create preview/display windows | `libobs-wrapper::display` | Native display lifetime and callback details stay inside the wrapper. |
| Call a libobs function the wrapper does not expose yet | `libobs-wrapper::sys` / `libobs` | This is the unsafe escape hatch. Prefer adding a safe wrapper for reusable functionality. |
| Ship or prepare OBS binaries | `libobs-bootstrapper` / `cargo-obs-build` | Runtime and build-time installation are intentionally separate from the object model. |

A useful rule is: **`libobs-simple` chooses for you; `libobs-wrapper` exposes the choices.**

## `libobs-wrapper`: module map

Most applications using the full wrapper only need a small subset of modules at a time.

### Runtime and startup

- `utils::StartupInfo` configures startup and produces an `ObsContext`.
- `context::ObsContext` is the root safe handle for one libobs runtime.
- `runtime` contains the actor/runtime machinery. Normal callers usually do not need to interact with it directly.

All managed OBS objects belong to the runtime that created them. The wrapper checks runtime affinity before combining objects.

### Discover what this OBS installation can do

Use `capabilities` when functionality may vary with installed OBS plugins:

- source/filter/transition types;
- outputs and their codec/protocol flags;
- encoders and codecs;
- services;
- loaded modules;
- compatibility planning for output graphs.

For example, use `ObsCapabilities::best_output_plan` when you care about *H.264 over RTMP* rather than a specific NVENC/QSV/x264 plugin ID.

### Configure arbitrary plugins

Use `settings` together with descriptor methods such as `EncoderTypeInfo::settings_snapshot_for`.

The generic settings layer exposes:

- recursive property metadata;
- numeric ranges and steps;
- lists/enums and disabled entries;
- visibility/enabled state;
- current and default typed values;
- editable-list entries and font objects using OBS's native data shapes;
- frame-rate options as well as numeric rates;
- checkable group values;
- validation before mutation;
- dynamic refresh through OBS property-modified callbacks.

Button properties remain action metadata rather than `ObsData` values: invoking an OBS property button is instance-specific and is intentionally not disguised as a settings assignment.

This is the preferred foundation for generated GUIs, CLI configuration, or remote-control schemas. The older `data::properties` typed wrappers remain available for callers that already use them.

### Sources and filters

Use `sources` for managed source/filter handles and source-level behavior. A discovered `SourceTypeInfo` can create a source without a plugin-specific builder.

Use `libobs-simple::sources` instead when a convenience builder already exists and you do not need plugin-generic configuration.

### Scenes and composition

Use `scenes` for:

- scenes and scene items;
- native bottom-to-top item order;
- position, scale, rotation, bounds and crop;
- visibility and locking;
- scale filters and blend modes;
- transform/state snapshots;
- managed OBS groups and group child enumeration.

`ObsSceneItemRef<T>` preserves the concrete source type for ordinary source insertion. `ObsSceneItemHandle` is the type-erased managed handle used when libobs itself replaces a native scene item, notably while ungrouping.

OBS has an important native semantic here: ungrouping creates replacement parent-scene items. `ObsSceneGroupRef::ungroup` therefore returns mappings from previous object IDs to replacement handles rather than pretending the old handles still identify the visible items.

### Encoders, services and outputs

Use:

- `encoders` for managed audio/video encoders;
- `services` for streaming-service objects;
- `data::output` for output lifecycle and composition;
- `capabilities` to choose compatible concrete implementations.

There are two output levels:

1. `ObsOutputPipelineBuilder` validates a complete graph before output creation. Prefer it for new output graphs.
2. `ObsOutputComposition` and `ObsOutputTrait` expose lower-level attachment/detachment and lifecycle behavior when your application intentionally manages the graph itself.

### Signals and displays

- `signals` and per-object `signals()` accessors expose libobs events while retaining callback lifetimes safely.
- `display` manages preview/display surfaces.

### Raw escape hatch

`libobs-wrapper::sys` re-exports the raw `libobs` bindings. Managed handles also have deliberately unsafe, doc-hidden pointer escape hatches for advanced integrations. Crossing this seam means the caller owns libobs thread-affinity and reference-count rules.

If the same raw operation is useful to multiple applications, prefer adding it to `libobs-wrapper` instead of duplicating unsafe FFI downstream.

## Common workflows

### 1. Common recording: stay in `libobs-simple`

```rust,no_run
use libobs_simple::output::simple::ObsContextSimpleExt;
use libobs_wrapper::utils::{ObsPath, StartupInfo};

let context = StartupInfo::new()
    // Embedders should keep OBS plugin state in an application-owned writable directory.
    .set_module_config_path(ObsPath::new("/path/to/app/config/obs-modules"))
    .start()?;
let output = context
    .simple_output_builder("recording", ObsPath::new("recording.mp4"))
    .video_bitrate(6_000)
    .audio_bitrate(160)
    .build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The default video path asks the loaded OBS installation for H.264 and prefers hardware while retaining software fallback.

### 2. Common RTMP streaming: stay in `libobs-simple`

```rust,no_run
use libobs_simple::output::streaming::ObsContextStreamingExt;
use libobs_wrapper::utils::StartupInfo;

let context = StartupInfo::new().start()?;
let output = context
    .simple_rtmp_stream("live", "rtmp://example.invalid/live", "stream-key")
    .video_bitrate(6_000)
    .build()?;

// output.start()? when you are ready to connect.
# Ok::<(), Box<dyn std::error::Error>>(())
```

### 3. Plugin-generic configuration: use wrapper capabilities + settings

```rust,no_run
use libobs_wrapper::{
    settings::PropertyValue,
    utils::StartupInfo,
};

let context = StartupInfo::new().start()?;
let encoder = context
    .capabilities()?
    .select_video_encoder()
    .codec("h264")
    .prefer_hardware()
    .best_available()
    .cloned()
    .ok_or("no H.264 encoder")?;

let mut settings = encoder.default_settings_mut()?;
let schema = encoder.settings_schema_for(&settings)?;
if schema.property("bitrate").is_some() {
    schema.set(&mut settings, "bitrate", PropertyValue::Integer(6_000))?;
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

### 4. Custom output graph: use compatibility planning + a validated pipeline

Ask `ObsCapabilities::best_output_plan` for the media/protocol requirements, create the selected encoder/service objects, then pass them to `ObsContext::output_pipeline`. The pipeline validates runtime affinity, output flags, mixer indices, codecs, and service protocol before it creates or mutates the output.

### 5. Scene editor: use native-order items and snapshots

Use `ObsSceneRef::items_in_order` for the actual libobs order, `SceneItemTrait::state_snapshot` for a restorable item state, and `ObsSceneRef::create_group` for native OBS groups. Use `ObsSceneGroupRef::items_in_order` to enumerate group children.

## Ownership model in one paragraph

Managed wrapper objects own native references through runtime-affine leases. Cloning a Rust handle shares that lifetime; final native release is scheduled through the OBS runtime. Opaque `NativeObjectId` values are for identity, not pointer recovery. Do not cache raw pointers. When libobs has replacement semantics—as with group ungrouping—the wrapper returns new managed handles instead of aliasing a replaced native object.

## Where should new functionality live?

When contributing:

- Put a safe representation of broadly useful libobs functionality in **`libobs-wrapper`**.
- Put opinionated defaults, automatic selection, and common multi-step workflows in **`libobs-simple`**.
- Keep raw FFI in **`libobs`**.
- Avoid implementing the same capability discovery, native lifetime, or compatibility logic separately in `libobs-simple`.

That layering keeps `libobs-wrapper` comprehensive while letting `libobs-simple` remain genuinely simple.
