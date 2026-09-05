use anyhow::{Context, Result};
use libobs_simple::{
    scene_collection::{RestoreMode, SceneCollectionDocument, SceneCollectionExportPolicy},
    wrapper::{data::ObsDataSetters, graphics::Vec2, scenes::SceneItemTrait, utils::StartupInfo},
};

fn main() -> Result<()> {
    let path = std::env::current_dir()?.join("scene-collection.json");

    let mut obs = StartupInfo::new().start()?;
    let color_type = obs
        .source_type("color_source_v3")?
        .context("the loaded OBS runtime does not provide color_source_v3")?;
    let mut settings = color_type.default_settings_mut()?;
    settings
        .set_int("width", 640)?
        .set_int("height", 360)?
        .set_int("color", 0xFFCC6633_u32 as i64)?;
    let shared = obs.create_source(&color_type, "shared-color", Some(settings))?;

    let mut scene = obs.scene("collection-demo", None)?;
    let first = scene.add(shared.clone())?;
    first.set_position(Vec2::new(40.0, 40.0))?;
    let second = scene.add(shared)?;
    second.set_position(Vec2::new(360.0, 240.0))?;

    let document = SceneCollectionDocument::capture_context(&obs)?;
    let redactions =
        document.save_atomic(&path, SceneCollectionExportPolicy::portable_redacted())?;
    println!(
        "saved {} sources / {} scenes to {} ({} redactions)",
        document.sources.len(),
        document.scenes.len(),
        path.display(),
        redactions.len()
    );

    // libobs is process-global, so shut down this context before demonstrating restoration into a
    // fresh runtime. Shared source UUIDs are restored once and reused by both scene items.
    drop(obs);
    let loaded = SceneCollectionDocument::load(&path)?;
    let mut restored_obs = StartupInfo::new().start()?;
    let restored = loaded.restore(&mut restored_obs, RestoreMode::BestEffort)?;
    println!(
        "restored {} sources / {} scenes; diagnostics: {:?}",
        restored.sources.len(),
        restored.scenes.len(),
        restored.diagnostics
    );

    Ok(())
}
