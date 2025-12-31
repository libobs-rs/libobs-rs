use std::ffi::CStr;

pub mod window_capture;
pub use window_capture::{
    WindowCaptureSource, WindowCaptureSourceBuilder, WindowCaptureSourceUpdater,
};

mod capture;
pub use capture::*;

pub mod game_capture;
pub use game_capture::{
    GameCaptureSource, GameCaptureSourceBuilder, GameCaptureSourceUpdater, ObsGameCaptureMode,
    ObsGameCaptureRgbaSpace,
};

pub mod monitor_capture;
pub use monitor_capture::{MonitorCaptureSourceBuilder, MonitorCaptureSourceUpdater};

#[cfg(feature = "window-list")]
pub use libobs_window_helper::{WindowInfo, WindowSearchMode};

// There's no way to get that through the bindings, so I'll just define it here
const AUDIO_SOURCE_TYPE: &CStr = c"wasapi_process_output_capture";
pub(super) fn audio_capture_available() -> bool {
    unsafe { !libobs::obs_get_latest_input_type_id(AUDIO_SOURCE_TYPE.as_ptr()).is_null() }
}
