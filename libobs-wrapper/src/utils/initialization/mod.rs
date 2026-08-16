#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub(crate) use windows::*;

#[cfg(not(windows))]
mod other;

#[cfg(not(windows))]
pub(crate) use other::*;

#[cfg(any(target_os = "linux", doc, feature = "__test_environment"))]
#[derive(Clone, Debug)]
pub enum PlatformType {
    X11,
    Wayland,
    Invalid,
}

/// Raw native display pointer whose lifetime is owned by the caller's GUI toolkit.
///
/// Safe code cannot construct this type. The caller must guarantee that the display
/// outlives the OBS context and that it belongs to the platform selected in
/// [`NixDisplay`].
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Clone, Copy, Debug)]
pub struct NativeDisplayHandle(*mut std::os::raw::c_void);

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
impl NativeDisplayHandle {
    /// # Safety
    /// `raw` must remain valid for the complete OBS context lifetime.
    pub unsafe fn from_raw(raw: *mut std::os::raw::c_void) -> Self {
        Self(raw)
    }

    pub(crate) fn as_ptr(self) -> *mut std::os::raw::c_void {
        self.0
    }
}

// Construction is unsafe and the pointer is never dereferenced outside the serialized
// OBS/platform initialization paths. This is intentionally specific to display handles,
// rather than a generic "make anything Send" wrapper.
unsafe impl Send for NativeDisplayHandle {}
unsafe impl Sync for NativeDisplayHandle {}

#[derive(Debug, Clone)]
pub enum NixDisplay {
    X11(NativeDisplayHandle),
    Wayland(NativeDisplayHandle),
}

impl NixDisplay {
    /// Construct an X11 display descriptor.
    ///
    /// # Safety
    /// The display pointer must be a live X11 display and outlive the OBS context.
    pub unsafe fn x11(raw: *mut std::os::raw::c_void) -> Self {
        // SAFETY: This constructor forwards the caller's documented lifetime and X11
        // validity guarantees unchanged to NativeDisplayHandle.
        Self::X11(unsafe { NativeDisplayHandle::from_raw(raw) })
    }

    /// Construct a Wayland display descriptor.
    ///
    /// # Safety
    /// The display pointer must be a live Wayland display and outlive the OBS context.
    pub unsafe fn wayland(raw: *mut std::os::raw::c_void) -> Self {
        // SAFETY: This constructor forwards the caller's documented lifetime and
        // Wayland validity guarantees unchanged to NativeDisplayHandle.
        Self::Wayland(unsafe { NativeDisplayHandle::from_raw(raw) })
    }
}
