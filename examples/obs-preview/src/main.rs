use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, RwLock};

#[cfg(target_os = "linux")]
use libobs_sources::linux::LinuxGeneralScreenCapture;
#[cfg(target_os = "linux")]
use libobs_wrapper::utils::NixDisplay;

#[cfg(windows)]
use libobs_wrapper::utils::traits::ObsUpdatable;

#[cfg(windows)]
use libobs_sources::windows::{
    GameCaptureSourceBuilder, MonitorCaptureSourceBuilder, MonitorCaptureSourceUpdater,
    ObsGameCaptureMode, WindowSearchMode,
};
#[cfg(windows)]
use libobs_sources::ObsObjectUpdater;
use libobs_wrapper::data::video::ObsVideoInfoBuilder;
use libobs_wrapper::display::{
    ObsDisplayCreationData, ObsDisplayRef, ObsWindowHandle, ShowHideTrait, WindowPositionTrait,
};
use libobs_wrapper::encoders::{ObsAudioEncoderType, ObsContextEncoders, ObsVideoEncoderType};
#[cfg(windows)]
use libobs_wrapper::sources::ObsSourceBuilder;
use libobs_wrapper::sources::ObsSourceRef;
use libobs_wrapper::unsafe_send::Sendable;
use libobs_wrapper::utils::{AudioEncoderInfo, OutputInfo};
use libobs_wrapper::{context::ObsContext, utils::StartupInfo};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
#[cfg(target_os = "linux")]
use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId};

#[derive(Clone)]
struct ObsInner {
    context: ObsContext,
    display: ObsDisplayRef,
    #[cfg_attr(not(windows), allow(dead_code))]
    source: ObsSourceRef,
}

impl ObsInner {
    fn new(_event_loop: &ActiveEventLoop, window: &Window) -> anyhow::Result<Self> {
        //TODO This scales the output to 1920x1080, the captured window may be at a different aspect ratio
        let v = ObsVideoInfoBuilder::new()
            .base_width(1920)
            .base_height(1080)
            .output_width(1920)
            .output_height(1080)
            .build();

        #[allow(unused_mut)]
        let mut info = StartupInfo::new().set_video_info(v);

        //NOTE - This is very important if you are running a GUI application, ensure that a nix display is set on linux!
        #[cfg(target_os = "linux")]
        if let RawDisplayHandle::Wayland(handle) = _event_loop.display_handle().unwrap().as_raw() {
            info = unsafe {
                info.set_nix_display(NixDisplay::Wayland(Sendable(handle.display.as_ptr() as _)))
            };
        }

        let mut context = info.start()?;

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

        #[cfg(windows)]
        let apex = GameCaptureSourceBuilder::get_windows(WindowSearchMode::ExcludeMinimized)?;
        #[cfg(windows)]
        let apex = apex
            .iter()
            .find(|e| e.title.is_some() && e.title.as_ref().unwrap().contains("Apex"));

        #[cfg(windows)]
        let monitor_src = context
            .source_builder::<MonitorCaptureSourceBuilder, _>("Monitor capture")?
            .set_monitor(
                &MonitorCaptureSourceBuilder::get_monitors().expect("Couldn't get monitors")[0],
            )
            .add_to_scene(&mut scene)?;

        // You could also read a restore token here frm a file
        #[cfg(target_os = "linux")]
        let monitor_src = LinuxGeneralScreenCapture::auto_detect(
            context.runtime().clone(),
            "Monitor capture",
            None,
        )
        .unwrap()
        .add_to_scene(&mut scene)?;

        scene.set_source_position(&monitor_src, libobs_wrapper::Vec2::new(0.0, 0.0))?;
        scene.set_source_scale(&monitor_src, libobs_wrapper::Vec2::new(1.0, 1.0))?;

        #[cfg(windows)]
        let mut _apex_source = None;
        #[cfg(windows)]
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

        let hwnd = window.window_handle().unwrap().as_raw();

        #[cfg(windows)]
        let obs_handle = {
            let hwnd = if let RawWindowHandle::Win32(hwnd) = hwnd {
                hwnd.hwnd
            } else {
                panic!("Expected a Win32 window handle");
            };

            ObsWindowHandle::new_from_handle(hwnd.get() as *mut _)
        };

        #[cfg(target_os = "linux")]
        let obs_handle = {
            if let RawWindowHandle::Xlib(handle) = hwnd {
                //TODO check if this is actually u32
                ObsWindowHandle::new_from_x11(context.runtime(), handle.window as u32).unwrap()
            } else if let RawWindowHandle::Wayland(handle) = hwnd {
                ObsWindowHandle::new_from_wayland(handle.surface.as_ptr() as *mut _)
            } else {
                panic!("Unsupported window handle for this platform");
            }
        };

        let size = window.inner_size();
        let width = size.width;
        let height = size.height;
        let data: ObsDisplayCreationData =
            ObsDisplayCreationData::new(obs_handle, 0, 0, width, height);

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

        #[cfg_attr(not(target_os = "linux"), allow(unused_unsafe))]
        let display = unsafe { context.display(data)? };
        Ok(Self {
            context,
            #[cfg_attr(not(target_os = "linux"), allow(unused_unsafe))]
            display,
            source: monitor_src,
        })
    }
}

struct App {
    window: Arc<RwLock<Option<Sendable<Window>>>>,
    obs: Arc<RwLock<Option<ObsInner>>>,
    #[cfg_attr(not(windows), allow(dead_code))]
    monitor_index: Arc<AtomicUsize>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(
                Window::default_attributes().with_inner_size(LogicalSize::new(1920 / 2, 1080 / 2)),
            )
            .unwrap();

        self.obs
            .write()
            .unwrap()
            .replace(ObsInner::new(event_loop, &window).unwrap());

        let _ = self.window.write().unwrap().replace(Sendable(window));
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        println!("Stopping output...");
        // The obs context is dropped here before the window / event loop is closed!
        let mut inner = self.obs.write().unwrap().take().unwrap();
        inner.context.remove_display(&inner.display).unwrap();
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

                if let Some(obs) = self.obs.write().unwrap().clone() {
                    let _ = obs.display.set_size(display_width, display_height);
                }
            }
            WindowEvent::Moved(_) => {
                if let Some(obs) = self.obs.write().unwrap().clone() {
                    let _ = obs.display.update_color_space();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if !matches!(state, ElementState::Pressed) {
                    return;
                }

                match button {
                    #[cfg(windows)]
                    // Technically we could also switch monitors on X11, but we would like to keep it simple for now...
                    MouseButton::Left => {
                        let inner = self.obs.write().unwrap().clone();
                        if let Some(inner) = inner {
                            let monitor_index = self.monitor_index.clone();

                            let mut source = inner.source;
                            let monitors = MonitorCaptureSourceBuilder::get_monitors().unwrap();

                            let monitor_index = monitor_index
                                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                                % monitors.len();
                            let monitor = &monitors[monitor_index];

                            source
                                .create_updater::<MonitorCaptureSourceUpdater>()
                                .unwrap()
                                .set_monitor(monitor)
                                .update()
                                .unwrap();
                        }
                    }
                    MouseButton::Right => {
                        let inner = self.obs.write().unwrap().clone();
                        if let Some(inner) = inner {
                            let display = inner.display.clone();
                            let pos = display.get_pos().unwrap();
                            println!("Display position: {:?}", pos);

                            display.set_pos(pos.0 + 10, pos.1 + 10).unwrap();
                            println!(
                                "Moved display to position: {:?}",
                                display.get_pos().unwrap()
                            );
                        }
                    }
                    MouseButton::Middle => {
                        let inner = self.obs.write().unwrap().clone();
                        if let Some(inner) = inner {
                            let mut display = inner.display;
                            let visible = display.is_visible().unwrap();
                            if visible {
                                println!("Hiding display");
                                display.hide().unwrap();
                            } else {
                                println!("Showing display");
                                display.show().unwrap();
                            }
                        }
                    }
                    _ => (),
                };
            }
            _ => (),
        }
    }
}

pub fn main() -> anyhow::Result<()> {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();

    let mut app = App {
        window: Arc::new(RwLock::new(None)),
        obs: Arc::new(RwLock::new(None)),
        monitor_index: Arc::new(AtomicUsize::new(1)),
    };

    event_loop.run_app(&mut app)?;

    println!("Done with mainloop.");
    Ok(())
}
