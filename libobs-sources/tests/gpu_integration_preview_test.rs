#![cfg(target_family = "windows")]

use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use libobs_sources::windows::MonitorCaptureSourceBuilder;
use libobs_wrapper::data::video::ObsVideoInfoBuilder;
use libobs_wrapper::display::{
    ObsDisplayCreationData, ObsDisplayRef, ObsWindowHandle, WindowPositionTrait,
};
use libobs_wrapper::encoders::{ObsAudioEncoderType, ObsContextEncoders, ObsVideoEncoderType};
use libobs_wrapper::sources::ObsSourceRef;
use libobs_wrapper::unsafe_send::Sendable;
use libobs_wrapper::utils::{AudioEncoderInfo, OutputInfo};
use libobs_wrapper::{context::ObsContext, sources::ObsSourceBuilder, utils::StartupInfo};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::platform::windows::EventLoopBuilderExtWindows;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId};

struct App {
    window: Arc<RwLock<Option<Sendable<Window>>>>,
    display: Arc<RwLock<Option<Pin<Box<ObsDisplayRef>>>>>,
    context: Arc<RwLock<ObsContext>>,
    _source_ref: Arc<RwLock<ObsSourceRef>>,
    initialized_at: Instant,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(
                Window::default_attributes().with_inner_size(PhysicalSize::new(1920 / 2, 1080 / 2)),
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
        self.initialized_at = Instant::now();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let elapsed = self.initialized_at.elapsed();
        if elapsed.as_secs() >= 1 {
            if let Some(display) = self.display.write().unwrap().clone() {
                let ctx = self.context.clone();

                ctx.write().unwrap().remove_display(&display).unwrap();
            }

            event_loop.exit();
        }
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
            _ => (),
        }
    }
}

#[test]
fn test_preview() {
    env_logger::init();

    let v = ObsVideoInfoBuilder::new()
        .base_width(1920)
        .base_height(1080)
        .output_width(1920)
        .output_height(1080)
        .build();
    let info = StartupInfo::new().set_video_info(v);

    let mut context = ObsContext::new(info).unwrap();

    let output_info = OutputInfo::new("ffmpeg_muxer", "output", None, None);
    let mut output = context.output(output_info).unwrap();

    // Register the video encoder
    let mut video_settings = context.data().unwrap();
    video_settings
        .bulk_update()
        .set_int("bf", 0)
        .set_bool("psycho_aq", true)
        .set_bool("lookahead", true)
        .set_string("profile", "high")
        .set_string("preset", "fast")
        .set_string("rate_control", "cbr")
        .set_int("bitrate", 10000)
        .update()
        .unwrap();

    let encoders = context.available_video_encoders().unwrap();

    let mut encoder = encoders
        .into_iter()
        .find(|e| {
            e.get_encoder_id() == &ObsVideoEncoderType::H264_TEXTURE_AMF
                || e.get_encoder_id() == &ObsVideoEncoderType::AV1_TEXTURE_AMF
                || e.get_encoder_id() == &ObsVideoEncoderType::OBS_NVENC_H264_TEX
        })
        .unwrap();

    encoder.set_settings(video_settings);

    println!("Using encoder {:?}", encoder.get_encoder_id());
    encoder.set_to_output(&mut output, "video_encoder").unwrap();

    // Register the audio encoder
    let mut audio_settings = context.data().unwrap();
    audio_settings.set_int("bitrate", 160).unwrap();

    let audio_info = AudioEncoderInfo::new(
        ObsAudioEncoderType::FFMPEG_AAC,
        "audio_encoder",
        Some(audio_settings),
        None,
    );

    output.create_and_set_audio_encoder(audio_info, 0).unwrap();

    let mut scene = context.scene("Main Scene").unwrap();

    let source_ref = context
        .source_builder::<MonitorCaptureSourceBuilder, _>("Monitor capture")
        .unwrap()
        .set_monitor(&MonitorCaptureSourceBuilder::get_monitors().unwrap()[0])
        .add_to_scene(&mut scene)
        .unwrap();

    scene.set_to_channel(0).unwrap();

    let event_loop = winit::event_loop::EventLoop::builder()
        .with_any_thread(true)
        .build()
        .expect("Failed to create event loop");
    let mut app = App {
        window: Arc::new(RwLock::new(None)),
        display: Arc::new(RwLock::new(None)),
        context: Arc::new(RwLock::new(context)),
        _source_ref: Arc::new(RwLock::new(source_ref)),
        initialized_at: Instant::now(),
    };

    event_loop.run_app(&mut app).unwrap();
}
