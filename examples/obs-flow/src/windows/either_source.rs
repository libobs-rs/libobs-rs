//! This file explains how you could use the ObsEitherSource struct to have multiple
//! source types being represented in one SceneItem.

use libobs_simple::sources::{
    ObsEitherSource, ObsObjectUpdater, ObsSourceBuilder,
    windows::{
        GameCaptureSourceBuilder, ObsHookableSourceTrait, ObsWindowCaptureMethod,
        WindowCaptureSourceBuilder, WindowSearchMode,
    },
};
use libobs_wrapper::{
    context::ObsContext,
    scenes::{ObsSceneRef, SceneItemTrait},
};

pub fn either_source(context: ObsContext, mut scene: ObsSceneRef) -> anyhow::Result<()> {
    let use_window_capture = false;

    let source = if use_window_capture {
        let source = context
            .source_builder::<WindowCaptureSourceBuilder, _>("Either Window Capture")?
            // You can set specific settings on the builder here
            .set_capture_method(ObsWindowCaptureMethod::MethodBitBlt)
            .build()?;

        ObsEitherSource::Left(source)
    } else {
        let game_window =
            GameCaptureSourceBuilder::get_windows(WindowSearchMode::ExcludeMinimized)?;
        // You'd select a window from the list here, for demo purposes we just take the first one

        let source = context
            .source_builder::<GameCaptureSourceBuilder, _>("Either Game Capture")?
            .set_window(&game_window[0])
            .build()?;

        ObsEitherSource::Right(source)
    };

    let mut scene_item = scene.add(source)?;

    // Now you can just use the scene item as usual
    scene_item.fit_source_to_screen()?;

    // Or you can also update the source
    match scene_item.inner_source_mut() {
        ObsEitherSource::Left(window_capture) => {
            window_capture
                .create_updater()?
                // Edit some settings here
                .update()?;
        }
        ObsEitherSource::Right(game_capture) => {
            game_capture
                .create_updater()?
                // Edit some settings here
                .update()?;
        }
    }

    // You an listen if for when a window has been hooked:
    let _receiver = scene_item
        .inner_source_mut()
        .source_specific_signals()
        .on_hooked()?;

    // Wait for hooked event (in a real application you probably want to do this in a separate thread)
    // receiver.recv()?;

    // And we can also remove the scene item again immediately. Existing Rust handles remain
    // valid managed references, but the item is detached from its scene at this point.
    scene.remove_item(&scene_item)?;

    Ok(())
}
