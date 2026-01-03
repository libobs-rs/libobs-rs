mod camera;
pub use camera::*;

mod desktop;
pub use desktop::*;

mod screen;
pub use screen::*;

mod window;
pub use window::*;

mod restore_updater;
pub use restore_updater::*;

use libobs_wrapper::{
    data::{object::ObsObjectTrait, ObsDataGetters},
    run_with_obs,
    sources::ObsSourceRef,
    utils::ObsError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// PipeWire source type
pub enum ObsPipeWireSourceType {
    /// Screen capture via desktop portal
    DesktopCapture,
    WindowCapture,
    ScreenCapture,
    /// Camera capture via camera portal
    CameraCapture,
}

#[derive(Debug, Clone)]
/// General PipeWire source reference wrapper, which has a restore_token
pub struct ObsPipeWireSourceRef {
    source: ObsSourceRef,
    source_type: ObsPipeWireSourceType,
}

libobs_wrapper::forward_obs_object_impl!(ObsPipeWireSourceRef, source, *mut libobs::obs_source);
libobs_wrapper::forward_obs_source_impl!(ObsPipeWireSourceRef, source);

impl ObsPipeWireSourceRef {
    /// Creates a new `ObsPipeWireSourceRef` from an `ObsSourceRef`.
    pub fn new(source: ObsSourceRef, source_type: ObsPipeWireSourceType) -> Result<Self, ObsError> {
        Ok(Self {
            source,
            source_type,
        })
    }

    /// Gets the restore token used for reconnecting to previous sessions for `pipewire-desktop-capture-source` and `pipewire-window-capture-source` sources.
    ///
    /// As of right now, there is no callback or signal to notify when the token has been set, you have to call this method to get the restore token.
    ///
    /// The restore token will most probably be of `Some(String)` after the user has selected a screen or window to capture.
    pub fn get_restore_token(&self) -> Result<Option<String>, ObsError> {
        let source_ptr = self.as_ptr();
        run_with_obs!(self.runtime(), (source_ptr), move || unsafe {
            // Safety: Safe because we are using a smart pointer
            libobs::obs_source_save(source_ptr.get_ptr());
        })?;

        let settings = self.settings()?;
        let token = settings.get_string("RestoreToken")?;
        Ok(token)
    }

    pub fn create_updater<'a>(
        &'a mut self,
    ) -> Result<ObsPipeWireGeneralUpdater<'a>, libobs_wrapper::utils::ObsError> {
        use libobs_wrapper::data::object::ObsObjectTrait;
        use libobs_wrapper::data::ObsObjectUpdater;
        ObsPipeWireGeneralUpdater::create_update(self.runtime().clone(), self)
    }

    pub fn get_source_type(&self) -> ObsPipeWireSourceType {
        self.source_type
    }
}

macro_rules! impl_pipewire_source_builder {
    ($struct_name: ident, $source_type: expr) => {
        impl libobs_wrapper::sources::ObsSourceBuilder for $struct_name {
            type T = crate::sources::linux::pipewire::ObsPipeWireSourceRef;

            fn build(self) -> Result<Self::T, libobs_wrapper::utils::ObsError>
            where
                Self: Sized,
            {
                use libobs_wrapper::data::ObsObjectBuilder;
                let runtime = self.runtime.clone();
                let info = self.object_build()?;

                let source = libobs_wrapper::sources::ObsSourceRef::new_from_info(info, runtime)?;

                crate::sources::linux::pipewire::ObsPipeWireSourceRef::new(source, $source_type)
            }
        }
    };
}

pub(crate) use impl_pipewire_source_builder;
