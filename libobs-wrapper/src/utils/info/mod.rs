mod startup;
pub use startup::*;

use crate::data::{ImmutableObsData, ObsData};

use super::ObsString;

#[derive(Debug)]
pub struct ObjectInfo {
    pub id: ObsString,
    pub name: ObsString,
    pub settings: Option<ImmutableObsData>,
    pub hotkey_data: Option<ImmutableObsData>,
}

impl ObjectInfo {
    pub fn new(
        id: impl Into<ObsString>,
        name: impl Into<ObsString>,
        settings: Option<ObsData>,
        hotkey_data: Option<ObsData>,
    ) -> Self {
        let id = id.into();
        let name = name.into();

        Self {
            id,
            name,
            settings: settings.map(|s| s.into_immutable()),
            hotkey_data: hotkey_data.map(|h| h.into_immutable()),
        }
    }
}

pub type OutputInfo = ObjectInfo;
pub type SourceInfo = ObjectInfo;
pub type FilterInfo = ObjectInfo;
pub type AudioEncoderInfo = ObjectInfo;
pub type VideoEncoderInfo = ObjectInfo;
