use libobs_wrapper::sources::{ObsSourceBuilder, ObsSourceRef};

use crate::macro_helper::define_object_manager;

define_object_manager!(
    #[derive(Debug)]
    struct JackOutputSource("jack_output_capture") for ObsSourceRef {
        /// Whether the JACK server should start when the source is created
        #[obs_property(type_t = "string", settings_key="startjack")]
        start_jack: String,

        #[obs_property(type_t = "int")]
        channels: i64,
    }
);

impl ObsSourceBuilder for JackOutputSourceBuilder {}
