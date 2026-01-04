mod enums;
mod macros;
pub mod prop_impl;
pub mod types;

use std::{collections::HashMap, ffi::CStr};

use libobs::obs_properties;
use macros::*;

pub use enums::*;
use types::*;

use crate::{
    impl_obs_drop, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::{Sendable, SmartPointerSendable},
    utils::{ObsDropGuard, ObsError, ObsString},
};

#[derive(Debug)]
pub(crate) struct _ObsPropertiesDropGuard {
    properties: Sendable<*mut obs_properties>,
    runtime: ObsRuntime,
}

impl ObsDropGuard for _ObsPropertiesDropGuard {}

impl_obs_drop!(_ObsPropertiesDropGuard, (properties), move || unsafe {
    // Safety: The pointer is valid as long as we are in the runtime and the guard is alive.
    libobs::obs_properties_destroy(properties.0);
});

impl _ObsPropertiesDropGuard {
    pub(crate) fn new(properties: Sendable<*mut obs_properties>, runtime: ObsRuntime) -> Self {
        Self {
            properties,
            runtime,
        }
    }
}

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
    fn get_properties_raw(
        &self,
    ) -> Result<SmartPointerSendable<*mut libobs::obs_properties_t>, ObsError>;
    fn get_properties_by_id_raw<T: Into<ObsString> + Sync + Send>(
        id: T,
        runtime: ObsRuntime,
    ) -> Result<SmartPointerSendable<*mut libobs::obs_properties_t>, ObsError>;
}

pub(crate) fn property_ptr_to_struct(
    properties_raw: SmartPointerSendable<*mut obs_properties>,
    runtime: ObsRuntime,
) -> Result<HashMap<String, ObsProperty>, ObsError> {
    let runtime_clone = runtime.clone();
    run_with_obs!(runtime, (properties_raw, runtime_clone), move || {
        let mut result = HashMap::new();
        let mut property = unsafe {
            // Safety: Safe because of smart pointer
            libobs::obs_properties_first(properties_raw.get_ptr())
        };
        while !property.is_null() {
            let name = unsafe { libobs::obs_property_name(property) };
            if name.is_null() {
                let success = unsafe {
                    // Safety: Safe because property is not null and we are just moving forward.
                    libobs::obs_property_next(&mut property)
                };

                if !success {
                    break;
                }
                continue;
            }

            let name = unsafe {
                // Safety: Safe because of we did a null check
                CStr::from_ptr(name as _)
            };
            let name = name.to_string_lossy().to_string();

            let p_type = unsafe {
                // Safety: Safe because we just got the property pointer
                libobs::obs_property_get_type(property)
            };

            let p_type = crate::macros::enum_from_number!(ObsPropertyType, p_type);

            log::trace!("Property: {:?}", name);
            match p_type {
                Some(p_type) => {
                    let prop_struct = unsafe {
                        // Safety: Safe because we just got the property pointer
                        p_type.get_property_struct(&runtime_clone, Sendable(property))
                    };
                    if let Ok(r) = prop_struct {
                        result.insert(name, r);
                    }
                }
                None => {
                    result.insert(name, ObsProperty::Invalid);
                }
            }

            // Move to the next property
            let has_next = unsafe {
                // Safety: We didn't drop the property, so it is still valid and we can proceed
                libobs::obs_property_next(&mut property)
            };

            if !has_next {
                break;
            }
        }

        result
    })
}

/// This trait is implemented for all obs objects that can have properties
pub trait ObsPropertyObject: ObsPropertyObjectPrivate {
    /// Returns the properties of the object
    fn get_properties(&self) -> Result<HashMap<String, ObsProperty>, ObsError>;
    fn get_properties_by_source_id<T: Into<ObsString> + Sync + Send>(
        id: T,
        runtime: &ObsRuntime,
    ) -> Result<HashMap<String, ObsProperty>, ObsError> {
        let properties_raw = Self::get_properties_by_id_raw(id, runtime.clone())?;
        property_ptr_to_struct(properties_raw, runtime.clone())
    }
}
