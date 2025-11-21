use libobs::gs_init_data;
use num_traits::ToPrimitive;

use crate::display::ObsWindowHandle;

use super::{GsColorFormat, GsZstencilFormat};

pub type RawDisplayHandle = *mut ::std::os::raw::c_void;

#[derive(Clone)]
pub struct ObsDisplayCreationData {
    pub(super) window_handle: ObsWindowHandle,
    pub(super) create_child: bool,
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) format: GsColorFormat,
    pub(super) zsformat: GsZstencilFormat,
    pub(super) adapter: u32,
    pub(super) backbuffers: u32,
    pub(super) background_color: u32,
}

impl ObsDisplayCreationData {
    pub fn new(window_handle: ObsWindowHandle, x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            window_handle,
            create_child: true,
            format: GsColorFormat::BGRA,
            zsformat: GsZstencilFormat::ZSNone,
            x,
            y,
            width,
            height,
            adapter: 0,
            backbuffers: 0,
            background_color: 0,
        }
    }

    pub fn set_format(mut self, format: GsColorFormat) -> Self {
        self.format = format;
        self
    }

    pub fn set_zsformat(mut self, zsformat: GsZstencilFormat) -> Self {
        self.zsformat = zsformat;
        self
    }

    pub fn set_adapter(mut self, adapter: u32) -> Self {
        self.adapter = adapter;
        self
    }

    pub fn set_backbuffers(mut self, backbuffers: u32) -> Self {
        self.backbuffers = backbuffers;
        self
    }

    pub fn set_background_color(mut self, background_color: u32) -> Self {
        self.background_color = background_color;
        self
    }

    /// If enabled, creating the display will result in a child window being created inside the provided window handle. The display is attached to that child window. This is on by default.
    ///
    /// ## Platform
    /// This is only applicable on Windows.
    pub fn set_create_child(mut self, should_create: bool) -> Self {
        self.create_child = should_create;
        self
    }

    pub(super) fn build(self, window_override: Option<ObsWindowHandle>) -> gs_init_data {
        gs_init_data {
            cx: self.width,
            cy: self.height,
            #[cfg(target_family = "windows")]
            format: self.format.to_i32().unwrap(),

            #[cfg(not(target_family = "windows"))]
            format: self.format.to_u32().unwrap(),

            #[cfg(not(target_family = "windows"))]
            zsformat: self.zsformat.to_u32().unwrap(),

            #[cfg(target_family = "windows")]
            zsformat: self.zsformat.to_i32().unwrap(),

            window: window_override
                .map(|s| s.0 .0)
                .unwrap_or_else(|| self.window_handle.0 .0),
            adapter: self.adapter,
            num_backbuffers: self.backbuffers,
        }
    }
}
