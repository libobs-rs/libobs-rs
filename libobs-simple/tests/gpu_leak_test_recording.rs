#![cfg(all(target_family = "windows", feature = "window-list"))]

mod common;

use std::{path::PathBuf, process::Command, time::Duration};

use libobs_simple::sources::windows::{ObsWindowCaptureMethod, WindowCaptureSourceBuilder};
use libobs_wrapper::{
    data::output::ReplayBufferOutput,
    sources::ObsSourceBuilder,
    utils::{ObjectInfo, ObsPath, ObsString},
};

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
    let ae = {
        let ae = output.audio_encoders().read().unwrap();
        ae.as_ref().unwrap().clone()
    };

    let mut replay_buffer_settings = context.data().unwrap();
    replay_buffer_settings
        .bulk_update()
        .set_string("directory", ObsPath::from_relative("."))
        .set_string("format", "%CCYY-%MM-%DD %hh-%mm-%ss")
        .set_string("extension", "mp4")
        .set_int("max_time_sec", 15)
        .set_int("max_size_mb", 500)
        .update()
        .unwrap();

    let mut replay_buffer = context
        .output(ObjectInfo {
            id: ObsString::new("replay_buffer"),
            name: ObsString::new("replay_buffer_output"),
            hotkey_data: None,
            settings: Some(replay_buffer_settings),
        })
        .unwrap();

    replay_buffer
        .set_video_encoder(output.get_current_video_encoder().unwrap().unwrap())
        .unwrap();

    replay_buffer.set_audio_encoder(ae.clone(), 0).unwrap();

    let mut scene = context.scene("main").unwrap();
    scene.set_to_channel(0).unwrap();

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
