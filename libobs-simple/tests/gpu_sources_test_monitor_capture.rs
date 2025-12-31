#![cfg(target_family = "windows")]

mod common;

use std::{path::PathBuf, time::Duration};

use libobs_simple::sources::windows::{MonitorCaptureSourceBuilder, ObsDisplayCaptureMethod};
use libobs_wrapper::{
    data::{output::ObsOutputTrait, ObsObjectUpdater},
    sources::ObsSourceBuilder,
    utils::ObsPath,
};

use crate::common::{assert_not_black, initialize_obs};

#[test]
pub fn monitor_list_check() {
    MonitorCaptureSourceBuilder::get_monitors().unwrap();
}

/// DXGI is not supported for now
const ENABLE_DXGI_TEST: bool = 1 == 0; // I know this is false, this is just so RustRover doesn't complain about dead code

#[test]
pub fn record() {
    let rec_file = ObsPath::from_relative("monitor_capture.mp4");
    let path_out: PathBuf = rec_file.clone().into();

    let (mut context, mut output) = initialize_obs(rec_file);
    let mut scene = context.scene("main", Some(0)).unwrap();

    let monitor = MonitorCaptureSourceBuilder::get_monitors().unwrap()[0].clone();
    println!("Using monitor {:?}", monitor);

    let mut capture_source = context
        .source_builder::<MonitorCaptureSourceBuilder, _>("monitor_capture")
        .unwrap()
        .set_monitor(&monitor)
        .add_to_scene(&mut scene)
        .unwrap();

    output.start().unwrap();

    println!("Recording started");
    std::thread::sleep(Duration::from_secs(5));
    if ENABLE_DXGI_TEST {
        println!("Testing DXGI capture method");
        capture_source
            .create_updater()
            .unwrap()
            .set_capture_method(ObsDisplayCaptureMethod::MethodDXGI)
            .update()
            .unwrap();

        std::thread::sleep(Duration::from_secs(5));
    }
    println!("Recording stop");

    output.stop().unwrap();

    assert_not_black(&path_out, if ENABLE_DXGI_TEST { 2.0 } else { 1.0 });
}
