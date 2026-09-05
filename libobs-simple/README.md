# libobs-simple

The opinionated convenience layer for `libobs-rs`. Use this crate when you want to **record, stream, replay, or create common capture sources without assembling the native OBS graph yourself**.

`libobs-simple` deliberately does not maintain its own native lifetime or plugin-discovery model. It builds on [`libobs-wrapper`](../libobs-wrapper/README.md), which remains the full safe OBS object model.

> **Rule of thumb:** `libobs-simple` chooses for you; `libobs-wrapper` exposes the choices.

## Where do I start?

| Goal | Start with |
| --- | --- |
| Record to a file | `output::simple::ObsContextSimpleExt::simple_output_builder` |
| Stream to a custom RTMP endpoint | `output::streaming::ObsContextStreamingExt::simple_rtmp_stream` |
| Replay buffer | `output::replay` |
| Versioned scene collection save/restore/history | `scene_collection` |
| Common monitor/window/platform capture sources | `sources` |
| Need groups, generic plugin settings, custom output graphs, arbitrary services, or exact capability control | Drop down to the re-exported `libobs_simple::wrapper` / `libobs-wrapper` |

See the repository [API orientation guide](../docs/api_orientation.md) for the complete module map.

## Recording

```rust,no_run
use libobs_simple::output::simple::ObsContextSimpleExt;
use libobs_wrapper::{
    data::{output::ObsOutputTrait, video::ObsVideoInfoBuilder},
    utils::{ObsPath, StartupInfo},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = StartupInfo::new()
        .set_video_info(ObsVideoInfoBuilder::new().build())
        .start()?;

    let output = context
        .simple_output_builder("recording", ObsPath::new("recording.mp4"))
        .video_bitrate(6_000)
        .audio_bitrate(160)
        .build()?;

    output.start()?;
    std::thread::sleep(std::time::Duration::from_secs(5));
    output.stop()?;
    Ok(())
}
```

By default the builder asks the loaded OBS installation for H.264, **prefers an available hardware encoder, and transparently falls back to software only for encoder/backend failures**. Destination/auth/path failures are not retried as encoder fallbacks. `build_with_report()` / managed-start reports expose the concrete backend, applied/ignored settings, and fallback attempts; strict configuration mode turns unsupported requested properties into errors. You can still explicitly select x264, require hardware, or choose a custom encoder when you need control.

## RTMP streaming

```rust,no_run
use libobs_simple::output::streaming::ObsContextStreamingExt;
use libobs_wrapper::{
    data::output::ObsOutputTrait,
    utils::StartupInfo,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = StartupInfo::new().start()?;

    let output = context
        .simple_rtmp_stream("live", "rtmp://example.invalid/live", "stream-key")
        .video_bitrate(6_000)
        .audio_bitrate(160)
        .build()?;

    // build() only prepares the graph. Network I/O starts here:
    output.start()?;
    // ...
    output.stop()?;
    Ok(())
}
```

The streaming builder discovers a compatible encoded RTMP output, H.264 encoder and AAC encoder at runtime, configures the standard custom RTMP service, then uses the wrapper's validated output pipeline. It does not hard-code NVENC/QSV/AMF/x264 as the only valid installation.

## When to move down to `libobs-wrapper`

Use the full wrapper when the application needs any of these:

- enumerate arbitrary OBS plugins and modules;
- generate settings UIs from OBS property metadata;
- inspect current/default typed property values;
- create plugin-generic sources/encoders/services;
- work with native scene ordering, groups, transform snapshots or blend modes;
- assemble a custom codec/protocol/output graph;
- create displays/previews or subscribe to detailed signals;
- access libobs functionality that has no simple opinionated workflow.

You do not need a second context. `libobs-simple` uses the same `libobs_wrapper::context::ObsContext`, and `libobs_simple::wrapper` re-exports the wrapper crate so an application can mix simple and advanced calls naturally.


## Scene collections and richer application workflows

`scene_collection::SceneCollectionDocument` persists the managed libobs-rs scene model with stable source UUIDs, shared-source identity, filters, native groups/order, transforms, settings and hotkeys. Portable export redacts likely secrets and absolute machine-local paths by default; restoration can be strict or best-effort with structured diagnostics for missing plugins/files. See [`examples/scene-collection`](../examples/scene-collection).

For independent audio mixes/tracks see [`examples/audio-mixing`](../examples/audio-mixing). Output health snapshots and bitrate sampling live in the re-exported wrapper and are demonstrated by [`examples/output-health`](../examples/output-health).
