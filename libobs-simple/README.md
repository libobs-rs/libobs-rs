# libobs-simple

Convenience builders on top of [`libobs-wrapper`](https://crates.io/crates/libobs-wrapper) for common recording, replay-buffer, and capture workflows. It uses the wrapper's runtime, ownership, capability discovery, and validated native handles rather than maintaining a second lifetime model.

## Recording example

```rust,no_run
use libobs_simple::output::simple::{ObsContextSimpleExt, X264Preset};
use libobs_wrapper::{
    data::video::ObsVideoInfoBuilder,
    data::output::ObsOutputTrait,
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
        .x264_encoder(X264Preset::VeryFast)
        .build()?;

    output.start()?;
    std::thread::sleep(std::time::Duration::from_secs(5));
    output.stop()?;
    Ok(())
}
```

Generic hardware selection in the simple/replay builders now uses `libobs-wrapper`'s capability metadata instead of a separate hard-coded availability list. For applications assembling custom streaming/recording graphs, use `ObsContext::capabilities()` to select descriptors and `ObsContext::output_pipeline()` to validate the complete encoder/service/output combination before native creation.
