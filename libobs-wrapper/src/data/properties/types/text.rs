use getters0::Getters;

use crate::{
    data::properties::{get_enum, macros::is_of_type_result, ObsTextInfoType, ObsTextType},
    run_with_obs,
};

use super::PropertyCreationInfo;

#[derive(Debug, Getters, Clone)]
#[skip_new]
pub struct ObsTextProperty {
    name: String,
    description: Option<String>,
    monospace: bool,
    text_type: ObsTextType,
    info_type: ObsTextInfoType,
    word_wrap: bool,
}

impl TryFrom<PropertyCreationInfo> for ObsTextProperty {
    type Error = crate::utils::ObsError;

    fn try_from(
        PropertyCreationInfo {
            name,
            description,
            pointer,
            runtime,
        }: PropertyCreationInfo,
    ) -> Result<Self, Self::Error> {
        run_with_obs!(runtime, (pointer), move || {
            is_of_type_result!(Text, pointer)?;

            let info_type = get_enum!(pointer, text_info_type, ObsTextInfoType)?;
            let text_type = get_enum!(pointer, text_type, ObsTextType)?;

            let monospace = unsafe { libobs::obs_property_text_monospace(pointer) };
            let word_wrap = unsafe { libobs::obs_property_text_info_word_wrap(pointer) };

            Ok(ObsTextProperty {
                name,
                description,
                monospace,
                text_type,
                info_type,
                word_wrap,
            })
        })?
    }
}
