use getters0::Getters;

use crate::{
    data::properties::{get_enum, get_opt_str, macros::is_of_type_result, ObsButtonType},
    run_with_obs,
};

use super::PropertyCreationInfo;

#[derive(Debug, Getters, Clone)]
#[skip_new]
pub struct ObsButtonProperty {
    name: String,
    description: Option<String>,
    button_type: ObsButtonType,
    url: Option<String>,
}

impl TryFrom<PropertyCreationInfo> for ObsButtonProperty {
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
            is_of_type_result!(Button, pointer)?;

            let url = get_opt_str!(pointer, button_url);
            let button_type = get_enum!(pointer, button_type, ObsButtonType)?;

            Ok(Self {
                name,
                description,
                button_type,
                url,
            })
        })?
    }
}
