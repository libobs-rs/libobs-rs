//! # Important Notice
//! All structs in this module use direct obs calls to get the data from the obs_property_t struct. **ALWAYS MAKE SURE THIS IS RUNNING ON THE OBS THREAD**

mod button;
impl_general_property!(Color);
mod editable_list;
impl_general_property!(Font);
impl_general_property!(FrameRate);
impl_general_property!(Group);
impl_general_property!(ColorAlpha);
mod list;
mod number;
mod path;
mod text;

pub(crate) struct PropertyCreationInfo {
    name: String,
    description: Option<String>,
    pointer: Sendable<*mut libobs::obs_property>,
    runtime: ObsRuntime,
}

impl PropertyCreationInfo {
    /// # Safety
    /// `pointer` must be non-null and borrowed from a live obs_properties_t tree.
    /// The caller must retain that tree, without mutation or destruction, until the
    /// returned info has been consumed by its TryFrom conversion. All native access
    /// and conversion must run on `runtime`'s actor (nested actor calls run inline).
    pub(crate) unsafe fn new(
        name: String,
        description: Option<String>,
        pointer: Sendable<*mut libobs::obs_property>,
        runtime: ObsRuntime,
    ) -> Self {
        Self {
            name,
            description,
            pointer,
            runtime,
        }
    }
}

use std::ffi::CStr;

pub use button::*;
pub use editable_list::*;
use libobs::obs_property;
pub use list::*;
pub use number::*;
pub use path::*;
pub use text::*;

use crate::{run_with_obs, runtime::ObsRuntime, unsafe_send::Sendable, utils::ObsError};

use super::{macros::impl_general_property, ObsProperty, ObsPropertyType};

impl ObsPropertyType {
    /// # Safety
    /// Run on `runtime`'s actor. `pointer` must belong to a live property tree retained
    /// without mutation or destruction throughout this call and all nested conversions.
    unsafe fn inner_to_property_struct(
        &self,
        runtime: &ObsRuntime,
        pointer: Sendable<*mut obs_property>,
    ) -> Result<ObsProperty, ObsError> {
        let (name, description) = run_with_obs!(runtime, (pointer), move || {
            let name = unsafe {
                // SAFETY: The caller retains the parent tree throughout this actor conversion.
                libobs::obs_property_name(pointer.0)
            };
            if name.is_null() {
                return Err(ObsError::NullPointer(Some(
                    "Property name pointer is null".to_string(),
                )));
            }

            let name = unsafe {
                // SAFETY: obs_property_name returns a tree-owned NUL-terminated string;
                // the caller retains the unchanged tree until this copy completes.
                CStr::from_ptr(name)
            };
            let name = name.to_string_lossy().to_string();

            let description = unsafe { libobs::obs_property_description(pointer.0) };
            let description = if description.is_null() {
                None
            } else {
                let description = unsafe {
                    // SAFETY: The non-null description is a tree-owned NUL-terminated string;
                    // the caller retains the unchanged tree until this copy completes.
                    CStr::from_ptr(description)
                };
                Some(description.to_string_lossy().to_string())
            };

            Ok((name, description))
        })??;

        let info = PropertyCreationInfo::new(
            name,
            description,
            pointer,
            runtime.clone(), //
        );

        let data = match self {
            ObsPropertyType::Invalid => ObsProperty::Invalid,
            ObsPropertyType::Bool => ObsProperty::Bool,
            ObsPropertyType::Int => ObsProperty::Int(ObsNumberProperty::<i32>::try_from(info)?),
            ObsPropertyType::Float => ObsProperty::Float(ObsNumberProperty::<f64>::try_from(info)?),
            ObsPropertyType::Text => ObsProperty::Text(ObsTextProperty::try_from(info)?),
            ObsPropertyType::Path => ObsProperty::Path(ObsPathProperty::try_from(info)?),
            ObsPropertyType::List => ObsProperty::List(ObsListProperty::try_from(info)?),
            ObsPropertyType::Color => ObsProperty::Color(ObsColorProperty::try_from(info)?),
            ObsPropertyType::Button => ObsProperty::Button(ObsButtonProperty::try_from(info)?),
            ObsPropertyType::Font => ObsProperty::Font(ObsFontProperty::try_from(info)?),
            ObsPropertyType::EditableList => {
                ObsProperty::EditableList(ObsEditableListProperty::try_from(info)?)
            }
            ObsPropertyType::FrameRate => {
                ObsProperty::FrameRate(ObsFrameRateProperty::try_from(info)?)
            }
            ObsPropertyType::Group => ObsProperty::Group(ObsGroupProperty::try_from(info)?),
            ObsPropertyType::ColorAlpha => {
                ObsProperty::ColorAlpha(ObsColorAlphaProperty::try_from(info)?)
            }
        };

        Ok(data)
    }

    /// # Safety
    /// Run on `runtime`'s actor. `pointer` is a non-null iterator borrow, not an owned
    /// reference; retain its parent property tree unchanged until conversion finishes.
    pub(in crate::data::properties) unsafe fn get_property_struct(
        &self,
        runtime: &ObsRuntime,
        pointer: Sendable<*mut obs_property>,
    ) -> Result<ObsProperty, ObsError> {
        self.inner_to_property_struct(runtime, pointer)
    }
}
