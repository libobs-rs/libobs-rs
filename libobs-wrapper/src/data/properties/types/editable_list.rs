use getters0::Getters;

use crate::{
    data::properties::{get_enum, get_opt_str, unsafe_is_of_type_result, ObsEditableListType},
    run_with_obs,
};

use super::PropertyCreationInfo;

#[derive(Debug, Getters, Clone)]
#[skip_new]
pub struct ObsEditableListProperty {
    name: String,
    description: Option<String>,
    list_type: ObsEditableListType,
    filter: String,
    default_path: String,
}

impl TryFrom<PropertyCreationInfo> for ObsEditableListProperty {
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
            unsafe_is_of_type_result!(EditableList, pointer)?;

            let list_type = get_enum!(pointer, list_type, ObsEditableListType)?;
            let filter = unsafe {
                // Safety: The pointer must be valid because of the unsafe new method of PropertyCreationInfo
                get_opt_str!(pointer, path_filter)
            }
            .unwrap_or_default();
            let default_path = unsafe {
                // Safety: The pointer must be valid because of the unsafe new method of PropertyCreationInfo
                get_opt_str!(pointer, path_default_path)
            }
            .unwrap_or_default();

            Ok(Self {
                name,
                description,
                list_type,
                filter,
                default_path,
            })
        })?
    }
}
