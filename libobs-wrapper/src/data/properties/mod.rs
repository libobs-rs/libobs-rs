mod enums;
mod macros;
pub mod prop_impl;
pub mod types;

use std::{collections::HashMap, ffi::CStr};

use libobs::obs_properties;
use macros::*;

pub use enums::*;
use num_traits::FromPrimitive;
use types::*;

use crate::{
    run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::Sendable,
    utils::{ObsError, ObsString},
};

#[derive(Debug, Clone)]
pub enum ObsProperty {
    /// A property that is not valid
    Invalid,
    /// A boolean property
    Bool,
    /// An integer property
    Int(ObsNumberProperty<i32>),
    /// A float property
    Float(ObsNumberProperty<f64>),
    /// A text property
    Text(ObsTextProperty),
    /// A path property
    Path(ObsPathProperty),
    /// A list property
    List(ObsListProperty),
    /// A color property
    Color(ObsColorProperty),
    /// A button property
    Button(ObsButtonProperty),
    /// A font property
    Font(ObsFontProperty),
    /// An editable list property
    EditableList(ObsEditableListProperty),
    /// A frame rate property
    FrameRate(ObsFrameRateProperty),
    /// A group property
    Group(ObsGroupProperty),
    /// A color alpha property
    ColorAlpha(ObsColorAlphaProperty),
}

pub trait ObsPropertyObjectPrivate {
    fn get_properties_raw(&self) -> Result<Sendable<*mut libobs::obs_properties_t>, ObsError>;
    fn get_properties_by_id_raw<T: Into<ObsString> + Sync + Send>(
        id: T,
        runtime: ObsRuntime,
    ) -> Result<Sendable<*mut libobs::obs_properties_t>, ObsError>;
}

pub(crate) fn get_properties_inner(
    properties_raw: Sendable<*mut obs_properties>,
    runtime: ObsRuntime,
) -> Result<HashMap<String, ObsProperty>, ObsError> {
    let properties_raw = properties_raw.clone();
    if properties_raw.0.is_null() {
        let ptr_clone = properties_raw.clone();
        run_with_obs!(runtime, (ptr_clone), move || {
            unsafe { libobs::obs_properties_destroy(ptr_clone) };
        })?;

        return Ok(HashMap::new());
    }

    run_with_obs!(runtime, (properties_raw), move || {
        let mut result = HashMap::new();
        let mut property = unsafe { libobs::obs_properties_first(properties_raw) };
        while !property.is_null() {
            let name = unsafe { libobs::obs_property_name(property) };
            let name = unsafe { CStr::from_ptr(name as _) };
            let name = name.to_string_lossy().to_string();

            let p_type = unsafe { libobs::obs_property_get_type(property) };

            #[cfg(target_family = "windows")]
            let p_type = ObsPropertyType::from_i32(p_type);

            #[cfg(not(target_family = "windows"))]
            let p_type = ObsPropertyType::from_u32(p_type);

            println!("Property: {:?}", name);
            match p_type {
                Some(p_type) => {
                    result.insert(name, unsafe { p_type.to_property_struct(property) });
                }
                None => {
                    result.insert(name, ObsProperty::Invalid);
                }
            }

            // Move to the next property
            unsafe { libobs::obs_property_next(&mut property) };
        }

        unsafe { libobs::obs_properties_destroy(properties_raw) };
        result
    })
}

/// This trait is implemented for all obs objects that can have properties
pub trait ObsPropertyObject: ObsPropertyObjectPrivate {
    /// Returns the properties of the object
    fn get_properties(&self) -> Result<HashMap<String, ObsProperty>, ObsError>;
    fn get_properties_by_id<T: Into<ObsString> + Sync + Send>(
        id: T,
        runtime: &ObsRuntime,
    ) -> Result<HashMap<String, ObsProperty>, ObsError> {
        let properties_raw = Self::get_properties_by_id_raw(id, runtime.clone())?;
        get_properties_inner(properties_raw, runtime.clone())
    }
}
