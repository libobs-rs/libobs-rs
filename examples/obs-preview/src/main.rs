use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

#[cfg(target_os = "linux")]
use libobs_simple::sources::linux::{
    LinuxGeneralScreenCaptureBuilder, LinuxGeneralScreenCaptureSourceRef,
};
#[cfg(windows)]
use libobs_simple::sources::windows::monitor_capture::MonitorCaptureSource;
#[cfg(target_os = "linux")]
use libobs_wrapper::scenes::ObsSceneItemRef;
#[cfg(windows)]
use libobs_wrapper::scenes::ObsSceneItemRef;
use libobs_wrapper::scenes::SceneItemTrait;
#[cfg(target_os = "linux")]
use libobs_wrapper::utils::NixDisplay;

#[cfg(windows)]
use libobs_simple::sources::ObsObjectUpdater;
#[cfg(windows)]
use libobs_simple::sources::windows::{
    GameCaptureSourceBuilder, MonitorCaptureSourceBuilder, ObsDisplayCaptureMethod,
    ObsGameCaptureMode, WindowSearchMode,
};
use libobs_wrapper::data::video::ObsVideoInfoBuilder;
use libobs_wrapper::display::{
    ObsDisplayCreationData, ObsDisplayRef, ObsWindowHandle, ShowHideTrait, WindowPositionTrait,
};
use libobs_wrapper::sources::ObsSourceBuilder;
use libobs_wrapper::sources::ObsSourceTrait;
use libobs_wrapper::unsafe_send::Sendable;
use libobs_wrapper::{context::ObsContext, utils::StartupInfo};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
#[cfg(target_os = "linux")]
use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId};

struct SignalThreadGuard {
    should_exit: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Drop for SignalThreadGuard {
    fn drop(&mut self) {
        self.should_exit.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let res = handle.join();
            if let Err(e) = res {
                eprintln!("Couldn't join signal thread {e:?}");
            }
        }
    }
}

struct ObsInner {
    context: ObsContext,
    display: ObsDisplayRef,
    #[cfg(windows)]
    _scene_item: ObsSceneItemRef<MonitorCaptureSource>,
    #[cfg(target_os = "linux")]
    _scene_item: ObsSceneItemRef<LinuxGeneralScreenCaptureSourceRef>,
    _guard: SignalThreadGuard,
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
                // Safety: We know that the display handle is valid as long as the event loop is running.
                info.set_nix_display(NixDisplay::Wayland(Sendable(handle.display.as_ptr() as _)))
            };
        }

        let mut context = info.start()?;

        // You could also build an output and start recording/streaming right away here
        //let _output = context.simple_output_builder("recording.mp4")
        //    .build()?;

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
            // You can also set a capture method if you want to
            .set_capture_method(ObsDisplayCaptureMethod::MethodDXGI)
            .set_monitor(
                &MonitorCaptureSourceBuilder::get_monitors().expect("Couldn't get monitors")[0],
            )
            .add_to_scene(&mut scene)?;

        // You could also read a restore token here frm a file
        #[cfg(target_os = "linux")]
        let monitor_item = context
            .source_builder::<LinuxGeneralScreenCaptureBuilder, _>("Monitor capture")?
            .add_to_scene(&mut scene)?;

        monitor_item.fit_source_to_screen()?;

        #[cfg(windows)]
        if let Some(apex) = apex {
            println!(
                "Is used by other instance: {}",
                GameCaptureSourceBuilder::is_window_in_use_by_other_instance(apex.pid)?
            );
            let game_capture_item = context
                .source_builder::<GameCaptureSourceBuilder, _>("Game capture")?
                .set_capture_mode(ObsGameCaptureMode::CaptureSpecificWindow)
                .set_window(apex)
                .add_to_scene(&mut scene)?;

            game_capture_item.fit_source_to_screen()?;
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

        // Example for signals and events with libobs
        let tmp = monitor_item.inner_source().clone();
        let should_exit = Arc::new(AtomicBool::new(false));
        let thread_exit = should_exit.clone();
        let handle = std::thread::spawn(move || {
            let signal_manager = tmp.signals();
            let mut x = signal_manager.on_update().unwrap();

            println!("Listening for updates");
            while !thread_exit.load(Ordering::Relaxed) {
                if x.try_recv().is_ok() {
                    println!("Monitor Source has been updated!");
                }

                std::thread::sleep(Duration::from_millis(100));
            }
        });

        #[cfg_attr(not(target_os = "linux"), allow(unused_unsafe))]
        let display = unsafe {
            // Safety: All references are dropped of the display before the display is dropped.
            context.display(data)?
        };
        Ok(Self {
            context,
            #[cfg_attr(not(target_os = "linux"), allow(unused_unsafe))]
            display,
            _scene_item: monitor_item,
            _guard: SignalThreadGuard {
                should_exit,
                handle: Some(handle),
            },
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

                if let Some(obs) = self.obs.write().unwrap().as_ref() {
                    let _ = obs.display.set_size(display_width, display_height);
                }
            }
            WindowEvent::Moved(_) => {
                if let Some(obs) = self.obs.write().unwrap().as_ref() {
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
                        let mut inner = self.obs.write().unwrap();
                        let inner = inner.as_mut();
                        if let Some(inner) = inner {
                            let monitor_index = self.monitor_index.clone();

                            let scene_item = &mut inner._scene_item;
                            let monitors = MonitorCaptureSourceBuilder::get_monitors().unwrap();

                            let monitor_index = monitor_index
                                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                                % monitors.len();
                            let monitor = &monitors[monitor_index];

                            scene_item
                                .inner_source_mut()
                                .create_updater()
                                .unwrap()
                                .set_monitor(monitor)
                                .update()
                                .unwrap();
                        }
                    }
                    MouseButton::Right => {
                        let inner = self.obs.write().unwrap();
                        let inner = inner.as_ref();
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
                        let mut inner = self.obs.write().unwrap();
                        let inner = inner.as_mut();
                        if let Some(inner) = inner {
                            let display = &mut inner.display;
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
