use std::env;


/// Display server type detection
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayServerType {
    /// Wayland display server
    Wayland,
    /// X11/Xorg display server
    X11,
    /// Unknown or undetected display server
    Unknown,
}

impl DisplayServerType {
    /// Detect the current display server type using environment variables.
    ///
    /// Checks in order:
    /// 1. `XDG_SESSION_TYPE` (most reliable)
    /// 2. `WAYLAND_DISPLAY` (indicates Wayland)
    /// 3. `DISPLAY` (indicates X11)
    pub fn detect() -> Self {
        // First, check XDG_SESSION_TYPE (most reliable)
        if let Ok(session_type) = env::var("XDG_SESSION_TYPE") {
            let session_type = session_type.to_lowercase();
            if session_type.contains("wayland") {
                return DisplayServerType::Wayland;
            } else if session_type.contains("x11") {
                return DisplayServerType::X11;
            }
        }

        // Check WAYLAND_DISPLAY (if set, we're on Wayland)
        if env::var("WAYLAND_DISPLAY").is_ok() {
            return DisplayServerType::Wayland;
        }

        // Check DISPLAY (if set and no Wayland indicators, we're on X11)
        if env::var("DISPLAY").is_ok() {
            return DisplayServerType::X11;
        }

        DisplayServerType::Unknown
    }

    /// Returns whether PipeWire should be preferred for this display server.
    ///
    /// PipeWire is the modern capture API and works on both X11 and Wayland,
    /// but is essential for Wayland and optional for X11.
    pub fn prefer_pipewire(&self) -> bool {
        match self {
            DisplayServerType::Wayland => true, // PipeWire is required for Wayland
            DisplayServerType::X11 => false,    // X11 has native capture
            DisplayServerType::Unknown => true, // Default to PipeWire for safety
        }
    }
}