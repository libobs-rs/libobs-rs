#![cfg(all(target_family = "windows", feature = "window-list"))]

mod common;

use std::{
    cmp,
    io::{stdout, Write},
    path::PathBuf,
    process::Command,
    time::Duration,
};

use crate::common::{assert_not_black, find_notepad, initialize_obs};
use libobs_simple::sources::windows::{ObsWindowCaptureMethod, WindowCaptureSourceBuilder};
use libobs_window_helper::WindowSearchMode;
use libobs_wrapper::data::{output::ObsOutputTrait, ObsObjectUpdater};
use libobs_wrapper::{sources::ObsSourceBuilder, utils::ObsPath};

#[test]
// For this test to work, notepad must be open
pub fn record() {
    let rec_file = ObsPath::from_relative("window_capture.mp4");
    let path_out: PathBuf = rec_file.clone().into();

    let mut window = find_notepad();
    let mut cmd = None;
    if window.is_none() {
        cmd = Some(Command::new("notepad.exe").spawn().unwrap());
        std::thread::sleep(Duration::from_millis(350));

        window = find_notepad();
    }

    let window = window.expect("Couldn't find notepad window");

    println!("Recording {:?}", window.0.obs_id);

    let (mut context, mut output) = initialize_obs(rec_file);
    let mut scene = context.scene("main", Some(0)).unwrap();

    let source_name = "test_capture";
    let mut source = context
        .source_builder::<WindowCaptureSourceBuilder, _>(source_name)
        .unwrap()
        .set_capture_method(ObsWindowCaptureMethod::MethodAuto)
        .set_window(&window)
        .add_to_scene(&mut scene)
        .unwrap();

    output.start().unwrap();
    println!("Recording started");

    let windows = WindowCaptureSourceBuilder::get_windows(WindowSearchMode::ExcludeMinimized)
        .unwrap()
        .into_iter()
        .filter(|e| {
            e.0.obs_id.to_lowercase().contains("code")
                || e.0.obs_id.to_lowercase().contains("rover")
        })
        .collect::<Vec<_>>();
    for i in 0..cmp::min(5, windows.len()) {
        let w = windows.get(i).unwrap();
        println!("Setting to {:?}", w.0.obs_id);

        source
            .create_updater()
            .unwrap()
            .set_window(w)
            .update()
            .unwrap();

        println!("Recording for {} seconds", i);
        stdout().flush().unwrap();
        std::thread::sleep(Duration::from_secs(1));
    }
    println!("Recording stop");

    output.stop().unwrap();

    if let Some(mut c) = cmd {
        let _ = c.kill();
        c.wait().unwrap();
    }

    assert_not_black(&path_out, 1.0);
}
