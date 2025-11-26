use crate::{
    context::ObsContext,
    data::{audio::ObsAudioInfo, video::ObsVideoInfo},
    logger::{ConsoleLogger, ObsLogger},
    utils::{initialization::NixDisplay, ObsError, ObsPath, ObsString},
};

/// Contains information to start a libobs context.
/// This is passed to the creation of `ObsContext`.
///
/// ## Platform Notes
/// On Linux platforms, if your application uses a GUI
/// framework (like GTK, Qt, etc.), it is crucial to set
/// the appropriate `NixDisplay` in the `StartupInfo` **if you want to create a preview window**.
/// This ensures that libobs can correctly interface with
/// the display server (X11 or Wayland) used by your application.
/// If this is not set, libobs will not be able to create a preview window and the application will crash.
#[derive(Debug)]
pub struct StartupInfo {
    pub(crate) startup_paths: StartupPaths,
    pub(crate) obs_video_info: ObsVideoInfo,
    pub(crate) obs_audio_info: ObsAudioInfo,
    // Option because logger is taken when creating
    pub(crate) logger: Option<Box<dyn ObsLogger + Sync + Send>>,
    pub(crate) start_glib_loop: bool,
    pub(crate) nix_display: Option<NixDisplay>,
}

impl StartupInfo {
    pub fn new() -> StartupInfo {
        Self::default()
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn set_startup_paths(mut self, paths: StartupPaths) -> Self {
        self.startup_paths = paths;
        self
    }

    pub fn set_video_info(mut self, ovi: ObsVideoInfo) -> Self {
        self.obs_video_info = ovi;
        self
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn get_video_info(&self) -> &ObsVideoInfo {
        &self.obs_video_info
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn set_logger(mut self, logger: Box<dyn ObsLogger + Sync + Send>) -> Self {
        self.logger = Some(logger);
        self
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn set_start_glib_loop(mut self, start: bool) -> Self {
        self.start_glib_loop = start;
        self
    }

    /// This sets the Nix display (X11 or Wayland) to use when starting libobs.
    ///
    /// This is **important** if your application creates any preview windows using libobs on Linux.
    /// Otherwise if you don't plan to use preview windows **AND** are not using a GUI framework, you can skip this.
    ///
    /// Wayland requires this display to be the same as the one used by the GUI application (if you have one).
    /// Failing to set this may result in libobs being unable to create preview windows,
    ///
    /// X11 however works without setting this display, in fact your window may become unresponsive if a display is set.
    ///
    /// # Safety
    /// Make sure that the display is closed AFTER the whole OBS context has been dropped!
    pub unsafe fn set_nix_display(mut self, display: NixDisplay) -> Self {
        self.nix_display = Some(display);
        self
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn start(self) -> Result<ObsContext, ObsError> {
        ObsContext::new(self)
    }
}

impl Default for StartupInfo {
    fn default() -> StartupInfo {
        Self {
            startup_paths: StartupPaths::default(),
            obs_video_info: ObsVideoInfo::default(),
            obs_audio_info: ObsAudioInfo::default(),
            logger: Some(Box::new(ConsoleLogger::new())),
            start_glib_loop: true,
            nix_display: None,
        }
    }
}

/// Contains the necessary paths for starting the
/// libobs context built from `ObsPath`.
///
/// Note that these strings are copied when parsed,
/// meaning that these can be freed immediately
/// after all three strings have been used.
#[derive(Clone, Debug)]
pub struct StartupPaths {
    libobs_data_path: ObsString,
    plugin_bin_path: ObsString,
    plugin_data_path: ObsString,
}

impl StartupPaths {
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn new(
        libobs_data_path: ObsPath,
        plugin_bin_path: ObsPath,
        plugin_data_path: ObsPath,
    ) -> StartupPaths {
        Self {
            libobs_data_path: libobs_data_path.build(),
            plugin_bin_path: plugin_bin_path.build(),
            plugin_data_path: plugin_data_path.build(),
        }
    }

    pub fn libobs_data_path(&self) -> &ObsString {
        &(self.libobs_data_path)
    }

    pub fn plugin_bin_path(&self) -> &ObsString {
        &(self.plugin_bin_path)
    }

    pub fn plugin_data_path(&self) -> &ObsString {
        &(self.plugin_data_path)
    }
}

impl Default for StartupPaths {
    fn default() -> Self {
        StartupPathsBuilder::new().build()
    }
}

#[derive(Clone, Debug)]
pub struct StartupPathsBuilder {
    libobs_data_path: ObsPath,
    plugin_bin_path: ObsPath,
    plugin_data_path: ObsPath,
}

impl StartupPathsBuilder {
    fn new() -> Self {
        #[cfg(not(target_os = "linux"))]
        return Self {
            libobs_data_path: ObsPath::from_relative("data/libobs"),
            plugin_bin_path: ObsPath::from_relative("obs-plugins/64bit"),
            plugin_data_path: ObsPath::from_relative("data/obs-plugins/%module%"),
        };

        #[cfg(target_os = "linux")]
        let arch = std::env::consts::ARCH;
        #[cfg(target_os = "linux")]
        let lib_path = match arch {
            "x86_64" => "/usr/lib/x86_64-linux-gnu",
            "aarch64" => "/usr/lib/aarch64-linux-gnu",
            "arm" => "/usr/lib/arm-linux-gnueabihf",
            _ => "/usr/lib",
        };

        #[cfg(target_os = "linux")]
        return Self {
            libobs_data_path: ObsPath::new("/usr/share/obs/libobs"),
            plugin_bin_path: ObsPath::new(&(lib_path.to_string() + "/obs-plugins/%module%")),
            plugin_data_path: ObsPath::new("/usr/share/obs/obs-plugins/%module%"),
        };
    }

    pub fn build(self) -> StartupPaths {
        StartupPaths {
            libobs_data_path: self.libobs_data_path.build(),
            plugin_bin_path: self.plugin_bin_path.build(),
            plugin_data_path: self.plugin_data_path.build(),
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn libobs_data_path(mut self, value: ObsPath) -> Self {
        self.libobs_data_path = value;
        self
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn plugin_bin_path(mut self, value: ObsPath) -> Self {
        self.plugin_bin_path = value;
        self
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn plugin_data_path(mut self, value: ObsPath) -> Self {
        self.plugin_data_path = value;
        self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for StartupPathsBuilder {
    fn default() -> StartupPathsBuilder {
        Self::new()
    }
}
