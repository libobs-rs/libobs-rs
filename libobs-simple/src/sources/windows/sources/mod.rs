use std::ffi::CStr;

pub mod window_capture;
use libobs_wrapper::{
    impl_signal_manager, run_with_obs, runtime::ObsRuntime, sources::ObsSourceTrait,
    unsafe_send::SmartPointerSendable, utils::ObsError,
};
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

use crate::sources::ObsEitherSource;

// There's no way to get that through the bindings, so I'll just define it here
const AUDIO_SOURCE_TYPE: &CStr = c"wasapi_process_output_capture";
pub(super) fn audio_capture_available(runtime: &ObsRuntime) -> Result<bool, ObsError> {
    run_with_obs!(runtime, || unsafe {
        // Safety: This is safe because we know that this type ID exists in OBS if the feature is available
        !libobs::obs_get_latest_input_type_id(AUDIO_SOURCE_TYPE.as_ptr()).is_null()
    })
}

impl_signal_manager!(|ptr: SmartPointerSendable<*mut libobs::obs_source>| unsafe {
    // Safety: We are using a smart pointer, so it is fine
    libobs::obs_source_get_signal_handler(ptr.get_ptr())
}, ObsHookableSourceSignals for *mut libobs::obs_source, [
    "hooked": {struct HookedSignal {
        title: String,
        class: String,
        executable: String;
        POINTERS {
            source: *mut libobs::obs_source_t,
        }
    }},
    "unhooked": {struct UnhookedSignal {
        POINTERS {
            source: *mut libobs::obs_source_t,
        }
    }},
]);

pub trait ObsHookableSourceTrait {
    fn source_specific_signals(&self) -> std::sync::Arc<ObsHookableSourceSignals>;
}

impl<
        A: ObsHookableSourceTrait + ObsSourceTrait + Clone + 'static,
        B: ObsHookableSourceTrait + ObsSourceTrait + Clone + 'static,
    > ObsHookableSourceTrait for ObsEitherSource<A, B>
{
    fn source_specific_signals(&self) -> std::sync::Arc<ObsHookableSourceSignals> {
        match self {
            ObsEitherSource::Left(a) => a.source_specific_signals(),
            ObsEitherSource::Right(b) => b.source_specific_signals(),
        }
    }
}
