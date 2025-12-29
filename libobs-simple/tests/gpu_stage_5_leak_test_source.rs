#![cfg(all(target_family = "windows", feature = "window-list"))]

mod common;

use std::{process::Command, time::Duration};

use libobs_simple::sources::windows::{ObsWindowCaptureMethod, WindowCaptureSourceBuilder};
use libobs_wrapper::{sources::ObsSourceBuilder, utils::ObsPath};

use common::initialize_obs;

use crate::common::find_notepad;

/// Stage 5: Initialize OBS, create output with encoders, scene, and add window capture source
#[test]
pub fn test_source() {
    let rec_file = ObsPath::from_relative("leak_test_source.mp4");

    let mut window = find_notepad();
    let mut cmd = None;
    if window.is_none() {
        cmd = Some(Command::new("notepad.exe").spawn().unwrap());
        std::thread::sleep(Duration::from_millis(350));

        window = find_notepad();
    }
    let window = window.expect("Couldn't find notepad window");

    println!("Recording {:?}", window.0.obs_id);

    let (mut context, mut _output) = initialize_obs(rec_file);
    let mut scene = context.scene("main", Some(0)).unwrap();

    let source_name = "test_capture";
    context
        .source_builder::<WindowCaptureSourceBuilder, _>(source_name)
        .unwrap()
        .set_capture_method(ObsWindowCaptureMethod::MethodAuto)
        .set_window(&window)
        .add_to_scene(&mut scene)
        .unwrap();

    let _ = cmd.take().map(|mut c| {
        c.kill().unwrap();
        c.wait().unwrap();
    });
}
