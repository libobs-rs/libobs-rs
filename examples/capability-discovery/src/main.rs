use anyhow::Result;
use libobs_wrapper::{
    data::{ObsDataGetters, ObsDataSetters, object::ObsObjectTrait},
    utils::StartupInfo,
};

fn main() -> Result<()> {
    let obs = StartupInfo::new().start()?;
    let capabilities = obs.capabilities()?;

    println!("Source types:");
    for source in capabilities.source_types() {
        println!(
            "  {} ({:?}){}",
            source.id(),
            source.kind(),
            source
                .display_name()
                .map(|name| format!(" — {name}"))
                .unwrap_or_default()
        );
    }

    if let Some(source) = capabilities.source_types().first() {
        println!("\nProperties for {}:", source.id());
        for property in source.properties()? {
            println!(
                "  {}: {:?} [visible={}, enabled={}]",
                property.name, property.kind, property.visible, property.enabled
            );
        }

        if let Some(defaults) = source.default_settings()? {
            println!("  default settings: {}", defaults.get_json()?);
        }
    }

    println!("\nEncoders:");
    for encoder in capabilities.encoders() {
        println!(
            "  {} ({:?}, codec={:?})",
            encoder.id(),
            encoder.kind(),
            encoder.codec()
        );
    }

    println!("\nOutputs:");
    for output in capabilities.outputs() {
        println!(
            "  {} (video={:?}, audio={:?}, protocols={:?}, flags={:?})",
            output.id(),
            output.video_codecs(),
            output.audio_codecs(),
            output.protocols(),
            output.capability_flags()
        );
    }

    println!("\nServices:");
    for service in capabilities.services() {
        println!("  {}", service.id());
    }

    println!("\nProtocols: {:?}", capabilities.protocols());
    println!("Loaded modules: {}", capabilities.modules().len());

    if let Some(encoder) = capabilities
        .select_video_encoder()
        .codec("h264")
        .prefer_hardware()
        .best_available()
    {
        println!(
            "Preferred available H.264 encoder: {} (hardware-likely={})",
            encoder.id(),
            encoder.is_likely_hardware_accelerated()
        );
    }
    if let Some(output) = capabilities
        .select_output()
        .protocol("RTMP")
        .video_codec("h264")
        .audio_codec("aac")
        .best_available()
    {
        println!("Compatible RTMP H.264/AAC output: {}", output.id());
    }

    // Discovery descriptors are actionable: callers can start from plugin defaults,
    // change settings generically, and create a typed managed object without hard-coding
    // raw libobs calls. This block simply skips when image-source is not installed.
    if let Some(color_type) = obs.source_type("color_source_v3")? {
        let mut settings = color_type.default_settings_mut()?;
        settings.set_int("width", 640)?.set_int("height", 360)?;
        let source = obs.create_source(&color_type, "discovered-color", Some(settings))?;
        println!(
            "Created {} from discovered type {} (object {:?})",
            source.name(),
            color_type.id(),
            source.object_id()
        );
    }

    Ok(())
}
