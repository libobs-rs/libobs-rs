use libobs_wrapper::{
    data::ObsObjectBuilder,
    runtime::ObsRuntime,
    sources::ObsSourceBuilder,
    utils::{ObjectInfo, ObsError, ObsString, PlatformType},
};

use crate::sources::linux::{
    pipewire::{ObsPipeWireSourceRef, PipeWireWindowCaptureSourceBuilder},
    Either, EitherSource, XCompositeInputSource, XCompositeInputSourceBuilder,
};

pub struct LinuxGeneralWindowCaptureBuilder {
    underlying_builder: Either<XCompositeInputSourceBuilder, PipeWireWindowCaptureSourceBuilder>,
}

impl ObsObjectBuilder for LinuxGeneralWindowCaptureBuilder {
    fn new<T: Into<ObsString> + Send + Sync>(name: T, runtime: ObsRuntime) -> Result<Self, ObsError>
    where
        Self: Sized,
    {
        let underlying_builder = match runtime.get_platform()? {
            PlatformType::X11 => Either::Left(XCompositeInputSourceBuilder::new(name, runtime)?),
            PlatformType::Wayland => {
                Either::Right(PipeWireWindowCaptureSourceBuilder::new(name, runtime)?)
            }
            PlatformType::Invalid => {
                return Err(ObsError::PlatformInitError(
                    "No platform could be found to create the source on.".to_string(),
                ))
            }
        };

        Ok(Self { underlying_builder })
    }

    fn runtime(&self) -> &ObsRuntime {
        match &self.underlying_builder {
            Either::Left(builder) => builder.runtime(),
            Either::Right(builder) => builder.runtime(),
        }
    }

    fn get_name(&self) -> ObsString {
        match &self.underlying_builder {
            Either::Left(builder) => builder.get_name(),
            Either::Right(builder) => builder.get_name(),
        }
    }

    fn object_build(self) -> Result<ObjectInfo, ObsError>
    where
        Self: Sized,
    {
        match self.underlying_builder {
            Either::Left(builder) => builder.object_build(),
            Either::Right(builder) => builder.object_build(),
        }
    }

    fn get_settings(&self) -> &libobs_wrapper::data::ObsData {
        match &self.underlying_builder {
            Either::Left(builder) => builder.get_settings(),
            Either::Right(builder) => builder.get_settings(),
        }
    }

    fn get_settings_updater(&mut self) -> &mut libobs_wrapper::data::ObsDataUpdater {
        match &mut self.underlying_builder {
            Either::Left(builder) => builder.get_settings_updater(),
            Either::Right(builder) => builder.get_settings_updater(),
        }
    }

    fn get_hotkeys(&self) -> &libobs_wrapper::data::ObsData {
        match &self.underlying_builder {
            Either::Left(builder) => builder.get_hotkeys(),
            Either::Right(builder) => builder.get_hotkeys(),
        }
    }

    fn get_hotkeys_updater(&mut self) -> &mut libobs_wrapper::data::ObsDataUpdater {
        match &mut self.underlying_builder {
            Either::Left(builder) => builder.get_hotkeys_updater(),
            Either::Right(builder) => builder.get_hotkeys_updater(),
        }
    }

    fn get_id() -> ObsString {
        ObsString::from("linux_general_window_capture")
    }
}

pub type LinuxGeneralWindowCaptureSourceRef =
    EitherSource<XCompositeInputSource, ObsPipeWireSourceRef>;

impl ObsSourceBuilder for LinuxGeneralWindowCaptureBuilder {
    type T = LinuxGeneralWindowCaptureSourceRef;

    fn build(self) -> Result<Self::T, ObsError>
    where
        Self: Sized,
    {
        match self.underlying_builder {
            Either::Left(builder) => {
                let source = builder.build()?;
                Ok(EitherSource::Left(source))
            }
            Either::Right(builder) => {
                let source = builder.build()?;
                Ok(EitherSource::Right(source))
            }
        }
    }
}

impl LinuxGeneralWindowCaptureBuilder {
    /// Set the PipeWire restore token, which will be used to re-establish the same selection the
    /// user did previously.
    /// # Display Server
    /// PipeWire only
    pub fn set_restore_token(mut self, token: &str) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder),
            Either::Right(builder) => Either::Right(builder.set_restore_token(token.to_string())),
        };

        self
    }

    /// # Display Server
    /// All supported display servers
    pub fn set_show_cursor(mut self, show: bool) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_show_cursor(show)),
            Either::Right(builder) => Either::Right(builder.set_show_cursor(show)),
        };

        self
    }

    /// Set the window to capture (window ID as string)
    /// # Display Server
    /// XComposite (X11) only
    pub fn set_capture_window(mut self, capture_window: &str) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => {
                Either::Left(builder.set_capture_window(capture_window.to_string()))
            }
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }

    /// Crop from top (in pixels)
    /// # Display Server
    /// XComposite (X11) only
    pub fn set_cut_top(mut self, cut_top: i64) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_cut_top(cut_top)),
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }

    /// Crop from left (in pixels)
    /// # Display Server
    /// XComposite (X11) only
    pub fn set_cut_left(mut self, cut_left: i64) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_cut_left(cut_left)),
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }

    /// Crop from right (in pixels)
    /// # Display Server
    /// XComposite (X11) only
    pub fn set_cut_right(mut self, cut_right: i64) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_cut_right(cut_right)),
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }

    /// Crop from bottom (in pixels)
    /// # Display Server
    /// XComposite (X11) only
    pub fn set_cut_bot(mut self, cut_bot: i64) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_cut_bot(cut_bot)),
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }

    /// Include window border/decorations
    /// # Display Server
    /// XComposite (X11) only
    pub fn set_include_border(mut self, include_border: bool) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_include_border(include_border)),
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }

    /// Exclude alpha channel (disable transparency)
    /// # Display Server
    /// XComposite (X11) only
    pub fn set_exclude_alpha(mut self, exclude_alpha: bool) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_exclude_alpha(exclude_alpha)),
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }
}
