//! # OBS Window Helper
//! This crate provides necessary information about windows that could be used
//! so they can be captured with the `window_capture` or `game_capture` source in OBS.
//! <br> The function you probably want to use is `get_all_windows` which returns a list of `WindowInfo` structs.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

#[cfg(not(target_pointer_width = "64"))]
compile_error!("compilation is only allowed for 64-bit targets");

mod error;
pub use error::WindowHelperError;

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(windows)]
mod game;
#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(windows)]
mod monitor;
#[cfg(windows)]
mod util;
#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(windows)]
mod window;

#[cfg(windows)]
pub use util::*;
#[cfg(test)]
#[cfg(windows)]
mod test;

#[cfg(windows)]
pub use game::*;
#[cfg(windows)]
pub use helper::*;
#[cfg(windows)]
use win_iterator::{first_window, next_window};
#[cfg(windows)]
use windows::Win32::{Foundation::HWND, System::Console::GetConsoleWindow};

/// Retrieves information about all windows based on the specified search mode and game check flag.
///
/// # Arguments
///
/// * `mode` - The search mode to use for window enumeration.
/// * `check_game` - A flag indicating wether a `game_capture` or a `window_capture` is used
///
/// # Returns
///
/// A `Result` containing a vector of `WindowInfo` structs representing the retrieved window information, or a `WindowHelperError` if an error occurs.
#[cfg(windows)]
pub fn get_all_windows(mode: WindowSearchMode) -> Result<Vec<WindowInfo>, WindowHelperError> {
    let mut use_find_window_ex = false;

    let mut parent = None as Option<HWND>;
    let window = first_window(mode, &mut parent, &mut use_find_window_ex)?;
    let mut window = Some(window);

    // Safety: Retrieves the HWND of the current console window; the handle is only compared, not used
    // for further operations.
    let curr = unsafe { GetConsoleWindow() };

    let mut out = Vec::new();
    while window.is_some_and(|e| !e.is_invalid()) {
        let w = window.unwrap();
        if curr != w {
            let res = get_window_info(w);
            if let Ok(info) = res {
                out.push(info);
            } else {
                //eprintln!("Error: {:?}", res.err().unwrap());
            }
        }

        window = next_window(window, mode, &mut parent, use_find_window_ex)?;
    }

    Ok(out)
}

#[cfg(windows)]
const OBS_PIPE_NAME: &str = "CaptureHook_Pipe";

#[cfg(windows)]
pub fn is_window_in_use_by_other_instance(window_pid: u32) -> std::io::Result<bool> {
    #[cfg(not(windows))]
    return false;

    let pipe_name = format!("{}{}", OBS_PIPE_NAME, window_pid);
    let paths = std::fs::read_dir(r"\\.\pipe\")?;

    for path in paths {
        let path = path?;
        let name = path.file_name();
        let name = name.to_string_lossy();

        if name == pipe_name {
            return Ok(true);
        }
    }

    Ok(false)
}
