use anyhow::Result;
use libobs_wrapper::{data::ObsDataGetters, utils::StartupInfo};

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
            "  {} (video={:?}, audio={:?})",
            output.id(),
            output.video_codecs(),
            output.audio_codecs()
        );
    }

    println!("\nServices:");
    for service in capabilities.services() {
        println!("  {}", service.id());
    }

    println!("\nProtocols: {:?}", capabilities.protocols());
    println!("Loaded modules: {}", capabilities.modules().len());

    Ok(())
}
