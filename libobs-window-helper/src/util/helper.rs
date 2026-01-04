use crate::error::WindowHelperError;
use windows::{
    core::{Error, Result as WinResult},
    Win32::{Foundation::HWND, UI::WindowsAndMessaging::GetWindowThreadProcessId},
};

use crate::{
    is_blacklisted_window,
    monitor::get_monitor_id,
    validators::is_microsoft_internal_exe,
    window::{
        get_command_line_args, get_exe, get_product_name, get_title, get_window_class,
        hwnd_to_monitor, intersects_with_multiple_monitors,
    },
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone)]
/// Represents information about a window.
pub struct WindowInfo {
    /// The full path to the executable associated with the window.
    pub full_exe: String,
    /// The unique identifier of the window in OBS.
    pub obs_id: String,
    #[cfg(all(not(feature = "serde"), not(feature = "specta")))]
    /// The handle to the window (only enabled when feature `serde` is disabled).
    pub handle: HWND,
    /// The process ID of the window.
    pub pid: u32,
    /// The title of the window.
    pub title: Option<String>,
    /// The class name of the window.
    pub class: Option<String>,
    /// The product name of the window.
    pub product_name: Option<String>,
    /// The monitor on which the window is located.
    pub monitor: Option<String>,
    /// Indicates whether the window is between multiple monitors.
    pub intersects: Option<bool>,
    /// The command line used to launch the process.
    pub cmd_line: Option<String>,
    /// If this window can be recorded using a game capture source.
    pub is_game: bool,
}

fn encode_string(s: &str) -> String {
    s.replace("#", "#22").replace(":", "#3A")
}

/// Retrieves the OBS window information associated with the given window handle.
///
/// # Arguments
///
/// * `handle` - The handle to the window.
/// * `is_game` - If this flag is true, only game windows (that can be captured by the game source) are considered. Otherwise `window_capture` source info is returned.
///
/// # Returns
///
/// Returns the OBS window information as struct
///
/// # Errors
///
/// Returns an error if there was a problem retrieving the OBS ID.
pub fn get_window_info(wnd: HWND) -> Result<WindowInfo, WindowHelperError> {
    let (proc_id, full_exe) = get_exe(wnd)?;
    let exe = full_exe
        .file_name()
        .ok_or(WindowHelperError::FileNameError)?;
    let exe = exe
        .to_str()
        .ok_or(WindowHelperError::StringConversionError)?;
    let exe = exe.to_string();

    if is_microsoft_internal_exe(&exe) {
        return Err(WindowHelperError::MicrosoftInternalExe);
    }

    if exe == "obs64.exe" {
        return Err(WindowHelperError::ObsExe);
    }

    let is_game = !is_blacklisted_window(&exe);

    let title = get_title(wnd).ok();
    let class = get_window_class(wnd).ok();

    let product_name = get_product_name(&full_exe).ok();
    let monitor = Some(hwnd_to_monitor(wnd)?);
    let intersects = intersects_with_multiple_monitors(wnd).ok();
    let cmd_line = get_command_line_args(wnd).ok();
    let monitor_id = monitor.and_then(|e| get_monitor_id(e).ok());

    let title_o = title.as_ref().map_or("", |v| v);
    let class_o = class.as_ref().map_or("", |v| v);

    let obs_id: Vec<String> = vec![title_o, class_o, &exe]
        .into_iter()
        .map(encode_string)
        .collect();

    let obs_id = obs_id.join(":");
    Ok(WindowInfo {
        full_exe: full_exe.to_string_lossy().to_string(),
        obs_id,
        #[cfg(all(not(feature = "serde"), not(feature = "specta")))]
        handle: wnd,
        pid: proc_id,
        title,
        class,
        product_name,
        monitor: monitor_id,
        intersects,
        cmd_line,
        is_game,
    })
}

pub struct ProcessInfo {
    pub process_id: u32,
    pub thread_id: u32,
}

pub fn get_thread_proc_id(wnd: HWND) -> WinResult<ProcessInfo> {
    let mut proc_id = 0u32;

    let thread_id = unsafe {
        // Safety: `wnd` is an HWND obtained from Win32; `proc_id` is a valid out pointer to receive the
        // process id from `GetWindowThreadProcessId`.
        GetWindowThreadProcessId(wnd, Some(&mut proc_id))
    };
    if thread_id == 0 {
        return Err(Error::from_thread());
    }

    Ok(ProcessInfo {
        process_id: proc_id,
        thread_id,
    })
}
