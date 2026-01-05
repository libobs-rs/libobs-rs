#![allow(unknown_lints, require_safety_comments_on_unsafe)]

use std::sync::{Arc, RwLock};

#[cfg(target_os = "linux")]
use libobs_simple::sources::linux::{
    LinuxGeneralScreenCaptureBuilder, LinuxGeneralScreenCaptureSourceRef,
};
#[cfg(target_os = "linux")]
use libobs_simple::sources::ObsEitherSource;
use libobs_wrapper::graphics::Vec2;
#[cfg(target_os = "linux")]
use libobs_wrapper::scenes::ObsSceneItemRef;
use libobs_wrapper::scenes::SceneItemTrait;
#[cfg(target_os = "linux")]
use libobs_wrapper::utils::NixDisplay;

#[cfg(windows)]
use libobs_simple::sources::windows::{
    GameCaptureSourceBuilder, MonitorCaptureSourceBuilder, WindowSearchMode,
};
use libobs_wrapper::data::video::ObsVideoInfoBuilder;
use libobs_wrapper::display::{
    ObsDisplayCreationData, ObsDisplayRef, ObsWindowHandle, WindowPositionTrait,
};
use libobs_wrapper::sources::ObsSourceBuilder;
use libobs_wrapper::unsafe_send::Sendable;
use libobs_wrapper::{context::ObsContext, utils::StartupInfo};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoopBuilder};
#[cfg(target_os = "linux")]
use winit::platform::wayland::EventLoopBuilderExtWayland;
#[cfg(target_os = "windows")]
use winit::platform::windows::EventLoopBuilderExtWindows;
#[cfg(target_os = "linux")]
use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId};

#[derive(Clone)]
struct ObsInner {
    context: ObsContext,
    display: ObsDisplayRef,
    #[cfg(target_os = "linux")]
    _source: ObsSceneItemRef<LinuxGeneralScreenCaptureSourceRef>,
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
        let mut scene = context.scene("Main Scene", Some(0))?;

        #[cfg(windows)]
        let apex = GameCaptureSourceBuilder::get_windows(WindowSearchMode::ExcludeMinimized)?;
        #[cfg(windows)]
        let apex = apex
            .iter()
            .find(|e| e.title.is_some() && e.title.as_ref().unwrap().contains("Apex"));

        #[cfg(windows)]
        let monitor_item = context
            .source_builder::<MonitorCaptureSourceBuilder, _>("Monitor capture")?
            .set_monitor(
                &MonitorCaptureSourceBuilder::get_monitors().expect("Couldn't get monitors")[0],
            )
            .add_to_scene(&mut scene)?;

        #[cfg(target_os = "linux")]
        let monitor_item = {
            use std::path::PathBuf;

            let restore_token_path = std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .join(PathBuf::from("pipewire_restore_token.txt"));
            let restore_token = if restore_token_path.exists() {
                Some(std::fs::read_to_string(&restore_token_path).unwrap())
            } else {
                None
            };

            context
                .source_builder::<LinuxGeneralScreenCaptureBuilder, _>("Monitor capture")
                .unwrap()
                .set_restore_token(&restore_token.unwrap_or_default())
                .add_to_scene(&mut scene)?
        };

        monitor_item.set_source_position(Vec2::new(0.0, 0.0))?;
        monitor_item.set_source_scale(Vec2::new(1.0, 1.0))?;

        #[cfg(windows)]
        if let Some(apex) = apex {
            use libobs_simple::sources::windows::game_capture::ObsGameCaptureMode;

            println!(
                "Is used by other instance: {}",
                GameCaptureSourceBuilder::is_window_in_use_by_other_instance(apex.pid)?
            );
            let item = context
                .source_builder::<GameCaptureSourceBuilder, _>("Game capture")?
                .set_capture_mode(ObsGameCaptureMode::CaptureSpecificWindow)
                .set_window(apex)
                .add_to_scene(&mut scene)?;

            item.set_source_position(Vec2::new(0.0, 0.0))?;
            item.set_source_scale(Vec2::new(1.0, 1.0))?;
        } else {
            println!("No Apex window found for game capture");
        }

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

        #[cfg_attr(not(target_os = "linux"), allow(unused_unsafe))]
        let display = unsafe { context.display(data)? };
        Ok(Self {
            context,
            #[cfg_attr(not(target_os = "linux"), allow(unused_unsafe))]
            display,
            #[cfg(target_os = "linux")]
            _source: monitor_item,
        })
    }
}

struct App {
    window: Arc<RwLock<Option<Sendable<Window>>>>,
    obs: Arc<RwLock<Option<ObsInner>>>,
    start_time: Option<std::time::Instant>,
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

        self.start_time = Some(std::time::Instant::now());
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(start_time) = self.start_time {
            if start_time.elapsed() >= std::time::Duration::from_secs(5) {
                println!("5 seconds elapsed, exiting...");
                event_loop.exit();
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        println!("Stopping output...");
        // The obs context is droppde here before the window / event loop is closed!
        let mut inner = self.obs.write().unwrap().take().unwrap();
        inner.context.remove_display(&inner.display).unwrap();

        #[cfg(target_os = "linux")]
        if let ObsEitherSource::Right(pipewire) = inner._source.inner_source() {
            if let Ok(Some(token)) = pipewire.get_restore_token() {
                let restore_token_path = std::env::current_exe()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join(std::path::PathBuf::from("pipewire_restore_token.txt"));

                std::fs::write(restore_token_path, token).unwrap();
            }
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
            _ => (),
        }
    }
}

#[test]
pub fn test_preview() -> anyhow::Result<()> {
    let event_loop = EventLoopBuilder::default()
        .with_any_thread(true)
        .build()
        .unwrap();

    let mut app = App {
        window: Arc::new(RwLock::new(None)),
        obs: Arc::new(RwLock::new(None)),
        start_time: None,
    };

    event_loop.run_app(&mut app)?;

    Ok(())
}
