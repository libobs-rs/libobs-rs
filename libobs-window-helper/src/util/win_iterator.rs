use crate::error::WindowHelperError;
use windows::{
    core::PWSTR,
    Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            FindWindowExW, GetDesktopWindow, GetWindow, GW_CHILD, GW_HWNDNEXT,
        },
    },
};

use crate::{
    get_thread_proc_id,
    validators::{is_window_valid, WindowSearchMode},
    window::get_window_class,
    ProcessInfo,
};

pub fn is_uwp_window(hwnd: HWND) -> Result<bool, WindowHelperError> {
    if hwnd.is_invalid() {
        return Ok(false);
    }

    let class = get_window_class(hwnd)?;
    Ok(class == "ApplicationFrameWindow")
}

pub fn get_uwp_actual_window(parent: HWND) -> Result<Option<HWND>, WindowHelperError> {
    let ProcessInfo {
        process_id: parent_id,
        ..
    } = get_thread_proc_id(parent)?;

    let mut child = unsafe {
        // Safety: `parent` comes from the caller and is treated as a trusted HWND; null class/title pointers
        // request the next child window and are permitted by the Win32 API.
        FindWindowExW(Some(parent), None, PWSTR::null(), PWSTR::null())?
    };

    while !child.is_invalid() {
        let ProcessInfo {
            process_id: child_id,
            ..
        } = get_thread_proc_id(child)?;

        if child_id != parent_id {
            return Ok(Some(child));
        }

        child = unsafe {
            // Safety: `parent` and `child` were obtained from Win32 enumeration and remain valid for
            // iteration; passing null class/title continues enumeration per Win32 docs.
            FindWindowExW(Some(parent), Some(child), PWSTR::null(), PWSTR::null())
                .unwrap_or(HWND::default())
        };
    }

    Ok(None)
}

pub fn next_window(
    window: Option<HWND>,
    mode: WindowSearchMode,
    parent: &mut Option<HWND>,
    use_find_window_ex: bool,
) -> Result<Option<HWND>, WindowHelperError> {
    let mut window = window.unwrap_or_default();

    let parent_valid = parent.is_some_and(|e| !e.is_invalid());
    if parent_valid {
        window = parent.unwrap_or_default();
        *parent = None;
    }

    loop {
        window = if use_find_window_ex {
            let desktop = unsafe {
                // Safety: `GetDesktopWindow` returns a process-wide pseudo-handle that is always valid.
                GetDesktopWindow()
            };

            unsafe {
                // Safety: `desktop` and `window` originate from Win32; null class/title values are allowed
                // to iterate the top-level window list via FindWindowExW.
                FindWindowExW(Some(desktop), Some(window), PWSTR::null(), PWSTR::null())
            }
        } else {
            unsafe {
                // Safety: `window` is sourced from Win32 enumeration, making it valid for GW_HWNDNEXT
                // traversal via `GetWindow`.
                GetWindow(window, GW_HWNDNEXT)
            }
        }
        .unwrap_or(HWND::default());

        let valid = is_window_valid(window, mode).ok().unwrap_or(false);
        if window.is_invalid() || valid {
            break;
        }
    }

    let window_opt = if window.is_invalid() {
        None
    } else {
        Some(window)
    };

    if is_uwp_window(window)? {
        if format!("{:?}", window.0).ends_with("041098") {
            println!("UWP Window: {:?}", window);
        }
        let actual = get_uwp_actual_window(window)?;
        if let Some(child) = actual {
            *parent = window_opt;

            return Ok(Some(child));
        }
    }

    Ok(window_opt)
}

pub fn first_window(
    mode: WindowSearchMode,
    parent: &mut Option<HWND>,
    use_find_window_ex: &mut bool,
) -> Result<HWND, WindowHelperError> {
    let desktop = unsafe {
        // Safety: `GetDesktopWindow` returns a valid pseudo-handle for the desktop window.
        GetDesktopWindow()
    };

    let mut window = unsafe {
        // Safety: Enumerating the first top-level window from the desktop is allowed with null class/title
        // pointers per Win32 API.
        FindWindowExW(Some(desktop), None, PWSTR::null(), PWSTR::null()).ok()
    };

    if window.is_none() {
        *use_find_window_ex = false;
        window = unsafe {
            // Safety: `desktop` is a valid pseudo-handle; GW_CHILD fetches its first child.
            GetWindow(desktop, GW_CHILD).ok()
        };
    } else {
        *use_find_window_ex = true;
    }

    *parent = None;

    let is_valid = window.is_some_and(|e| is_window_valid(e, mode).unwrap_or(false));

    if !is_valid {
        window = next_window(window, mode, parent, *use_find_window_ex)?;

        if window.is_none() && *use_find_window_ex {
            *use_find_window_ex = false;

            window = unsafe {
                // Safety: `desktop` is valid; fetching its first child window is permitted.
                GetWindow(desktop, GW_CHILD).ok()
            };
            let valid = window.is_some_and(|e| is_window_valid(e, mode).unwrap_or(false));

            if !valid {
                window = next_window(window, mode, parent, *use_find_window_ex)?;
            }
        }
    }

    if window.is_none() {
        return Err(WindowHelperError::NoWindowFound);
    }

    let window = window.unwrap();
    if is_uwp_window(window)? {
        let child = get_uwp_actual_window(window)?;
        if let Some(c) = child {
            *parent = Some(window);
            return Ok(c);
        }
    }

    Ok(window)
}
