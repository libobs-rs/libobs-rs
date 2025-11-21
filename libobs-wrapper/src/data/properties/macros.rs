macro_rules! assert_type {
    ($prop_type: ident, $name: ident) => {{
        use crate::data::properties::ObsPropertyType;
        use num_traits::FromPrimitive;

        let p_type = unsafe { libobs::obs_property_get_type($name) };

        #[cfg(target_family = "windows")]
        let p_type = ObsPropertyType::from_i32(p_type);
        #[cfg(not(target_family = "windows"))]
        let p_type = ObsPropertyType::from_u32(p_type);

        if p_type.is_none_or(|e| !matches!(e, ObsPropertyType::$prop_type)) {
            panic!(
                "Invalid property type: expected {:?}, got {:?}",
                ObsPropertyType::$prop_type,
                p_type
            );
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
            impl From<crate::data::properties::PropertyCreationInfo> for [<Obs $type Property>] {
                fn from(
                    crate::data::properties::PropertyCreationInfo {
                        name,
                        description,
                        pointer,
                    }: crate::data::properties::PropertyCreationInfo,
                ) -> Self {
                    crate::data::properties::assert_type!($type, pointer);
                    Self { name, description }
                }
            }
        }
    };
}

macro_rules! get_enum {
    ($pointer_name: ident, $name: ident, $enum_name: ident) => {
        paste::paste! {
            {
                use num_traits::FromPrimitive;
                let v = unsafe { libobs::[<obs_property_ $name>]($pointer_name) };

                #[cfg(target_family="windows")]
                let v = $enum_name::from_i32(v);

                #[cfg(not(target_family="windows"))]
                let v = $enum_name::from_u32(v);

                if v.is_none() {
                    panic!("Invalid {} type got none", stringify!($name));
                }

                v.unwrap()
            }
        }
    };
}

macro_rules! get_opt_str {
    ($pointer_name: ident, $name: ident) => {{
        paste::paste! {
            let v = unsafe { libobs::[<obs_property_ $name>]($pointer_name) };
        }
        if v.is_null() {
            None
        } else {
            let v = unsafe { std::ffi::CStr::from_ptr(v as _) };
            let v = v.to_str().expect("OBS returned invalid string").to_string();
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        }
    }};
}

pub(super) use assert_type;
pub(super) use get_enum;
pub(super) use get_opt_str;
pub(super) use impl_general_property;
