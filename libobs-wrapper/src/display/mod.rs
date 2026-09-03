//! This module is used to create a `OBS display` which you can use to preview the
//! output of your recording.

mod creation_data;
mod enums;
mod window_manager;

pub use window_manager::{MiscDisplayTrait, ShowHideTrait, WindowPositionTrait};

pub use creation_data::*;
pub use enums::*;
use libobs::obs_video_info;

use crate::unsafe_send::SmartPointerSendable;
use crate::utils::{ObsDropGuard, ObsError};
use crate::{impl_obs_drop, run_with_obs, runtime::ObsRuntime, unsafe_send::Sendable};
use std::mem::MaybeUninit;
use std::{
    ffi::c_void,
    marker::PhantomPinned,
    sync::{atomic::AtomicUsize, Arc, RwLock},
};

static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);
#[derive(Debug, Clone)]
/// You can use the `ObsContext` to create this struct. This struct is stored in the
/// `ObsContext` itself and the display is removed if every instance of this struct is dropped
/// (and you have called `remove_display` on the `ObsContext`).
pub struct ObsDisplayRef {
    id: usize,
    render_state: Arc<DisplayRenderState>,

    /// Keep for window, manager is accessed by render thread as well so Arc and RwLock
    ///
    /// This is mostly used on windows to handle the size and position of the child window.
    #[cfg(windows)]
    #[allow(dead_code)]
    child_window_handler:
        Option<Arc<RwLock<window_manager::windows::WindowsPreviewChildWindowHandler>>>,

    /// Stored so the obs context is not dropped while this is alive
    runtime: ObsRuntime,
    display: SmartPointerSendable<*mut libobs::obs_display_t>,
}

#[derive(Debug)]
pub(super) struct DisplayRenderState {
    pub(super) position: RwLock<(i32, i32)>,
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
/// # Safety
/// Always call this function in the graphics/rendering thread of OBS, never call this function directly!
unsafe extern "C" fn render_display(data: *mut c_void, width: u32, height: u32) {
    let Some(state) = (data as *const DisplayRenderState).as_ref() else {
        log::error!("Display render callback received null userdata");
        return;
    };
    let pos = *state
        .position
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let mut ovi = MaybeUninit::<obs_video_info>::uninit();
    let was_ok = libobs::obs_get_video_info(ovi.as_mut_ptr());
    if !was_ok {
        log::error!("Failed to get video info in display render callback");
        return;
    }

    let ovi = unsafe {
        // Safety: was_ok checked that the video info was properly initialized
        ovi.assume_init()
    };

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

    libobs::obs_render_main_texture_src_color_only();

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
    /// Construct from an HWND supplied by an external windowing toolkit.
    ///
    /// # Safety
    /// `handle` must be a live HWND for the complete lifetime of any OBS display created
    /// from this value, and all toolkit thread-affinity requirements must be respected.
    #[cfg(windows)]
    pub unsafe fn new_from_handle(handle: *mut std::os::raw::c_void) -> Self {
        Self {
            window: Sendable(libobs::gs_window { hwnd: handle }),
            is_wayland: false,
        }
    }

    #[cfg(windows)]
    pub fn get_hwnd(&self) -> windows::Win32::Foundation::HWND {
        windows::Win32::Foundation::HWND(self.window.0.hwnd)
    }

    /// Construct from a Wayland surface supplied by an external windowing toolkit.
    ///
    /// # Safety
    /// `surface` must be a live `wl_surface` belonging to the configured Wayland display
    /// and must outlive any OBS display created from this handle.
    #[cfg(target_os = "linux")]
    pub unsafe fn new_from_wayland(surface: *mut c_void) -> Self {
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
            // Safety: We are just getting a pointer and we are in the runtime
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
    pub(crate) fn new(data: ObsDisplayCreationData, runtime: ObsRuntime) -> Result<Self, ObsError> {
        use std::sync::atomic::Ordering;

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
        let display = run_with_obs!(runtime, (init_data), move || {
            let display_ptr = unsafe {
                // Safety: All pointers are valid because we are keeping them in this scope and because we are cloning init_data into this scope
                libobs::obs_display_create(&init_data.0 .0, background_color)
            };

            if display_ptr.is_null() {
                Err(ObsError::NullPointer(None))
            } else {
                Ok(Sendable(display_ptr))
            }
        })??;

        let initial_pos = if create_child && cfg!(windows) {
            (0, 0)
        } else {
            (x, y)
        };
        let render_state = Arc::new(DisplayRenderState {
            position: RwLock::new(initial_pos),
        });

        let display = SmartPointerSendable::new(
            display.0,
            Arc::new(_ObsDisplayDropGuard {
                display,
                render_state: render_state.clone(),
                runtime: runtime.clone(),
            }),
            runtime.native_registry(),
        );

        #[cfg(windows)]
        if let Some(handler) = &mut child_handler {
            handler.set_display_handle(display.clone());
        }

        let id = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        let instance = Self {
            display: display.clone(),
            id,
            render_state: render_state.clone(),
            runtime: runtime.clone(),

            #[cfg(windows)]
            child_window_handler: child_handler.map(|e| Arc::new(RwLock::new(e))),
        };

        log::trace!("Adding draw callback with display {:?}", instance.display);

        let display_ptr = instance.as_ptr();
        run_with_obs!(runtime, (display_ptr), move || {
            unsafe {
                // Safety: The pointer is valid because we are using a smart pointer
                libobs::obs_display_add_draw_callback(
                    display_ptr.get_ptr(),
                    Some(render_display),
                    Arc::as_ptr(&render_state) as *mut c_void,
                );
            }
        })?;

        Ok(instance)
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn update_color_space(&self) -> Result<(), ObsError> {
        let display_ptr = self.as_ptr();
        run_with_obs!(self.runtime, (display_ptr), move || {
            unsafe {
                // Safety: The pointer is valid because we are using a smart pointer
                libobs::obs_display_update_color_space(display_ptr.get_ptr())
            }
        })
    }

    pub(crate) fn as_ptr(&self) -> SmartPointerSendable<*mut libobs::obs_display_t> {
        self.display.clone()
    }
}

#[derive(Debug)]
struct _ObsDisplayDropGuard {
    display: Sendable<*mut libobs::obs_display_t>,
    render_state: Arc<DisplayRenderState>,
    runtime: ObsRuntime,
}

impl ObsDropGuard for _ObsDisplayDropGuard {}

impl_obs_drop!(
    _ObsDisplayDropGuard,
    (display, render_state),
    move || unsafe {
        // SAFETY: `render_state` remains alive through callback deregistration and display
        // destruction, and the native pointer is leased by this drop guard.
        log::trace!("Removing callback of display {:?}...", display);
        libobs::obs_display_remove_draw_callback(
            display.0,
            Some(render_display),
            Arc::as_ptr(&render_state) as *mut c_void,
        );
        libobs::obs_display_destroy(display.0);
    }
);
