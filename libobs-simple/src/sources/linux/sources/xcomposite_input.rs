use libobs_wrapper::{
    data::ObsObjectBuilder,
    sources::{ObsSourceBuilder, ObsSourceRef},
};

use crate::sources::macro_helper::{define_object_manager, impl_custom_source};

define_object_manager!(
    #[derive(Debug)]
    /// A source to capture X11 windows using XComposite.
    ///
    /// This source provides window capture functionality on Linux systems running X11
    /// using the XComposite extension. It can capture individual windows with their
    /// transparency and effects intact.
    struct XCompositeInputSource("xcomposite_input", *mut libobs::obs_source) for ObsSourceRef {
        /// Window to capture (window ID as string)
        #[obs_property(type_t = "string")]
        capture_window: String,

        /// Crop from top (in pixels)
        #[obs_property(type_t = "int")]
        cut_top: i64,

        /// Crop from left (in pixels)
        #[obs_property(type_t = "int")]
        cut_left: i64,

        /// Crop from right (in pixels)
        #[obs_property(type_t = "int")]
        cut_right: i64,

        /// Crop from bottom (in pixels)
        #[obs_property(type_t = "int")]
        cut_bot: i64,

        /// Whether to show the cursor in the capture
        #[obs_property(type_t = "bool")]
        show_cursor: bool,

        /// Include window border/decorations
        #[obs_property(type_t = "bool")]
        include_border: bool,

        /// Exclude alpha channel (disable transparency)
        #[obs_property(type_t = "bool")]
        exclude_alpha: bool,
    }
);

impl_custom_source!(XCompositeInputSource, [
    //TODO Add support for the `linux-capture` type as it does not contain the `title` field (its 'name' instead)
    "hooked": {struct HookedSignal {
        name: String,
        class: String;
        POINTERS {
            source: *mut libobs::obs_source_t,
        }
    }},
    //TODO Add support for the `linux-capture` type as it does not contain the `title` field (its 'name' instead)
    "unhooked": {struct UnhookedSignal {
        POINTERS {
            source: *mut libobs::obs_source_t,
        }
    }},
]);

impl ObsSourceBuilder for XCompositeInputSourceBuilder {
    type T = XCompositeInputSource;

    fn build(self) -> Result<Self::T, libobs_wrapper::utils::ObsError>
    where
        Self: Sized,
    {
        let runtime = self.runtime.clone();
        let info = self.object_build()?;
        let source = ObsSourceRef::new_from_info(info, runtime)?;

        XCompositeInputSource::new(source)
    }
}
