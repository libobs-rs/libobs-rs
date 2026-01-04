//! This

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use crate::unsafe_send::Sendable;
use crate::utils::ObsError;
use crate::{display::ObsWindowHandle, unsafe_send::SmartPointerSendable};
use lazy_static::lazy_static;
use libobs::obs_display_t;
use windows::{
    core::{w, HSTRING, PCWSTR},
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Dwm::DwmIsCompositionEnabled,
        System::LibraryLoader::{GetModuleHandleA, GetModuleHandleW},
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW,
            LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassExW,
            SetLayeredWindowAttributes, SetParent, SetWindowLongPtrW, TranslateMessage, CS_HREDRAW,
            CS_NOCLOSE, CS_OWNDC, CS_VREDRAW, GWLP_USERDATA, GWL_EXSTYLE, GWL_STYLE, HTTRANSPARENT,
            IDC_ARROW, LWA_ALPHA, MSG, WM_DISPLAYCHANGE, WM_MOVE, WM_NCHITTEST,
            WM_WINDOWPOSCHANGED, WNDCLASSEXW, WS_CHILD, WS_EX_COMPOSITED, WS_EX_LAYERED,
            WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
        },
    },
};

const WM_DESTROY_WINDOW: u32 = 0x8001; // Custom message

/// Function to update color space from window user data
/// # Safety
/// This function may never be called if the display window handle attached to the window user data is invalid.
unsafe fn update_color_space_from_userdata(window: HWND) {
    let user_data = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut obs_display_t;
    if !user_data.is_null() {
        log::trace!("Updating color space for display change/move");

        // Safety: This function locks a mutex under the hood and only changes one bool, so this is fine.
        #[allow(unknown_lints)]
        #[allow(ensure_obs_call_in_runtime)]
        libobs::obs_display_update_color_space(user_data);
    }
}

extern "system" fn wndproc(
    window: HWND,
    message: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    unsafe {
        // Safety: This is a valid window procedure called by the OS. I've seen this plenty of times.
        // TODO: Check for safety when the window is closed but update_color_space_from_userdata is still called. Maybe we need a sender / receiver model here?
        match message {
            WM_NCHITTEST => LRESULT(HTTRANSPARENT as _),
            WM_DESTROY_WINDOW => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_DISPLAYCHANGE | WM_MOVE | WM_WINDOWPOSCHANGED => {
                // Update color space when display changes or window moves
                update_color_space_from_userdata(window);
                DefWindowProcW(window, message, w_param, l_param)
            }
            _ => DefWindowProcW(window, message, w_param, l_param),
        }
    }
}

lazy_static! {
    static ref REGISTERED_CLASS: AtomicBool = AtomicBool::new(false);
}

fn try_register_class() -> windows::core::Result<()> {
    if REGISTERED_CLASS.load(Ordering::Relaxed) {
        return Ok(());
    }

    let instance = unsafe {
        // Safety: This is being called during initialization, so the module handle should be valid.
        GetModuleHandleA(None)?
    };
    let cursor = unsafe {
        // Safety: Loading a standard cursor is always safe.
        LoadCursorW(None, IDC_ARROW)?
    };

    let mut style = CS_HREDRAW | CS_VREDRAW | CS_NOCLOSE;

    let enabled = unsafe {
        // Safety: Always safe
        DwmIsCompositionEnabled()
    }?
    .as_bool();
    if !enabled {
        style |= CS_OWNDC;
    }

    let window_class = w!("Win32DisplayClass");
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        hCursor: cursor,
        hInstance: instance.into(),
        lpszClassName: window_class,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        ..Default::default()
    };

    let atom = unsafe {
        // Safety: We did use correct initialized values, so this is safe to do as well.
        RegisterClassExW(&wc as *const _)
    };

    if atom == 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    REGISTERED_CLASS.store(true, Ordering::Relaxed);
    Ok(())
}

#[derive(Debug)]
pub(crate) struct WindowsPreviewChildWindowHandler {
    // Shouldn't really be needed
    pub(in crate::display::window_manager) child_message_thread:
        Option<std::thread::JoinHandle<()>>,
    pub(in crate::display::window_manager) should_exit: Arc<AtomicBool>,
    pub(in crate::display::window_manager) window_handle: ObsWindowHandle,

    pub(in crate::display::window_manager) x: i32,
    pub(in crate::display::window_manager) y: i32,

    pub(in crate::display::window_manager) width: u32,
    pub(in crate::display::window_manager) height: u32,

    pub(in crate::display::window_manager) is_hidden: AtomicBool,
    pub(in crate::display::window_manager) render_at_bottom: bool,

    pub(in crate::display::window_manager) obs_display:
        Option<SmartPointerSendable<*mut obs_display_t>>,
}

impl WindowsPreviewChildWindowHandler {
    pub fn new_child(
        parent: ObsWindowHandle,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<Self, ObsError> {
        log::trace!("Creating WindowsPreviewChildWindowHandler...");
        let (tx, rx) = oneshot::channel();

        let should_exit = Arc::new(AtomicBool::new(false));
        let tmp = should_exit.clone();

        let parent = parent.get_hwnd();
        let parent = Mutex::new(Sendable(parent));
        let message_thread = std::thread::spawn(move || {
            let parent = parent.lock().unwrap().0;
            // We have to have the whole window creation stuff here as well so the message loop functions
            let create = move || -> Result<Sendable<HWND>, ObsError> {
                log::trace!("Registering class...");
                try_register_class().map_err(|e| ObsError::DisplayCreationError(e.to_string()))?;
                let enabled = unsafe {
                    // Safety: Always safe
                    DwmIsCompositionEnabled()
                        .map_err(|e| ObsError::DisplayCreationError(e.to_string()))?
                        .as_bool()
                };

                let mut window_style = WS_EX_TRANSPARENT;
                if enabled {
                    window_style |= WS_EX_COMPOSITED;
                }

                let instance = unsafe {
                    // Safety: This is being called during initialization, so the module handle should be valid.
                    GetModuleHandleW(PCWSTR::null())
                        .map_err(|e| ObsError::DisplayCreationError(e.to_string()))?
                };

                let class_name = HSTRING::from("Win32DisplayClass");
                let window_name = HSTRING::from("LibObsChildWindowPreview");
                log::trace!("Creating window...");

                log::debug!(
                    "Creating window with x: {}, y: {}, width: {}, height: {}",
                    x,
                    y,
                    width,
                    height
                );
                let window = unsafe {
                    // Safety: All parameters are valid because we just created the class and are providing valid parameters.

                    // More at https://github.com/stream-labs/obs-studio-node/blob/4e19d8a61a4dd7744e75ce77624c664e371cbfcf/obs-studio-server/source/nodeobs_display.cpp#L170
                    CreateWindowExW(
                        WS_EX_LAYERED,
                        &class_name,
                        &window_name,
                        WS_POPUP | WS_VISIBLE,
                        x,
                        y,
                        width as i32,
                        height as i32,
                        None,
                        None,
                        Some(instance.into()),
                        None,
                    )
                    .map_err(|e| ObsError::DisplayCreationError(e.to_string()))?
                };

                log::trace!("HWND is {:?}", window);
                if !enabled {
                    log::trace!("Setting attributes alpha...");
                    unsafe {
                        // Safety: The window handle is valid as it was just created. Therefore we can also set layered window attributes

                        SetLayeredWindowAttributes(window, COLORREF(0), 255, LWA_ALPHA)
                            .map_err(|e| ObsError::DisplayCreationError(e.to_string()))?;
                    }
                }

                log::trace!("Setting parent...");
                unsafe {
                    // Safety: Both are valid window handles, so it is safe to set the parent.
                    SetParent(window, Some(parent))
                        .map_err(|e| ObsError::DisplayCreationError(e.to_string()))?;
                }

                log::trace!("Setting styles...");
                let mut style = unsafe {
                    // Safety: Again the window handle is valid, so we can get the style
                    GetWindowLongPtrW(window, GWL_STYLE)
                };
                //TODO Check casts here
                style &= !(WS_POPUP.0 as isize);
                style |= WS_CHILD.0 as isize;

                unsafe {
                    // Safety: The window handle is valid, so we can set the style
                    SetWindowLongPtrW(window, GWL_STYLE, style)
                };

                let mut ex_style = unsafe {
                    // Safety: The window handle is valid, so we can get the extended style
                    GetWindowLongPtrW(window, GWL_EXSTYLE)
                };
                ex_style |= window_style.0 as isize;

                unsafe {
                    // Safety: The window handle is valid, so we can set the extended style
                    SetWindowLongPtrW(window, GWL_EXSTYLE, ex_style);
                }

                Ok(Sendable(window))
            };

            let r = create();
            let window = r.as_ref().ok().map(|r| r.0);
            tx.send(r).unwrap();
            if window.is_none() {
                return;
            }
            let window = window.unwrap();

            log::trace!("Starting up message thread...");
            let mut msg = MSG::default();
            unsafe {
                // Safety: I've seen this plenty of times, and this is the correct way to run a message loop.
                while !tmp.load(Ordering::Relaxed)
                    && GetMessageW(&mut msg, Some(window), 0, 0).as_bool()
                {
                    //TODO check if this can really be ignored
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            log::trace!("Exiting message thread...");
        });

        let window = rx.recv();
        let window = window.map_err(|_| {
            ObsError::RuntimeChannelError("Failed to receive window creation result".to_string())
        })??;
        Ok(Self {
            x,
            y,
            width,
            height,
            window_handle: ObsWindowHandle::new_from_handle(window.0 .0),
            should_exit,
            child_message_thread: Some(message_thread),
            render_at_bottom: false,
            is_hidden: AtomicBool::new(false),
            obs_display: None,
        })
    }

    pub fn get_window_handle(&self) -> ObsWindowHandle {
        self.window_handle.clone()
    }

    /// Set the obs display pointer in the window's user data for message handling
    pub(crate) fn set_display_handle(
        &mut self,
        handle: SmartPointerSendable<*mut libobs::obs_display>,
    ) {
        // REVIEW: Check if this the display is still being dropped
        self.obs_display = Some(handle.clone());
        unsafe {
            // Safety: The window handle is valid because it was created and is owned by this struct.
            SetWindowLongPtrW(
                self.window_handle.get_hwnd(),
                GWLP_USERDATA,
                handle.get_ptr() as isize,
            );
        }
    }
}

impl Drop for WindowsPreviewChildWindowHandler {
    fn drop(&mut self) {
        log::trace!("Dropping DisplayWindowManager...");
        unsafe {
            // Safety: The window handle is valid because it was created and is owned by this struct.
            SetWindowLongPtrW(
                self.window_handle.get_hwnd(),
                GWLP_USERDATA,
                std::ptr::null_mut::<libobs::obs_display>() as isize,
            );
        }

        self.should_exit.store(true, Ordering::Relaxed);
        log::trace!("Destroying window...");

        let res = unsafe {
            // Safety: The window handle is valid because it was created and is owned by this struct.
            PostMessageW(
                Some(self.window_handle.get_hwnd()),
                WM_DESTROY_WINDOW,
                WPARAM(0),
                LPARAM(0),
            )
        };

        if let Err(err) = res {
            log::error!("Failed to post destroy window message: {:?}", err);
        }

        let thread = self.child_message_thread.take();
        if let Some(thread) = thread {
            log::trace!("Waiting for message thread to exit...");
            let r = thread.join();
            if r.is_ok() {
                log::trace!("Message thread exited cleanly");
                return;
            }

            if !std::thread::panicking() {
                log::error!("Message thread panicked: {:?}", r.unwrap_err());
            } else {
                r.unwrap();
            }
        }
    }
}
