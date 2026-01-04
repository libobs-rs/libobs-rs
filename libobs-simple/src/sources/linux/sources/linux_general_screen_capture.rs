use libobs_wrapper::{
    data::ObsObjectBuilder,
    runtime::ObsRuntime,
    sources::{ObsSourceBuilder, ObsSourceRef},
    utils::{ObjectInfo, ObsError, ObsString, PlatformType},
};

use crate::sources::linux::{
    pipewire::{ObsPipeWireSourceRef, PipeWireScreenCaptureSourceBuilder},
    Either, EitherSource, X11CaptureSourceBuilder,
};

pub struct LinuxGeneralScreenCaptureBuilder {
    underlying_builder: Either<X11CaptureSourceBuilder, PipeWireScreenCaptureSourceBuilder>,
}

impl ObsObjectBuilder for LinuxGeneralScreenCaptureBuilder {
    fn new<T: Into<ObsString> + Send + Sync>(name: T, runtime: ObsRuntime) -> Result<Self, ObsError>
    where
        Self: Sized,
    {
        let platform = runtime.get_platform()?;
        let underlying_builder = match platform {
            PlatformType::X11 => Either::Left(X11CaptureSourceBuilder::new(name, runtime)?),
            PlatformType::Wayland => {
                Either::Right(PipeWireScreenCaptureSourceBuilder::new(name, runtime)?)
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
        ObsString::from("linux_general_screen_capture")
    }
}

pub type LinuxGeneralScreenCaptureSourceRef = EitherSource<ObsSourceRef, ObsPipeWireSourceRef>;

impl ObsSourceBuilder for LinuxGeneralScreenCaptureBuilder {
    type T = LinuxGeneralScreenCaptureSourceRef;

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

impl LinuxGeneralScreenCaptureBuilder {
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

    /// Set the screen/display to capture
    /// # Display Server
    /// X11 only
    pub fn set_screen(mut self, screen: i64) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_screen(screen)),
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }

    /// Enable advanced settings for X11 capture
    /// # Display Server
    /// X11 only
    pub fn set_advanced(mut self, advanced: bool) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_advanced(advanced)),
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }

    /// Set the X server to connect to (when using advanced settings)
    /// # Display Server
    /// X11 only
    pub fn set_server(mut self, server: &str) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_server(server.to_string())),
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }

    /// Crop from top (in pixels)
    /// # Display Server
    /// X11 only
    pub fn set_cut_top(mut self, cut_top: i64) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_cut_top(cut_top)),
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }

    /// Crop from left (in pixels)
    /// # Display Server
    /// X11 only
    pub fn set_cut_left(mut self, cut_left: i64) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_cut_left(cut_left)),
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }

    /// Crop from right (in pixels)
    /// # Display Server
    /// X11 only
    pub fn set_cut_right(mut self, cut_right: i64) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_cut_right(cut_right)),
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }

    /// Crop from bottom (in pixels)
    /// # Display Server
    /// X11 only
    pub fn set_cut_bot(mut self, cut_bot: i64) -> Self {
        self.underlying_builder = match self.underlying_builder {
            Either::Left(builder) => Either::Left(builder.set_cut_bot(cut_bot)),
            Either::Right(builder) => Either::Right(builder),
        };

        self
    }

    pub fn capture_type_name(&self) -> PlatformType {
        match &self.underlying_builder {
            Either::Left(_) => PlatformType::X11,
            Either::Right(_) => PlatformType::Wayland,
        }
    }
}
