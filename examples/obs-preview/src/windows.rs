use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, RwLock};

use libobs_sources::windows::{
    GameCaptureSourceBuilder, MonitorCaptureSourceBuilder, MonitorCaptureSourceUpdater,
};
use libobs_sources::windows::{ObsGameCaptureMode, WindowSearchMode};
use libobs_sources::ObsObjectUpdater;
use libobs_wrapper::data::video::ObsVideoInfoBuilder;
use libobs_wrapper::display::{
    ObsDisplayCreationData, ObsDisplayRef, ObsWindowHandle, WindowPositionTrait,
};
use libobs_wrapper::encoders::{ObsAudioEncoderType, ObsContextEncoders, ObsVideoEncoderType};
use libobs_wrapper::sources::ObsSourceRef;
use libobs_wrapper::unsafe_send::Sendable;
use libobs_wrapper::utils::traits::ObsUpdatable;
use libobs_wrapper::utils::{AudioEncoderInfo, OutputInfo};
use libobs_wrapper::{context::ObsContext, sources::ObsSourceBuilder, utils::StartupInfo};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId};

struct App {
    window: Arc<RwLock<Option<Sendable<Window>>>>,
    display: Arc<RwLock<Option<Pin<Box<ObsDisplayRef>>>>>,
    context: Arc<RwLock<ObsContext>>,
    monitor_index: Arc<AtomicUsize>,
    source_ref: Arc<RwLock<ObsSourceRef>>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(
                Window::default_attributes().with_inner_size(LogicalSize::new(1920 / 2, 1080 / 2)),
            )
            .unwrap();

        let size = window.inner_size();
        let width = size.width;
        let height = size.height;

        let hwnd = window.window_handle().unwrap().as_raw();
        let hwnd = if let RawWindowHandle::Win32(hwnd) = hwnd {
            hwnd.hwnd
        } else {
            panic!("Expected a Win32 window handle");
        };

        println!("Created window with hwnd size {width} {height}: {:?}", hwnd);
        let w = self.window.clone();
        let d_rw = self.display.clone();
        let ctx = self.context.clone();
        let data = ObsDisplayCreationData::new(
            ObsWindowHandle::new_from_handle(hwnd.get() as *mut _),
            0,
            0,
            width,
            height,
        );

        let display = ctx.write().unwrap().display(data).unwrap();

        w.write().unwrap().replace(Sendable(window));
        d_rw.write().unwrap().replace(display);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let window = self.window.read().unwrap();
        if window.is_none() {
            return;
        }

        let window = window.as_ref().unwrap();
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                println!("Stopping output...");
                if let Some(display) = self.display.write().unwrap().clone() {
                    let ctx = self.context.clone();

                    ctx.write().unwrap().remove_display(&display).unwrap();
                }

                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                window.0.request_redraw();
            }
            WindowEvent::Resized(size) => {
                let window_width = size.width;
                let window_height = size.height;
                let target_aspect_ratio = 16.0 / 9.0;

                // Calculate dimensions that fit in the window while maintaining aspect ratio
                let (display_width, display_height) =
                    if window_width as f32 / window_height as f32 > target_aspect_ratio {
                        // Window is wider than target ratio, height is limiting factor
                        let height = window_height;
                        let width = (height as f32 * target_aspect_ratio) as u32;
                        (width, height)
                    } else {
                        // Window is taller than target ratio, width is limiting factor
                        let width = window_width;
                        let height = (width as f32 / target_aspect_ratio) as u32;
                        (width, height)
                    };

                if let Some(display) = self.display.write().unwrap().clone() {
                    let _ = display.set_size(display_width, display_height);
                }
            }
            WindowEvent::Moved(_) => {
                if let Some(display) = self.display.write().unwrap().clone() {
                    let _ = display.update_color_space();
                }
            }
            //TODO If the display settings change, call update_color_space as well
            WindowEvent::MouseInput { state, .. } => {
                if !matches!(state, ElementState::Pressed) {
                    return;
                }
                let tmp = self.source_ref.clone();
                let monitor_index = self.monitor_index.clone();

                let mut source = tmp.write().unwrap().clone();
                let monitors = MonitorCaptureSourceBuilder::get_monitors().unwrap();

                let monitor_index = monitor_index.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    % monitors.len();
                let monitor = &monitors[monitor_index];

                source
                    .create_updater::<MonitorCaptureSourceUpdater>()
                    .unwrap()
                    .set_monitor(monitor)
                    .update()
                    .unwrap();
            }
            _ => (),
        }
    }
}

pub fn main() -> anyhow::Result<()> {
    env_logger::init();

    //TODO This scales the output to 1920x1080, the captured window may be at a different aspect ratio
    let v = ObsVideoInfoBuilder::new()
        .base_width(1920)
        .base_height(1080)
        .output_width(1920)
        .output_height(1080)
        .build();
    let info = StartupInfo::new().set_video_info(v);

    let mut context = ObsContext::new(info)?;

    // Set up output to ./recording.mp4
    let mut output_settings = context.data()?;
    output_settings.set_string("path", "recording.mp4")?;

    let output_info = OutputInfo::new("ffmpeg_muxer", "output", Some(output_settings), None);
    let mut output = context.output(output_info)?;

    // Register the video encoder
    let mut video_settings = context.data()?;
    video_settings
        .bulk_update()
        .set_int("bf", 0)
        .set_bool("psycho_aq", true)
        .set_bool("lookahead", true)
        .set_string("profile", "high")
        .set_string("preset", "fast")
        .set_string("rate_control", "cbr")
        .set_int("bitrate", 10000)
        .update()?;

    let encoders = context.available_video_encoders()?;

    let mut encoder = encoders
        .into_iter()
        .find(|e| {
            e.get_encoder_id() == &ObsVideoEncoderType::OBS_NVENC_H264_TEX
                || e.get_encoder_id() == &ObsVideoEncoderType::AV1_TEXTURE_AMF
                || e.get_encoder_id() == &ObsVideoEncoderType::OBS_X264
        })
        .unwrap();

    encoder.set_settings(video_settings);

    println!("Using encoder {:?}", encoder.get_encoder_id());
    encoder.set_to_output(&mut output, "video_encoder")?;

    // Register the audio encoder
    let mut audio_settings = context.data()?;
    audio_settings.set_int("bitrate", 160)?;

    let audio_info = AudioEncoderInfo::new(
        ObsAudioEncoderType::FFMPEG_AAC,
        "audio_encoder",
        Some(audio_settings),
        None,
    );

    output.create_and_set_audio_encoder(audio_info, 0)?;

    let mut scene = context.scene("Main Scene")?;

    let apex = GameCaptureSourceBuilder::get_windows(WindowSearchMode::ExcludeMinimized)?;
    let apex = apex
        .iter()
        .find(|e| e.title.is_some() && e.title.as_ref().unwrap().contains("Apex"));

    let monitor_src = context
        .source_builder::<MonitorCaptureSourceBuilder, _>("Monitor capture")?
        .set_monitor(
            &MonitorCaptureSourceBuilder::get_monitors().expect("Couldn't get monitors")[0],
        )
        .add_to_scene(&mut scene)?;
    scene.set_source_position(&monitor_src, libobs_wrapper::Vec2::new(0.0, 0.0))?;
    scene.set_source_scale(&monitor_src, libobs_wrapper::Vec2::new(1.0, 1.0))?;

    let mut _apex_source = None;
    if let Some(apex) = apex {
        println!(
            "Is used by other instance: {}",
            GameCaptureSourceBuilder::is_window_in_use_by_other_instance(apex.pid)?
        );
        let source = context
            .source_builder::<GameCaptureSourceBuilder, _>("Game capture")?
            .set_capture_mode(ObsGameCaptureMode::CaptureSpecificWindow)
            .set_window(apex)
            .add_to_scene(&mut scene)?;

        scene.set_source_position(&source, libobs_wrapper::Vec2::new(0.0, 0.0))?;
        scene.set_source_scale(&source, libobs_wrapper::Vec2::new(1.0, 1.0))?;
        _apex_source = Some(source);
    } else {
        println!("No Apex window found for game capture");
    }

    scene.set_to_channel(0)?;

    // Example for signals and events with libobs
    let tmp = monitor_src.clone();
    std::thread::spawn(move || {
        let signal_manager = tmp.signal_manager();
        let mut x = signal_manager.on_update().unwrap();

        println!("Listening for updates");
        while x.blocking_recv().is_ok() {
            println!("Monitor Source has been updated!");
        }
    });

    let event_loop = EventLoop::new().unwrap();
    let mut app = App {
        window: Arc::new(RwLock::new(None)),
        display: Arc::new(RwLock::new(None)),
        context: Arc::new(RwLock::new(context)),
        monitor_index: Arc::new(AtomicUsize::new(1)),
        source_ref: Arc::new(RwLock::new(monitor_src)),
    };

    event_loop.run_app(&mut app).unwrap();

    println!("Done with mainloop.");
    Ok(())
}
