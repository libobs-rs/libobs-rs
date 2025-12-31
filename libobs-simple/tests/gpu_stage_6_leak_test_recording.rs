#![cfg(all(target_family = "windows", feature = "window-list"))]

mod common;

use std::{path::PathBuf, process::Command, time::Duration};

use libobs_simple::{
    output::replay::ObsContextReplayExt,
    sources::windows::{ObsWindowCaptureMethod, WindowCaptureSourceBuilder},
};
use libobs_wrapper::{data::output::ObsOutputTrait, sources::ObsSourceBuilder, utils::ObsPath};

use common::{assert_not_black, find_notepad, initialize_obs};

/// Stage 6: Initialize OBS, create output with encoders, scene, add source, and record
#[test]
pub fn test_recording() {
    let rec_file = ObsPath::from_relative("leak_test_recording.mp4");
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

    let mut replay_buffer = context
        .replay_buffer_builder("replay_output", ObsPath::from_relative("."))
        .build()
        .unwrap();

    let mut scene = context.scene("main", Some(0)).unwrap();

    let source_name = "test_capture";
    context
        .source_builder::<WindowCaptureSourceBuilder, _>(source_name)
        .unwrap()
        .set_capture_method(ObsWindowCaptureMethod::MethodAuto)
        .set_window(&window)
        .add_to_scene(&mut scene)
        .unwrap();

    // Start recording
    output.start().unwrap();
    replay_buffer.start().unwrap();
    println!("Recording started");

    // Record for 3 seconds
    std::thread::sleep(Duration::from_secs(3));
    replay_buffer.save_buffer().unwrap();

    println!("Recording stop");
    output.stop().unwrap();
    replay_buffer.stop().unwrap();

    // Clean up notepad process if we started it
    cmd.take()
        .map(|mut c| {
            c.kill().unwrap();
            c.wait().unwrap();
        })
        .unwrap_or_default();

    // Verify the recording isn't black
    assert_not_black(&path_out, 1.0);
}
