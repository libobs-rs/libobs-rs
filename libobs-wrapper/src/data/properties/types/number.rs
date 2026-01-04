use getters0::Getters;

use crate::data::properties::ObsNumberType;

#[derive(Debug, Getters, Clone)]
#[skip_new]
pub struct ObsNumberProperty<T>
where
    T: Clone + Copy + std::fmt::Debug,
{
    name: String,
    description: Option<String>,
    min: T,
    max: T,
    step: T,
    suffix: String,
    number_type: ObsNumberType,
}

macro_rules! impl_from_property {
    ($n_type: ident, $obs_number_name: ident) => {
        paste::paste! {
            impl TryFrom<super::PropertyCreationInfo> for ObsNumberProperty<[<$n_type>]> {
                type Error = crate::utils::ObsError;

                fn try_from(
                    crate::data::properties::PropertyCreationInfo {
                        name,
                        description,
                        pointer,
                        runtime,
                    }: crate::data::properties::PropertyCreationInfo,
                ) -> Result<Self, Self::Error> {
                    $crate::run_with_obs!(runtime, (pointer), move || {
                        use crate::data::properties::ObsNumberType;

                        let min = unsafe {
                            // Safety: The caller must have ensured that the pointer is valid
                            libobs::[<obs_property_ $obs_number_name _min>](pointer.0)
                        };

                        let max = unsafe {
                            // Safety: The caller must have ensured that the pointer is valid
                            libobs::[<obs_property_ $obs_number_name _max>](pointer.0)
                        };

                        let step = unsafe {
                            // Safety: The caller must have ensured that the pointer is valid
                            libobs::[<obs_property_ $obs_number_name _step>](pointer.0)
                        };

                        let suffix = unsafe {
                            // Safety: The caller must have ensured that the pointer is valid
                            libobs::[<obs_property_ $obs_number_name _suffix>](pointer.0)
                        };

                        let suffix = if suffix.is_null() {
                            String::new()
                        } else {
                            let suffix = unsafe {
                                // Safety: Safe because of we did a null check
                                std::ffi::CStr::from_ptr(suffix)
                            };

                            let suffix = suffix.to_str().unwrap_or_default();
                            suffix.to_string()
                        };

                        let number_type = unsafe {
                            // Safety: The caller must have ensured that the pointer is valid
                            libobs::[<obs_property_ $obs_number_name _type >](pointer.0)
                        };

                        let number_type = crate::macros::enum_from_number!(ObsNumberType, number_type);

                        if number_type.is_none() {
                            return Err(crate::utils::ObsError::EnumConversionError(format!(
                                "ObsNumberType for property {}",
                                name
                            )));
                        }

                        Ok(ObsNumberProperty {
                            name,
                            description,
                            min,
                            max,
                            step,
                            suffix,
                            number_type: number_type.unwrap(),
                        })
                    })?
                }
            }
        }
    };
}

impl_from_property!(i32, int);
impl_from_property!(f64, float);
