macro_rules! unsafe_is_of_type_result {
    ($prop_type: ident, $name: ident) => {{
        {
            use crate::data::properties::ObsPropertyType;

            let p_type = unsafe {
                // Safety: The caller must have ensured that the pointer is valid
                libobs::obs_property_get_type($name.0)
            };
            let p_type = crate::macros::enum_from_number!(ObsPropertyType, p_type);

            if p_type.is_none_or(|e| !matches!(e, ObsPropertyType::$prop_type)) {
                Err(crate::utils::ObsError::InvocationError(format!(
                    "Property is not of type {}",
                    stringify!($prop_type)
                )))
            } else {
                Ok(())
            }
        }
    }};
}

macro_rules! impl_general_property {
    ($type: ident) => {
        paste::paste! {
            #[derive(Debug, getters0::Getters, Clone)]
            #[skip_new]
            pub struct [<Obs $type Property>] {
                name: String,
                description: Option<String>
            }
            impl TryFrom<crate::data::properties::PropertyCreationInfo> for [<Obs $type Property>] {
                type Error = crate::utils::ObsError;

                fn try_from(
                    crate::data::properties::PropertyCreationInfo {
                        name,
                        description,
                        pointer,
                        runtime
                    }: crate::data::properties::PropertyCreationInfo,
                ) -> Result<Self, Self::Error> {
                    crate::run_with_obs!(runtime, (pointer), move || {
                        crate::data::properties::unsafe_is_of_type_result!($type, pointer)?;
                        Ok(())
                    })??;

                    Ok(Self { name, description })
                }
            }
        }
    };
}

macro_rules! get_enum {
    ($pointer_name: ident, $name: ident, $enum_name: ident) => {
        paste::paste! {
            {
                let v = unsafe {
                    // Safety: The caller must have ensured that the pointer is valid
                    libobs::[<obs_property_ $name>]($pointer_name.0)
                };

                crate::macros::enum_from_number!($enum_name, v)
                    .ok_or_else(|| {
                        crate::utils::ObsError::EnumConversionError(format!(
                            "Failed to convert {} to enum {}",
                            stringify!($name),
                            stringify!($enum_name)
                        ))
                    })
            }
        }
    };
}

macro_rules! get_opt_str {
    ($pointer_name: ident, $name: ident) => {{
        paste::paste! {
            let v = libobs::[<obs_property_ $name>]($pointer_name.0);
        }
        if v.is_null() {
            None
        } else {
            #[expect(unused_unsafe)]
            let v = unsafe {
                // Safety: The function didn't return a null pointer, so it must be valid
                std::ffi::CStr::from_ptr(v as _)
            };
            let v = v.to_string_lossy().to_string();
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        }
    }};
}

pub(super) use get_enum;
pub(super) use get_opt_str;
pub(super) use impl_general_property;
pub(super) use unsafe_is_of_type_result;
