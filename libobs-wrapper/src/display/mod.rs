//! This module is used to create a `OBS display` which you can use to preview the
//! output of your recording.

mod creation_data;
mod enums;
mod geometry;
mod window_manager;

pub use window_manager::{MiscDisplayTrait, ShowHideTrait, WindowPositionTrait};

pub use creation_data::*;
pub use enums::*;
pub use geometry::*;
use libobs::obs_video_info;

use crate::unsafe_send::SmartPointerSendable;
use crate::utils::{ObsDropGuard, ObsError};
use crate::{data::object::ObsObjectTrait, scenes::ObsSceneRef, sources::ObsSourceRef};
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

/// Content rendered by a preview display. The managed variants retain their native source/scene
/// lifetime while selected, including while the graphics callback is executing.
#[derive(Debug, Clone)]
pub enum PreviewTarget {
    Program,
    Scene(ObsSceneRef),
    Source(ObsSourceRef),
}

#[derive(Debug)]
pub(super) struct DisplayRenderState {
    pub(super) position: RwLock<(i32, i32)>,
    pub(super) scale_mode: RwLock<PreviewScaleMode>,
    pub(super) target: RwLock<PreviewTarget>,
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
    let mode = *state
        .scale_mode
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let target = state
        .target
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let mut ovi = MaybeUninit::<obs_video_info>::uninit();
    let was_ok = libobs::obs_get_video_info(ovi.as_mut_ptr());
    if !was_ok {
        log::error!("Failed to get video info in display render callback");
        return;
    }
    let ovi = unsafe {
        // Safety: was_ok checked that the video info was properly initialized.
        ovi.assume_init()
    };

    let (content_width, content_height, source_ptr) = match &*target {
        PreviewTarget::Program => (ovi.base_width, ovi.base_height, None),
        PreviewTarget::Source(source) => {
            let ptr = source.__native_handle();
            let width = libobs::obs_source_get_width(ptr.get_ptr());
            let height = libobs::obs_source_get_height(ptr.get_ptr());
            (width.max(1), height.max(1), Some(ptr.get_ptr()))
        }
        PreviewTarget::Scene(scene) => {
            let source = libobs::obs_scene_get_source(scene.as_ptr().get_ptr());
            if source.is_null() {
                return;
            }
            let width = libobs::obs_source_get_width(source)
                .max(ovi.base_width)
                .max(1);
            let height = libobs::obs_source_get_height(source)
                .max(ovi.base_height)
                .max(1);
            (width, height, Some(source))
        }
    };
    let Some(geometry) =
        PreviewGeometry::new(content_width, content_height, width, height, mode, 1.0)
    else {
        return;
    };

    libobs::gs_viewport_push();
    libobs::gs_projection_push();
    libobs::gs_ortho(
        0.0,
        content_width as f32,
        0.0,
        content_height as f32,
        -100.0,
        100.0,
    );
    libobs::gs_set_viewport(
        pos.0 + geometry.render_rect.x.round() as i32,
        pos.1 + geometry.render_rect.y.round() as i32,
        geometry.render_rect.width.round().max(1.0) as i32,
        geometry.render_rect.height.round().max(1.0) as i32,
    );

    match source_ptr {
        Some(source) => libobs::obs_source_video_render(source),
        None => libobs::obs_render_main_texture_src_color_only(),
    }

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
            scale_mode: RwLock::new(PreviewScaleMode::Fit),
            target: RwLock::new(PreviewTarget::Program),
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

    /// Selects fit/letterbox, fill/crop, or stretch mapping for subsequent renders.
    pub fn set_scale_mode(&self, mode: PreviewScaleMode) -> Result<(), ObsError> {
        *self
            .render_state
            .scale_mode
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))? = mode;
        Ok(())
    }

    pub fn scale_mode(&self) -> Result<PreviewScaleMode, ObsError> {
        self.render_state
            .scale_mode
            .read()
            .map(|mode| *mode)
            .map_err(|e| ObsError::LockError(e.to_string()))
    }

    /// Switches the preview target while retaining the selected source/scene for the graphics
    /// callback. Runtime affinity is checked before the target becomes visible to rendering.
    pub fn set_target(&self, target: PreviewTarget) -> Result<(), ObsError> {
        match &target {
            PreviewTarget::Program => {}
            PreviewTarget::Source(source) => self.runtime.ensure_same_runtime(source.runtime())?,
            PreviewTarget::Scene(scene) => self.runtime.ensure_same_runtime(scene.runtime())?,
        }
        *self
            .render_state
            .target
            .write()
            .map_err(|e| ObsError::LockError(e.to_string()))? = target;
        Ok(())
    }

    pub fn target(&self) -> Result<PreviewTarget, ObsError> {
        self.render_state
            .target
            .read()
            .map(|target| target.clone())
            .map_err(|e| ObsError::LockError(e.to_string()))
    }

    /// Resizes the native OBS display in physical pixels.
    pub fn resize(&self, width: u32, height: u32) -> Result<(), ObsError> {
        if width == 0 || height == 0 {
            return Err(ObsError::InvalidOperation(
                "preview display dimensions must be non-zero".into(),
            ));
        }
        let display = self.as_ptr();
        let runtime = self.runtime.clone();
        run_with_obs!(runtime, (display), move || unsafe {
            // Safety: display is retained for the actor call.
            libobs::obs_display_resize(display.get_ptr(), width, height);
        })
    }

    /// Computes the same geometry used by rendering for editor pointer conversion. Display size is
    /// physical pixels. Pass the toolkit's physical/logical DPI ratio for logical pointer helpers.
    pub fn preview_geometry(&self, dpi_scale: f32) -> Result<PreviewGeometry, ObsError> {
        let target = self.target()?;
        let mode = self.scale_mode()?;
        let display = self.as_ptr();
        let runtime = self.runtime.clone();
        run_with_obs!(runtime, (display, target), move || {
            let mut viewport_width = 0_u32;
            let mut viewport_height = 0_u32;
            unsafe {
                // Safety: display is retained and output pointers are valid stack locations.
                libobs::obs_display_size(
                    display.get_ptr(),
                    &mut viewport_width,
                    &mut viewport_height,
                );
            }

            let (content_width, content_height) = match target {
                PreviewTarget::Program => {
                    let mut ovi = MaybeUninit::<obs_video_info>::uninit();
                    // SAFETY: `ovi` is valid writable storage and this runs on the OBS actor.
                    if !unsafe { libobs::obs_get_video_info(ovi.as_mut_ptr()) } {
                        return Err(ObsError::InvalidOperation(
                            "OBS video info is unavailable for preview geometry".into(),
                        ));
                    }
                    // SAFETY: successful `obs_get_video_info` initialized the complete structure.
                    let ovi = unsafe { ovi.assume_init() };
                    (ovi.base_width, ovi.base_height)
                }
                PreviewTarget::Source(source) => {
                    let ptr = source.__native_handle();
                    // SAFETY: the managed source lease is retained for this actor command.
                    unsafe {
                        (
                            libobs::obs_source_get_width(ptr.get_ptr()),
                            libobs::obs_source_get_height(ptr.get_ptr()),
                        )
                    }
                }
                PreviewTarget::Scene(scene) => {
                    // SAFETY: the managed scene lease is retained for this actor command.
                    let source = unsafe { libobs::obs_scene_get_source(scene.as_ptr().get_ptr()) };
                    if source.is_null() {
                        return Err(ObsError::NullPointer(Some("scene preview source".into())));
                    }
                    // SAFETY: `source` is the non-null scene source borrowed for this actor call.
                    unsafe {
                        (
                            libobs::obs_source_get_width(source),
                            libobs::obs_source_get_height(source),
                        )
                    }
                }
            };
            PreviewGeometry::new(
                content_width,
                content_height,
                viewport_width,
                viewport_height,
                mode,
                dpi_scale,
            )
            .ok_or_else(|| ObsError::InvalidOperation("invalid preview geometry".into()))
        })?
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
