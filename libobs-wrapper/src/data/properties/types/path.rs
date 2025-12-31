use getters0::Getters;

use crate::{
    data::properties::{get_enum, get_opt_str, macros::is_of_type_result, ObsPathType},
    run_with_obs,
};

use super::PropertyCreationInfo;

#[derive(Debug, Getters, Clone)]
#[skip_new]
pub struct ObsPathProperty {
    name: String,
    description: Option<String>,
    path_type: ObsPathType,
    filter: String,
    default_path: String,
}

impl TryFrom<PropertyCreationInfo> for ObsPathProperty {
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
            is_of_type_result!(Path, pointer)?;

            let path_type = get_enum!(pointer, path_type, ObsPathType)?;
            let filter = get_opt_str!(pointer, path_filter).unwrap_or_default();
            let default_path = get_opt_str!(pointer, path_default_path).unwrap_or_default();
            Ok(Self {
                name,
                description,
                path_type,
                filter,
                default_path,
            })
        })?
    }
}
