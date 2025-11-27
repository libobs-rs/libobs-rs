//! For this display method to work, another preview window has to be created in order to create a swapchain
//! This is because the main window renderer is already handled by other processes

mod creation_data;
mod enums;
mod window_manager;

pub use window_manager::{MiscDisplayTrait, ShowHideTrait, WindowPositionTrait};

pub use creation_data::*;
pub use enums::*;
use libobs::obs_video_info;

use crate::utils::ObsError;
use crate::{impl_obs_drop, run_with_obs, runtime::ObsRuntime, unsafe_send::Sendable};
use lazy_static::lazy_static;
use libobs::obs_render_main_texture_src_color_only;
use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::{
    ffi::c_void,
    marker::PhantomPinned,
    sync::{atomic::AtomicUsize, Arc, RwLock},
};

static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);
#[derive(Debug, Clone)]
//TODO: This has to be checked again, I'm unsure with pinning and draw callbacks from OBS
///
/// This is a wrapper around the obs_display struct and contains direct memory references.
/// You should ALWAYS use the context to get to this struct, and as said NEVER store it.
pub struct ObsDisplayRef {
    display: Sendable<*mut libobs::obs_display_t>,
    id: usize,

    _guard: Arc<_ObsDisplayDropGuard>,
    _pos_remove_guard: Arc<PosRemoveGuard>,

    /// Keep for window, manager is accessed by render thread as well so Arc and RwLock
    ///
    /// This is mostly used on windows to handle the size and position of the child window.
    #[cfg(windows)]
    #[allow(dead_code)]
    child_window_handler:
        Option<Arc<RwLock<window_manager::windows::WindowsPreviewChildWindowHandler>>>,

    /// Stored so the obs context is not dropped while this is alive
    pub(crate) runtime: ObsRuntime,
}

lazy_static! {
    pub(super) static ref DISPLAY_POSITIONS: Arc<RwLock<HashMap<usize, (i32, i32)>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

#[derive(Debug)]
struct PosRemoveGuard {
    id: usize,
}

impl Drop for PosRemoveGuard {
    fn drop(&mut self) {
        let mut map = DISPLAY_POSITIONS.write().unwrap();
        map.remove(&self.id);
    }
}

unsafe extern "C" fn render_display(data: *mut c_void, width: u32, height: u32) {
    let id = data as usize;
    let pos = DISPLAY_POSITIONS
        .read()
        .unwrap()
        .get(&id)
        .cloned()
        .unwrap_or((0, 0));

    let mut ovi = MaybeUninit::<obs_video_info>::uninit();
    libobs::obs_get_video_info(ovi.as_mut_ptr());

    let ovi = unsafe { ovi.assume_init() };

    libobs::gs_viewport_push();
    libobs::gs_projection_push();

    libobs::gs_ortho(
        0.0f32,
        ovi.base_width as f32,
        0.0f32,
        ovi.base_height as f32,
        -100.0f32,
        100.0f32,
    );
    libobs::gs_set_viewport(pos.0, pos.1, width as i32, height as i32);
    //draw_backdrop(&s.buffers, ovi.base_width as f32, ovi.base_height as f32);

    obs_render_main_texture_src_color_only();

    libobs::gs_projection_pop();
    libobs::gs_viewport_pop();
}

pub struct LockedPosition {
    pub x: i32,
    pub y: i32,
    /// This must not be moved in memory as the draw callback is a raw pointer to this struct
    _fixed_in_heap: PhantomPinned,
}

#[derive(Clone, Debug)]
pub struct ObsWindowHandle {
    pub(crate) window: Sendable<libobs::gs_window>,
    #[allow(dead_code)]
    pub(crate) is_wayland: bool,
}

impl ObsWindowHandle {
    #[cfg(windows)]
    pub fn new_from_handle(handle: *mut std::os::raw::c_void) -> Self {
        Self {
            window: Sendable(libobs::gs_window { hwnd: handle }),
            is_wayland: false,
        }
    }

    #[cfg(windows)]
    pub fn get_hwnd(&self) -> windows::Win32::Foundation::HWND {
        windows::Win32::Foundation::HWND(self.window.0.hwnd)
    }

    #[cfg(target_os = "linux")]
    pub fn new_from_wayland(surface: *mut c_void) -> Self {
        Self {
            window: Sendable(libobs::gs_window {
                display: surface,
                id: 0,
            }),
            is_wayland: true,
        }
    }

    #[cfg(target_os = "linux")]
    pub fn new_from_x11(runtime: &ObsRuntime, id: u32) -> Result<Self, ObsError> {
        let runtime = runtime.clone();
        let display = run_with_obs!(runtime, (), move || unsafe {
            Sendable(libobs::obs_get_nix_platform_display())
        })?;

        Ok(Self {
            window: Sendable(libobs::gs_window {
                display: display.0,
                id,
            }),
            is_wayland: false,
        })
    }
}

impl ObsDisplayRef {
    /// Call initialize to ObsDisplay#create the display
    pub(crate) fn new(data: ObsDisplayCreationData, runtime: ObsRuntime) -> anyhow::Result<Self> {
        use std::sync::atomic::Ordering;

        use anyhow::bail;
        use creation_data::ObsDisplayCreationData;

        use crate::run_with_obs;

        let ObsDisplayCreationData {
            x,
            y,
            background_color,
            create_child,
            #[cfg(windows)]
            height,
            #[cfg(windows)]
            width,
            #[cfg(windows)]
            window_handle,
            ..
        } = data.clone();

        #[cfg(windows)]
        let mut child_handler = if create_child {
            Some(
                window_manager::windows::WindowsPreviewChildWindowHandler::new_child(
                    window_handle.clone(),
                    x,
                    y,
                    width,
                    height,
                )?,
            )
        } else {
            None
        };

        #[cfg(windows)]
        let init_data = Sendable(data.build(child_handler.as_ref().map(|e| e.get_window_handle())));

        #[cfg(not(windows))]
        let init_data = Sendable(data.build(None));

        log::trace!("Creating obs display...");
        let display = run_with_obs!(runtime, (init_data), move || unsafe {
            Sendable(libobs::obs_display_create(&init_data.0, background_color))
        })?;

        if display.0.is_null() {
            bail!("OBS failed to create display");
        }

        #[cfg(windows)]
        if let Some(handler) = &mut child_handler {
            handler.set_display_handle(display.clone());
        }

        let initial_pos = if create_child && cfg!(windows) {
            (0, 0)
        } else {
            (x, y)
        };

        let id = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        DISPLAY_POSITIONS
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?
            .insert(id, initial_pos);

        let instance = Self {
            display: display.clone(),
            _guard: Arc::new(_ObsDisplayDropGuard {
                display,
                runtime: runtime.clone(),
            }),
            id,
            runtime: runtime.clone(),
            _pos_remove_guard: Arc::new(PosRemoveGuard { id }),

            #[cfg(windows)]
            child_window_handler: child_handler.map(|e| Arc::new(RwLock::new(e))),
        };

        log::trace!("Adding draw callback with display {:?}", instance.display);

        let display_ptr = instance.display.clone();
        run_with_obs!(runtime, (display_ptr), move || unsafe {
            libobs::obs_display_add_draw_callback(
                display_ptr,
                Some(render_display),
                id as *mut c_void,
            );
        })?;

        Ok(instance)
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn update_color_space(&self) -> Result<(), ObsError> {
        let display_ptr = self.display.clone();
        run_with_obs!(self.runtime, (display_ptr), move || unsafe {
            libobs::obs_display_update_color_space(display_ptr)
        })
    }
}

#[derive(Debug)]
struct _ObsDisplayDropGuard {
    display: Sendable<*mut libobs::obs_display_t>,
    pub(crate) runtime: ObsRuntime,
}

impl_obs_drop!(_ObsDisplayDropGuard, (display), move || unsafe {
    log::trace!("Removing callback of display {:?}...", display);
    libobs::obs_display_remove_draw_callback(display, Some(render_display), std::ptr::null_mut());

    libobs::obs_display_destroy(display);
});
