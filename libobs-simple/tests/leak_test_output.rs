use env_logger::Env;
use libobs_wrapper::{
    context::ObsContext,
    data::ObsDataSetters,
    utils::{OutputInfo, StartupInfo},
};

/// Stage 2: Initialize OBS and create basic output without encoders
#[test]
pub fn test_output() {
    let _ = env_logger::Builder::from_env(Env::default().default_filter_or("debug"))
        .is_test(true)
        .try_init();

    let startup_info = StartupInfo::default();
    let mut context = ObsContext::new(startup_info).unwrap();

    // Set up output settings
    let mut output_settings = context.data().unwrap();
    let rec_file = "leak_test_output.mp4";
    output_settings.set_string("path", rec_file).unwrap();

    // Create basic output without encoders
    let output_name = "output";
    let output_info = OutputInfo::new("ffmpeg_muxer", output_name, Some(output_settings), None);
    let _output = context.output(output_info).unwrap();

    // Output and context will be dropped here, testing for memory leaks
}
