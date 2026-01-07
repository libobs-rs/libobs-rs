#![cfg(target_family = "windows")]

mod common;

use std::time::Duration;

use libobs_simple::{
    output::replay::ObsContextReplayExt, sources::windows::MonitorCaptureSourceBuilder,
};
use libobs_wrapper::{
    data::output::ObsOutputTrait,
    sources::ObsSourceBuilder,
    utils::{ObsPath, StartupInfo},
};

use crate::common::assert_not_black;

#[test]
pub fn record() {
    let mut context = StartupInfo::default().start().unwrap();
    let mut replay_output = context
        .replay_buffer_builder("test-replay-buffer", ObsPath::from_relative("."))
        .max_time_sec(3)
        .build()
        .unwrap();

    let mut scene = context.scene("main", Some(0)).unwrap();

    let monitor = MonitorCaptureSourceBuilder::get_monitors().unwrap()[0].clone();
    println!("Using monitor {:?}", monitor);

    let mut _scene_item = context
        .source_builder::<MonitorCaptureSourceBuilder, _>("monitor_capture")
        .unwrap()
        .set_monitor(&monitor)
        .add_to_scene(&mut scene)
        .unwrap();

    replay_output.start().unwrap();

    println!("Recording started");
    std::thread::sleep(Duration::from_secs(5));
    println!("Recording stop");

    let path_output = replay_output.save_buffer().unwrap();
    replay_output.stop().unwrap();

    assert_not_black(&path_output, 1.0);
}
