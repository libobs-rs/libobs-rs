//! This

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

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

#[derive(Debug, Clone, Copy)]
struct MessageThreadHwnd(HWND);

// HWND is an opaque OS handle. It is only used through Win32 APIs, and the window
// itself is owned by the dedicated message thread until teardown completes.
unsafe impl Send for MessageThreadHwnd {}
unsafe impl Sync for MessageThreadHwnd {}

#[derive(Debug, Default)]
struct WindowUserData {
    display: Mutex<Option<SmartPointerSendable<*mut obs_display_t>>>,
}

/// Update color space using userdata owned by the message thread.
///
/// The message thread keeps an `Arc<WindowUserData>` alive for the complete window
/// lifetime. Cloning the native handle while holding the mutex leases the OBS display
/// for the duration of this callback, so teardown can race without a use-after-free.
unsafe fn update_color_space_from_userdata(window: HWND) {
    let user_data = GetWindowLongPtrW(window, GWLP_USERDATA) as *const WindowUserData;
    let Some(user_data) = user_data.as_ref() else {
        return;
    };
    let display = match user_data.display.lock() {
        Ok(display) => display.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    let Some(display) = display else {
        return;
    };

    log::trace!("Updating color space for display change/move");
    #[allow(unknown_lints)]
    #[allow(ensure_obs_call_in_runtime)]
    libobs::obs_display_update_color_space(display.get_ptr());
}

extern "system" fn wndproc(
    window: HWND,
    message: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    unsafe {
        // SAFETY: This is a valid window procedure called by the OS. The userdata
        // points to state retained by the message thread itself.
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

    user_data: Arc<WindowUserData>,
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
        let user_data = Arc::new(WindowUserData::default());
        let thread_user_data = user_data.clone();

        let parent = parent.get_hwnd();
        let parent = Mutex::new(MessageThreadHwnd(parent));
        let message_thread = std::thread::spawn(move || {
            let parent = parent
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
            // We have to have the whole window creation stuff here as well so the message loop functions
            let create = move || -> Result<MessageThreadHwnd, ObsError> {
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

                Ok(MessageThreadHwnd(window))
            };

            let r = create();
            let window = r.as_ref().ok().map(|r| r.0);
            if let Some(window) = window {
                unsafe {
                    // SAFETY: The message thread owns `thread_user_data` until its loop
                    // exits, so this pointer is stable for every WndProc invocation.
                    SetWindowLongPtrW(
                        window,
                        GWLP_USERDATA,
                        Arc::as_ptr(&thread_user_data) as isize,
                    );
                }
            }
            if tx.send(r).is_err() {
                log::warn!(
                    "Preview creator dropped before the window creation result was delivered"
                );
                return;
            }
            let Some(window) = window else {
                return;
            };

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
            window_handle: unsafe {
                // SAFETY: `window` was just created successfully by this message thread and
                // remains owned by it until the handler is dropped.
                ObsWindowHandle::new_from_handle(window.0 .0)
            },
            should_exit,
            child_message_thread: Some(message_thread),
            render_at_bottom: false,
            is_hidden: AtomicBool::new(false),
            user_data,
        })
    }

    pub fn get_window_handle(&self) -> ObsWindowHandle {
        self.window_handle.clone()
    }

    /// Set the obs display pointer in the window's user data for message handling
    pub(in crate::display::window_manager) fn has_display_handle(&self) -> bool {
        match self.user_data.display.lock() {
            Ok(display) => display.is_some(),
            Err(poisoned) => poisoned.into_inner().is_some(),
        }
    }

    pub(crate) fn set_display_handle(
        &mut self,
        handle: SmartPointerSendable<*mut libobs::obs_display>,
    ) {
        let mut display = match self.user_data.display.lock() {
            Ok(display) => display,
            Err(poisoned) => poisoned.into_inner(),
        };
        *display = Some(handle);
    }
}

impl Drop for WindowsPreviewChildWindowHandler {
    fn drop(&mut self) {
        log::trace!("Dropping DisplayWindowManager...");
        match self.user_data.display.lock() {
            Ok(mut display) => {
                display.take();
            }
            Err(poisoned) => {
                poisoned.into_inner().take();
            }
        }
        unsafe {
            // Safety: The window handle is valid because it was created and is owned by this struct.
            SetWindowLongPtrW(self.window_handle.get_hwnd(), GWLP_USERDATA, 0);
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

        // The quit message and `should_exit` flag request termination. Do not join from
        // Drop: a stuck native message loop must not stall application destruction.
        drop(self.child_message_thread.take());
    }
}
