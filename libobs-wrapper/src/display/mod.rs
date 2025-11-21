//! For this display method to work, another preview window has to be created in order to create a swapchain
//! This is because the main window renderer is already handled by other processes

mod creation_data;
mod enums;
//TODO
mod window_manager;

pub use window_manager::{MiscDisplayTrait, ShowHideTrait, WindowPositionTrait};

pub use creation_data::*;
pub use enums::*;
use libobs::obs_video_info;

use crate::utils::ObsError;
use crate::{impl_obs_drop, run_with_obs, runtime::ObsRuntime, unsafe_send::Sendable};
use libobs::obs_render_main_texture_src_color_only;
use std::mem::MaybeUninit;
use std::{
    ffi::c_void,
    marker::PhantomPinned,
    sync::{atomic::AtomicUsize, Arc, RwLock},
};

static ID_COUNTER: AtomicUsize = AtomicUsize::new(1);
#[derive(Debug, Clone)]
//TODO: This has to be checked again, I'm unsure with pinning and draw callbacks from OBS
/// # NEVER STORE THIS REF DIRECTLY!!
/// ALWAYS store the std::pin::Pin<Box<ObsDisplayRef>> that is created from ObsContext::display!
///
/// This is a wrapper around the obs_display struct and contains direct memory references.
/// You should ALWAYS use the context to get to this struct, and as said NEVER store it.
pub struct ObsDisplayRef {
    display: Sendable<*mut libobs::obs_display_t>,
    id: usize,

    _guard: Arc<_ObsDisplayDropGuard>,

    // Keep for window, manager is accessed by render thread as well so Arc and RwLock
    #[cfg(windows)]
    #[allow(dead_code)]
    manager: Arc<RwLock<window_manager::windows::WindowDisplayWindowManager>>,

    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    manager: Arc<RwLock<window_manager::linux::DisplayWindowManager>>,
    /// This must not be moved in memory as the draw callback is a raw pointer to this struct
    _fixed_in_heap: PhantomPinned,

    /// Stored so the obs context is not dropped while this is alive
    pub(crate) runtime: ObsRuntime,
}

unsafe extern "C" fn render_display(_data: *mut c_void, width: u32, height: u32) {
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
    libobs::gs_set_viewport(0, 0, width as i32, height as i32);
    //draw_backdrop(&s.buffers, ovi.base_width as f32, ovi.base_height as f32);

    obs_render_main_texture_src_color_only();

    libobs::gs_projection_pop();
    libobs::gs_viewport_pop();
}

#[derive(Clone, Debug)]
pub struct ObsWindowHandle(Sendable<libobs::gs_window>);

impl ObsWindowHandle {
    #[cfg(windows)]
    pub fn new_from_handle(handle: *mut std::os::raw::c_void) -> Self {
        Self(Sendable(libobs::gs_window { hwnd: handle }))
    }

    #[cfg(windows)]
    pub fn get_hwnd(&self) -> windows::Win32::Foundation::HWND {
        windows::Win32::Foundation::HWND(self.0 .0.hwnd)
    }

    #[cfg(target_os = "linux")]
    pub fn new_from_wayland(display: *mut c_void) -> Self {
        Self(Sendable(libobs::gs_window { display, id: 0 }))
    }

    #[cfg(target_os = "linux")]
    pub fn new_from_x11(display: *mut c_void, id: u32) -> Self {
        Self(Sendable(libobs::gs_window { display, id }))
    }
}

impl ObsDisplayRef {
    /// Call initialize to ObsDisplay#create the display
    /// NOTE: This must be pinned to prevent the draw callbacks from having an invalid pointer. DO NOT UNPIN
    pub(crate) fn new(
        data: ObsDisplayCreationData,
        runtime: ObsRuntime,
    ) -> anyhow::Result<std::pin::Pin<Box<Self>>> {
        use std::sync::atomic::Ordering;

        use anyhow::bail;
        use creation_data::ObsDisplayCreationData;

        use crate::run_with_obs;

        let ObsDisplayCreationData {
            x,
            y,
            height,
            width,
            window_handle,
            background_color,
            create_child,
            ..
        } = data.clone();

        #[cfg(windows)]
        let (mut manager, child_handle) = if create_child {
            let m = window_manager::windows::WindowDisplayWindowManager::new_child(
                window_handle.clone(),
                x,
                y,
                width,
                height,
            )?;
            let handle = m.get_window_handle();

            (m, Some(handle))
        } else {
            (
                window_manager::windows::WindowDisplayWindowManager::new(
                    window_handle.clone(),
                    x,
                    y,
                    width,
                    height,
                ),
                None,
            )
        };

        #[cfg(target_os = "linux")]
        let (mut manager, child_handle) = window_manager::linus::DisplayWindowManager::new(
            window_handle.clone(),
            x,
            y,
            width,
            height,
            create_child,
        )?;

        let init_data = Sendable(data.build(child_handle));

        log::trace!("Creating obs display...");
        let display = run_with_obs!(runtime, (init_data), move || unsafe {
            Sendable(libobs::obs_display_create(&init_data, background_color))
        })?;

        if display.0.is_null() {
            bail!("OBS failed to create display");
        }

        manager.set_display_handle(display.clone());

        let instance = Box::pin(Self {
            display: display.clone(),
            _guard: Arc::new(_ObsDisplayDropGuard {
                display,
                runtime: runtime.clone(),
            }),
            manager: Arc::new(RwLock::new(manager)),
            id: ID_COUNTER.fetch_add(1, Ordering::Relaxed),
            _fixed_in_heap: PhantomPinned,
            runtime: runtime.clone(),
        });

        log::trace!("Adding draw callback with display {:?}", instance.display);

        let display_ptr = instance.display.clone();
        run_with_obs!(runtime, (display_ptr), move || unsafe {
            libobs::obs_display_add_draw_callback(
                display_ptr,
                Some(render_display),
                std::ptr::null_mut(),
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
