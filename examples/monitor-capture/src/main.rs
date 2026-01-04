use libobs_simple::output::simple::ObsContextSimpleExt;
#[cfg(target_os = "linux")]
use libobs_simple::sources::ObsSourceBuilder;
use libobs_wrapper::data::output::ObsOutputTrait;
#[cfg(target_os = "linux")]
use libobs_wrapper::logger::ObsLogger;
#[cfg(windows)]
use libobs_wrapper::scenes::SceneItemTrait;
use libobs_wrapper::utils::StartupInfo;
use libobs_wrapper::{context::ObsContext, utils::ObsPath};

#[cfg(target_os = "linux")]
use libobs_simple::sources::linux::LinuxGeneralScreenCaptureBuilder;
#[cfg(windows)]
use libobs_simple::sources::windows::MonitorCaptureSourceBuilder;
#[cfg(windows)]
use libobs_wrapper::data::ObsObjectUpdater;
#[cfg(windows)]
use libobs_wrapper::sources::ObsSourceBuilder;
#[cfg(target_os = "linux")]
use std::io::{self, Write};

#[cfg(target_os = "linux")]
#[derive(Debug)]
pub struct NoLogger {}
#[cfg(target_os = "linux")]
impl ObsLogger for NoLogger {
    fn log(&mut self, _level: libobs_wrapper::enums::ObsLogLevel, _msg: String) {}
}

fn main() -> anyhow::Result<()> {
    // Start the OBS context
    let startup_info = StartupInfo::default();

    // FIXME This is not recommended in production. This is just for the purpose of this example.
    #[cfg(target_os = "linux")]
    let startup_info = startup_info.set_logger(Box::new(NoLogger {}));

    let mut context = ObsContext::new(startup_info)?;

    let mut scene = context.scene("main", Some(0))?;

    // Platform-specific screen/monitor capture setup
    #[cfg(windows)]
    let monitors = MonitorCaptureSourceBuilder::get_monitors()?;

    #[cfg(windows)]
    let mut monitor_item = context
        .source_builder::<MonitorCaptureSourceBuilder, _>("Monitor Capture")?
        .set_monitor(&monitors[0])
        .set_capture_method(libobs_simple::sources::windows::ObsDisplayCaptureMethod::MethodDXGI)
        .add_to_scene(&mut scene)?;

    #[cfg(windows)]
    monitor_item.fit_source_to_screen()?;

    #[cfg(target_os = "linux")]
    {
        // You could also read a restore token here from a file

        use libobs_wrapper::data::ObsObjectBuilder;
        let screen_capture =
            LinuxGeneralScreenCaptureBuilder::new("Screen Capture", context.runtime().clone())
                .map_err(|e| anyhow::anyhow!("Failed to create screen capture: {}", e))?;

        println!(
            "Using {:?} capture method",
            screen_capture.capture_type_name()
        );

        screen_capture.add_to_scene(&mut scene)?;
    }

    // Set up output to ./recording.mp4
    let mut output = context
        .simple_output_builder("monitor-capture-output", ObsPath::new("record.mp4"))
        .build()?;

    output.start()?;

    #[cfg(windows)]
    {
        use std::thread;
        use std::time::Duration;

        println!("Recording for 5 seconds and switching monitor...");
        thread::sleep(Duration::from_secs(5));

        // Switching monitor
        monitor_item
            .inner_source_mut()
            .create_updater()?
            .set_monitor(&monitors[1 % monitors.len()])
            .update()?;

        println!("Recording for another 5 seconds...");
        thread::sleep(Duration::from_secs(5));
    }

    #[cfg(target_os = "linux")]
    {
        print!("Recording... press Enter to stop.");
        io::stdout().flush()?;

        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        eprintln!("This example is only supported on Windows and Linux.");
        return Ok(());
    }

    // Stop recording
    output.stop()?;
    println!("Recording saved to recording.mp4");

    Ok(())
}
