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
            impl From<super::PropertyCreationInfo> for ObsNumberProperty<[<$n_type>]> {
                fn from(
                    super::PropertyCreationInfo {
                        name,
                        description,
                        pointer,
                    }: super::PropertyCreationInfo,
                ) -> Self {
                    use crate::data::properties::ObsNumberType;
                    use num_traits::FromPrimitive;

                    let min = unsafe { libobs::[<obs_property_ $obs_number_name _min>](pointer) };
                    let max = unsafe { libobs::[<obs_property_ $obs_number_name _max>](pointer) };
                    let step = unsafe { libobs::[<obs_property_ $obs_number_name _step>](pointer)};

                    let suffix = unsafe { libobs::[<obs_property_ $obs_number_name _suffix>](pointer) };
                    let suffix = if suffix.is_null() {
                        String::new()
                    } else {
                        let suffix = unsafe { std::ffi::CStr::from_ptr(suffix) };
                        let suffix = suffix.to_str().unwrap_or_default();
                        suffix.to_string()
                    };

                    let number_type = unsafe { libobs::[<obs_property_ $obs_number_name _type >](pointer) };
                    #[cfg(target_family="windows")]
                    let number_type = ObsNumberType::from_i32(number_type);
                    #[cfg(not(target_family="windows"))]
                    let number_type = ObsNumberType::from_u32(number_type);
                    if number_type.is_none() {
                        panic!("Invalid number type got none");
                    }

                    return ObsNumberProperty {
                        name,
                        description,
                        min,
                        max,
                        step,
                        suffix,
                        number_type: number_type.unwrap(),
                    };
                }
            }
        }
    };
}

impl_from_property!(i32, int);
impl_from_property!(f64, float);
