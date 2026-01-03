use super::PropertyCreationInfo;
use crate::{
    data::properties::{get_enum, unsafe_is_of_type_result, ObsComboFormat, ObsComboType},
    run_with_obs,
};
use getters0::Getters;
use std::ffi::CStr;

#[derive(Debug, Getters, Clone)]
#[skip_new]
pub struct ObsListProperty {
    name: String,
    description: Option<String>,
    list_type: ObsComboType,
    format: ObsComboFormat,
    items: Vec<ObsListItem>,
}

#[derive(Debug, Getters, Clone)]
#[skip_new]
pub struct ObsListItem {
    name: String,
    value: ObsListItemValue,
    disabled: bool,
}

#[derive(Debug, Clone)]
pub enum ObsListItemValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Invalid,
}

impl ObsListItem {
    fn new(name: String, value: ObsListItemValue, disabled: bool) -> Self {
        Self {
            name,
            value,
            disabled,
        }
    }
}

impl TryFrom<PropertyCreationInfo> for ObsListProperty {
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
            unsafe_is_of_type_result!(List, pointer)?;

            let list_type = get_enum!(pointer, list_type, ObsComboType)?;
            let format = get_enum!(pointer, list_format, ObsComboFormat)?;

            let count = unsafe {
                // Safety: Safe because of smart pointer
                libobs::obs_property_list_item_count(pointer.0)
            };
            let mut items = Vec::with_capacity(count);

            for i in 0..count {
                let list_item_name = unsafe {
                    // Safety: The caller must have ensured that the pointer is valid
                    libobs::obs_property_list_item_name(pointer.0, i)
                };

                if list_item_name.is_null() {
                    continue;
                }

                let list_name = unsafe {
                    // Safety: Safe because we did a null check
                    CStr::from_ptr(list_item_name)
                        .to_str()
                        .unwrap_or_default()
                        .to_string()
                };

                let is_disabled = unsafe {
                    // Safety: Safe because of smart pointer
                    libobs::obs_property_list_item_disabled(pointer.0, i)
                };
                let value = match format {
                    ObsComboFormat::Invalid => ObsListItemValue::Invalid,
                    ObsComboFormat::Int => {
                        let int_val = unsafe {
                            // Safety: The caller must have ensured that the pointer is valid
                            libobs::obs_property_list_item_int(pointer.0, i)
                        };
                        ObsListItemValue::Int(int_val)
                    }
                    ObsComboFormat::Float => {
                        let float_val = unsafe {
                            // Safety: The caller must have ensured that the pointer is valid
                            libobs::obs_property_list_item_float(pointer.0, i)
                        };
                        ObsListItemValue::Float(float_val)
                    }
                    ObsComboFormat::String => {
                        let item_string = unsafe {
                            // Safety: The caller must have ensured that the pointer is valid
                            libobs::obs_property_list_item_string(pointer.0, i)
                        };

                        if item_string.is_null() {
                            ObsListItemValue::String(String::new())
                        } else {
                            let string_val = unsafe {
                                // Safety: Safe because of null check
                                CStr::from_ptr(item_string).to_string_lossy().to_string()
                            };
                            ObsListItemValue::String(string_val)
                        }
                    }
                    ObsComboFormat::Bool => {
                        let bool_val = unsafe {
                            // Safety: The caller must have ensured that the pointer is valid
                            libobs::obs_property_list_item_bool(pointer.0, i)
                        };
                        ObsListItemValue::Bool(bool_val)
                    }
                };
                items.push(ObsListItem::new(list_name, value, is_disabled));
            }

            Ok(Self {
                name,
                description,
                list_type,
                format,
                items,
            })
        })?
    }
}
