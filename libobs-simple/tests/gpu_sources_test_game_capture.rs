#![cfg(all(target_family = "windows", feature = "window-list"))]

mod common;

use std::{path::PathBuf, time::Duration};

use libobs_simple::sources::windows::{game_capture::ObsGameCaptureMode, GameCaptureSourceBuilder};
use libobs_wrapper::{
    data::{object::ObsObjectTrait, output::ObsOutputTrait},
    sources::ObsSourceBuilder,
    utils::ObsPath,
};

use crate::common::{assert_not_black, initialize_obs};

#[test]
#[ignore]
pub fn record() {
    let rec_file = ObsPath::from_relative("game_capture.mp4");
    let path_out: PathBuf = rec_file.clone().into();

    let (mut context, mut output) = initialize_obs(rec_file);
    let mut scene = context.scene("main", Some(0)).unwrap();

    let game = GameCaptureSourceBuilder::get_windows(
        libobs_window_helper::WindowSearchMode::ExcludeMinimized,
    )
    .unwrap();
    let game = game
        .iter()
        .find(|e| e.title.is_some() && e.title.as_ref().unwrap().contains("Bloons"))
        .unwrap();

    println!("Using window: {:?}", game);

    let scene_item = context
        .source_builder::<GameCaptureSourceBuilder, _>("game_capture")
        .unwrap()
        .set_capture_mode(ObsGameCaptureMode::Any)
        .add_to_scene(&mut scene)
        .unwrap();

    output.start().unwrap();

    println!("Recording started");
    std::thread::sleep(Duration::from_secs(5));
    println!("Recording stop");

    // This is just so the capture source is not dropped before stopping the output
    let _x = scene_item.inner_source().id();
    output.stop().unwrap();

    assert_not_black(&path_out, 1.0);
}
