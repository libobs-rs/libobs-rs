//! Generic, plugin-agnostic settings schemas built from OBS property metadata.
//!
//! This module is the safe bridge between runtime-discovered [`crate::capabilities::PropertyMetadata`]
//! and [`crate::data::ObsData`]. It deliberately models values independently from concrete OBS plugins,
//! so applications can build configuration UIs or remote-control surfaces without hard-coding plugin IDs.
//!
//! Scalar values, combo lists, frame-rate values/options, editable lists, fonts, colors, and checkable
//! groups are represented as [`PropertyValue`]. Button properties are intentionally metadata/actions,
//! not data values; triggering a property button requires the concrete OBS object that owns the action.
//! Use [`SettingsSnapshot`] when a UI needs dynamic metadata plus current/default values together.

use std::ffi::CStr;

use crate::{
    capabilities::{
        FrameRate, GroupType, ListFormat, ListItem, ListType, ListValue, PropertyKind,
        PropertyMetadata,
    },
    data::{ImmutableObsData, ObsData, ObsDataGetters, ObsDataPointers, ObsDataSetters},
    run_with_obs,
    utils::{ObsError, ObsString},
};

/// A generic value that can be read from or applied to an OBS property.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    FrameRate(FrameRateSetting),
    EditableList(Vec<EditableListEntry>),
    Font(FontSetting),
}

impl PropertyValue {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Boolean(_) => "boolean",
            Self::Integer(_) => "integer",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::FrameRate(_) => "frame-rate",
            Self::EditableList(_) => "editable-list",
            Self::Font(_) => "font",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameRateSetting {
    pub frame_rate: FrameRate,
    pub option: Option<String>,
}

impl From<FrameRate> for PropertyValue {
    fn from(frame_rate: FrameRate) -> Self {
        Self::FrameRate(FrameRateSetting {
            frame_rate,
            option: None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditableListEntry {
    pub value: String,
    pub uuid: Option<String>,
    pub selected: bool,
    pub hidden: bool,
}

impl EditableListEntry {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            uuid: None,
            selected: false,
            hidden: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontSetting {
    pub face: String,
    pub style: String,
    pub size: i64,
    pub flags: u32,
}

fn list_value_to_property_value(value: &ListValue) -> Option<PropertyValue> {
    match value {
        ListValue::String(value) => Some(PropertyValue::String(value.clone())),
        ListValue::Int(value) => Some(PropertyValue::Integer(*value)),
        ListValue::Float(value) => Some(PropertyValue::Float(*value)),
        ListValue::Bool(value) => Some(PropertyValue::Boolean(*value)),
        ListValue::UnknownFormat(_) => None,
    }
}

/// Current/default values associated with one runtime-discovered property. Action-only properties
/// such as buttons intentionally have no data value while retaining their metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyState {
    pub metadata: PropertyMetadata,
    pub current_value: Option<PropertyValue>,
    pub default_value: Option<PropertyValue>,
}

/// A form-ready settings view containing dynamic metadata plus current/default values.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsSnapshot {
    schema: SettingsSchema,
    states: Vec<PropertyState>,
}

impl SettingsSnapshot {
    pub fn schema(&self) -> &SettingsSchema {
        &self.schema
    }

    pub fn states(&self) -> &[PropertyState] {
        &self.states
    }

    pub fn state(&self, name: &str) -> Option<&PropertyState> {
        self.states.iter().find(|state| state.metadata.name == name)
    }
}

/// An owned, recursively searchable snapshot of an OBS property tree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsSchema {
    properties: Vec<PropertyMetadata>,
}

impl SettingsSchema {
    pub fn new(properties: Vec<PropertyMetadata>) -> Self {
        Self { properties }
    }

    pub fn properties(&self) -> &[PropertyMetadata] {
        &self.properties
    }

    /// Finds a property by its OBS name, recursively descending through property groups.
    pub fn property(&self, name: &str) -> Option<&PropertyMetadata> {
        find_property(&self.properties, name)
    }

    /// Validates a value against the runtime-discovered metadata without mutating OBS data.
    pub fn validate(&self, name: &str, value: &PropertyValue) -> Result<(), ObsError> {
        let property = self
            .property(name)
            .ok_or_else(|| ObsError::PropertyNotFound {
                name: name.to_owned(),
            })?;
        validate_property_value(property, value)
    }

    /// Validates and applies one scalar property value to an [`ObsData`] object.
    pub fn set(
        &self,
        settings: &mut ObsData,
        name: &str,
        value: PropertyValue,
    ) -> Result<(), ObsError> {
        self.validate(name, &value)?;
        match value {
            PropertyValue::Boolean(value) => {
                settings.set_bool(name, value)?;
            }
            PropertyValue::Integer(value) => {
                settings.set_int(name, value)?;
            }
            PropertyValue::Float(value) => {
                settings.set_double(name, value)?;
            }
            PropertyValue::String(value) => {
                settings.set_string(name, value)?;
            }
            PropertyValue::FrameRate(value) => {
                set_frame_rate(settings, name, value)?;
            }
            PropertyValue::EditableList(entries) => {
                set_editable_list(settings, name, entries)?;
            }
            PropertyValue::Font(font) => {
                set_font(settings, name, font)?;
            }
        }
        Ok(())
    }

    /// Reads a scalar property using the value category declared by this schema.
    pub fn value(&self, settings: &ObsData, name: &str) -> Result<Option<PropertyValue>, ObsError> {
        let property = self
            .property(name)
            .ok_or_else(|| ObsError::PropertyNotFound {
                name: name.to_owned(),
            })?;
        read_property_value(settings, property)
    }

    /// Joins this dynamic schema with current and default values into one form-ready snapshot.
    pub fn snapshot(
        &self,
        current: &ObsData,
        defaults: Option<&ImmutableObsData>,
    ) -> Result<SettingsSnapshot, ObsError> {
        if let Some(defaults) = defaults {
            current.runtime().ensure_same_runtime(defaults.runtime())?;
        }
        let mut states = Vec::new();
        collect_property_states(&self.properties, current, defaults, &mut states)?;
        Ok(SettingsSnapshot {
            schema: self.clone(),
            states,
        })
    }
}

impl PropertyMetadata {
    /// Returns all enabled values advertised by a list property in their generic typed form.
    pub fn enabled_list_values(&self) -> Vec<PropertyValue> {
        match &self.kind {
            PropertyKind::List { items, .. } => items
                .iter()
                .filter(|item| !item.disabled)
                .filter_map(|item| list_value_to_property_value(&item.value))
                .collect(),
            _ => Vec::new(),
        }
    }
}

fn collect_property_states(
    properties: &[PropertyMetadata],
    current: &ObsData,
    defaults: Option<&ImmutableObsData>,
    states: &mut Vec<PropertyState>,
) -> Result<(), ObsError> {
    for property in properties {
        states.push(PropertyState {
            metadata: property.clone(),
            current_value: read_property_value(current, property)?,
            default_value: match defaults {
                Some(defaults) => read_property_value(defaults, property)?,
                None => None,
            },
        });
        if let PropertyKind::Group { properties, .. } = &property.kind {
            collect_property_states(properties, current, defaults, states)?;
        }
    }
    Ok(())
}

fn find_property<'a>(
    properties: &'a [PropertyMetadata],
    name: &str,
) -> Option<&'a PropertyMetadata> {
    for property in properties {
        if property.name == name {
            return Some(property);
        }
        if let PropertyKind::Group { properties, .. } = &property.kind {
            if let Some(found) = find_property(properties, name) {
                return Some(found);
            }
        }
    }
    None
}

fn expected_kind_name(kind: &PropertyKind) -> &'static str {
    match kind {
        PropertyKind::Bool => "boolean",
        PropertyKind::Integer { .. } | PropertyKind::Color | PropertyKind::ColorAlpha => "integer",
        PropertyKind::Float { .. } => "float",
        PropertyKind::Text { .. } | PropertyKind::Path { .. } => "string",
        PropertyKind::List { format, .. } => match format {
            ListFormat::String => "string",
            ListFormat::Int => "integer",
            ListFormat::Float => "float",
            ListFormat::Bool => "boolean",
            _ => "list value",
        },
        PropertyKind::FrameRate { .. } => "frame-rate",
        PropertyKind::EditableList { .. } => "editable-list",
        PropertyKind::Font => "font",
        PropertyKind::Group {
            group_type: GroupType::Checkable,
            ..
        } => "boolean",
        PropertyKind::Group { .. } => "non-value/structural",
        PropertyKind::Invalid | PropertyKind::Button { .. } | PropertyKind::Unknown(_) => {
            "non-value/action"
        }
    }
}

fn type_mismatch(property: &PropertyMetadata, value: &PropertyValue) -> ObsError {
    ObsError::PropertyValueTypeMismatch {
        name: property.name.clone(),
        expected: expected_kind_name(&property.kind).to_owned(),
        actual: value.kind_name().to_owned(),
    }
}

fn validate_property_value(
    property: &PropertyMetadata,
    value: &PropertyValue,
) -> Result<(), ObsError> {
    match (&property.kind, value) {
        (PropertyKind::Bool, PropertyValue::Boolean(_))
        | (
            PropertyKind::Group {
                group_type: GroupType::Checkable,
                ..
            },
            PropertyValue::Boolean(_),
        )
        | (PropertyKind::Text { .. }, PropertyValue::String(_))
        | (PropertyKind::Path { .. }, PropertyValue::String(_))
        | (PropertyKind::Color, PropertyValue::Integer(_))
        | (PropertyKind::ColorAlpha, PropertyValue::Integer(_)) => Ok(()),
        (PropertyKind::Integer { min, max, step, .. }, PropertyValue::Integer(value)) => {
            let min = i64::from(*min);
            let max = i64::from(*max);
            if *value < min || *value > max {
                return Err(ObsError::PropertyValueOutOfRange {
                    name: property.name.clone(),
                    value: value.to_string(),
                    min: min.to_string(),
                    max: max.to_string(),
                });
            }
            if *step > 0 && (*value - min) % i64::from(*step) != 0 {
                return Err(ObsError::PropertyValueNotAllowed {
                    name: property.name.clone(),
                    value: value.to_string(),
                });
            }
            Ok(())
        }
        (PropertyKind::Float { min, max, .. }, PropertyValue::Float(value)) => {
            if !value.is_finite() || *value < *min || *value > *max {
                return Err(ObsError::PropertyValueOutOfRange {
                    name: property.name.clone(),
                    value: value.to_string(),
                    min: min.to_string(),
                    max: max.to_string(),
                });
            }
            Ok(())
        }
        (
            PropertyKind::List {
                list_type,
                format,
                items,
            },
            value,
        ) => validate_list_value(property, *list_type, *format, items, value),
        (PropertyKind::FrameRate { options, ranges }, PropertyValue::FrameRate(value)) => {
            if let Some(option) = value.option.as_deref() {
                if options
                    .iter()
                    .any(|candidate| candidate.name.as_deref() == Some(option))
                {
                    return Ok(());
                }
                return Err(ObsError::PropertyValueNotAllowed {
                    name: property.name.clone(),
                    value: option.to_owned(),
                });
            }

            let fps = value.frame_rate;
            if fps.denominator == 0 {
                return Err(ObsError::PropertyValueNotAllowed {
                    name: property.name.clone(),
                    value: format!("{}/{}", fps.numerator, fps.denominator),
                });
            }
            if ranges.is_empty()
                || ranges
                    .iter()
                    .any(|range| frame_rate_in_range(fps, range.min, range.max))
            {
                Ok(())
            } else {
                Err(ObsError::PropertyValueNotAllowed {
                    name: property.name.clone(),
                    value: format!("{}/{}", fps.numerator, fps.denominator),
                })
            }
        }
        (PropertyKind::EditableList { .. }, PropertyValue::EditableList(_))
        | (PropertyKind::Font, PropertyValue::Font(_)) => Ok(()),
        (
            PropertyKind::Invalid
            | PropertyKind::Button { .. }
            | PropertyKind::Group { .. }
            | PropertyKind::Unknown(_),
            _,
        ) => Err(ObsError::PropertyValueUnsupported {
            name: property.name.clone(),
            property_type: format!("{:?}", property.kind),
        }),
        _ => Err(type_mismatch(property, value)),
    }
}

fn validate_list_value(
    property: &PropertyMetadata,
    list_type: ListType,
    format: ListFormat,
    items: &[ListItem],
    value: &PropertyValue,
) -> Result<(), ObsError> {
    let type_matches = matches!(
        (format, value),
        (ListFormat::String, PropertyValue::String(_))
            | (ListFormat::Int, PropertyValue::Integer(_))
            | (ListFormat::Float, PropertyValue::Float(_))
            | (ListFormat::Bool, PropertyValue::Boolean(_))
    );
    if !type_matches {
        return Err(type_mismatch(property, value));
    }

    if matches!(list_type, ListType::Editable) {
        return Ok(());
    }
    if items
        .iter()
        .filter(|item| !item.disabled)
        .filter_map(|item| list_value_to_property_value(&item.value))
        .any(|item| item == *value)
    {
        Ok(())
    } else {
        Err(ObsError::PropertyValueNotAllowed {
            name: property.name.clone(),
            value: format!("{value:?}"),
        })
    }
}

fn frame_rate_in_range(value: FrameRate, min: FrameRate, max: FrameRate) -> bool {
    let value = value.numerator as f64 / value.denominator as f64;
    let min = min.numerator as f64 / min.denominator.max(1) as f64;
    let max = max.numerator as f64 / max.denominator.max(1) as f64;
    value >= min && value <= max
}

fn set_frame_rate(settings: &ObsData, name: &str, value: FrameRateSetting) -> Result<(), ObsError> {
    let name = ObsString::new(name);
    let data = settings.as_ptr();
    run_with_obs!(settings.runtime(), (name, data, value), move || unsafe {
        // Safety: managed data/name stay alive for the actor call; libobs copies the rate and
        // optional string synchronously.
        let fps = libobs::media_frames_per_second {
            numerator: value.frame_rate.numerator,
            denominator: value.frame_rate.denominator,
        };
        let option = value.option.as_deref().map(ObsString::new);
        libobs::obs_data_set_frames_per_second(
            data.get_ptr(),
            name.as_ptr().0,
            fps,
            option
                .as_ref()
                .map_or(std::ptr::null(), |option| option.as_ptr().0),
        );
    })
}

fn set_editable_list(
    settings: &ObsData,
    name: &str,
    entries: Vec<EditableListEntry>,
) -> Result<(), ObsError> {
    let name = ObsString::new(name);
    let data = settings.as_ptr();
    run_with_obs!(settings.runtime(), (name, data, entries), move || unsafe {
        // Safety: all temporary obs_data/array objects are owned within this actor command. The
        // destination data object takes its own reference when the array is assigned.
        let array = libobs::obs_data_array_create();
        if array.is_null() {
            return Err(ObsError::NullPointer(Some(
                "creating editable-list data array".into(),
            )));
        }
        let value_key = ObsString::new("value");
        let uuid_key = ObsString::new("uuid");
        let selected_key = ObsString::new("selected");
        let hidden_key = ObsString::new("hidden");
        for entry in entries {
            let item = libobs::obs_data_create();
            if item.is_null() {
                libobs::obs_data_array_release(array);
                return Err(ObsError::NullPointer(Some(
                    "creating editable-list item".into(),
                )));
            }
            let value = ObsString::new(entry.value);
            libobs::obs_data_set_string(item, value_key.as_ptr().0, value.as_ptr().0);
            if let Some(uuid) = entry.uuid {
                let uuid = ObsString::new(uuid);
                libobs::obs_data_set_string(item, uuid_key.as_ptr().0, uuid.as_ptr().0);
            }
            libobs::obs_data_set_bool(item, selected_key.as_ptr().0, entry.selected);
            libobs::obs_data_set_bool(item, hidden_key.as_ptr().0, entry.hidden);
            libobs::obs_data_array_push_back(array, item);
            libobs::obs_data_release(item);
        }
        libobs::obs_data_set_array(data.get_ptr(), name.as_ptr().0, array);
        libobs::obs_data_array_release(array);
        Ok(())
    })?
}

fn set_font(settings: &ObsData, name: &str, font: FontSetting) -> Result<(), ObsError> {
    let name = ObsString::new(name);
    let data = settings.as_ptr();
    run_with_obs!(settings.runtime(), (name, data, font), move || unsafe {
        // Safety: the temporary font object lives until `obs_data_set_obj` takes its own reference.
        let object = libobs::obs_data_create();
        if object.is_null() {
            return Err(ObsError::NullPointer(Some(
                "creating font settings object".into(),
            )));
        }
        let face_key = ObsString::new("face");
        let style_key = ObsString::new("style");
        let size_key = ObsString::new("size");
        let flags_key = ObsString::new("flags");
        let face = ObsString::new(font.face);
        let style = ObsString::new(font.style);
        libobs::obs_data_set_string(object, face_key.as_ptr().0, face.as_ptr().0);
        libobs::obs_data_set_string(object, style_key.as_ptr().0, style.as_ptr().0);
        libobs::obs_data_set_int(object, size_key.as_ptr().0, font.size);
        libobs::obs_data_set_int(object, flags_key.as_ptr().0, i64::from(font.flags));
        libobs::obs_data_set_obj(data.get_ptr(), name.as_ptr().0, object);
        libobs::obs_data_release(object);
        Ok(())
    })?
}

fn read_frame_rate<T>(settings: &T, name: &str) -> Result<Option<PropertyValue>, ObsError>
where
    T: ObsDataPointers,
{
    let name = ObsString::new(name);
    let data = settings.as_ptr();
    run_with_obs!(settings.runtime(), (name, data), move || unsafe {
        // Safety: libobs writes only stack-local output values while the managed data handle lives.
        let mut fps = libobs::media_frames_per_second {
            numerator: 0,
            denominator: 0,
        };
        let mut option = std::ptr::null();
        if !libobs::obs_data_get_frames_per_second(
            data.get_ptr(),
            name.as_ptr().0,
            &mut fps,
            &mut option,
        ) {
            return None;
        }
        let option = if option.is_null() {
            None
        } else {
            Some(CStr::from_ptr(option).to_string_lossy().into_owned())
        };
        Some(PropertyValue::FrameRate(FrameRateSetting {
            frame_rate: FrameRate {
                numerator: fps.numerator,
                denominator: fps.denominator,
            },
            option,
        }))
    })
}

fn read_editable_list<T>(settings: &T, name: &str) -> Result<Option<PropertyValue>, ObsError>
where
    T: ObsDataPointers,
{
    let name = ObsString::new(name);
    let data = settings.as_ptr();
    run_with_obs!(settings.runtime(), (name, data), move || unsafe {
        // Safety: `obs_data_get_array`/`obs_data_array_item` return owned references which are
        // released before leaving the actor command. All strings are copied into Rust values.
        let array = libobs::obs_data_get_array(data.get_ptr(), name.as_ptr().0);
        if array.is_null() {
            return None;
        }
        let value_key = ObsString::new("value");
        let uuid_key = ObsString::new("uuid");
        let selected_key = ObsString::new("selected");
        let hidden_key = ObsString::new("hidden");
        let mut entries = Vec::with_capacity(libobs::obs_data_array_count(array));
        for index in 0..libobs::obs_data_array_count(array) {
            let item = libobs::obs_data_array_item(array, index);
            if item.is_null() {
                continue;
            }
            let value_ptr = libobs::obs_data_get_string(item, value_key.as_ptr().0);
            let uuid_ptr = libobs::obs_data_get_string(item, uuid_key.as_ptr().0);
            let value = if value_ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(value_ptr).to_string_lossy().into_owned()
            };
            let uuid = if uuid_ptr.is_null() {
                None
            } else {
                let uuid = CStr::from_ptr(uuid_ptr).to_string_lossy().into_owned();
                (!uuid.is_empty()).then_some(uuid)
            };
            entries.push(EditableListEntry {
                value,
                uuid,
                selected: libobs::obs_data_get_bool(item, selected_key.as_ptr().0),
                hidden: libobs::obs_data_get_bool(item, hidden_key.as_ptr().0),
            });
            libobs::obs_data_release(item);
        }
        libobs::obs_data_array_release(array);
        Some(PropertyValue::EditableList(entries))
    })
}

fn read_font<T>(settings: &T, name: &str) -> Result<Option<PropertyValue>, ObsError>
where
    T: ObsDataPointers,
{
    let name = ObsString::new(name);
    let data = settings.as_ptr();
    run_with_obs!(settings.runtime(), (name, data), move || unsafe {
        // Safety: `obs_data_get_obj` returns an owned reference; fields are copied before release.
        let object = libobs::obs_data_get_obj(data.get_ptr(), name.as_ptr().0);
        if object.is_null() {
            return None;
        }
        let face_key = ObsString::new("face");
        let style_key = ObsString::new("style");
        let size_key = ObsString::new("size");
        let flags_key = ObsString::new("flags");
        let face_ptr = libobs::obs_data_get_string(object, face_key.as_ptr().0);
        let style_ptr = libobs::obs_data_get_string(object, style_key.as_ptr().0);
        let face = if face_ptr.is_null() {
            String::new()
        } else {
            CStr::from_ptr(face_ptr).to_string_lossy().into_owned()
        };
        let style = if style_ptr.is_null() {
            String::new()
        } else {
            CStr::from_ptr(style_ptr).to_string_lossy().into_owned()
        };
        let result = PropertyValue::Font(FontSetting {
            face,
            style,
            size: libobs::obs_data_get_int(object, size_key.as_ptr().0),
            flags: libobs::obs_data_get_int(object, flags_key.as_ptr().0) as u32,
        });
        libobs::obs_data_release(object);
        Some(result)
    })
}

fn read_property_value<T>(
    settings: &T,
    property: &PropertyMetadata,
) -> Result<Option<PropertyValue>, ObsError>
where
    T: ObsDataGetters + ObsDataPointers,
{
    match &property.kind {
        PropertyKind::Bool
        | PropertyKind::Group {
            group_type: GroupType::Checkable,
            ..
        } => settings
            .get_bool(property.name.as_str())
            .map(|value| value.map(PropertyValue::Boolean)),
        PropertyKind::Integer { .. } | PropertyKind::Color | PropertyKind::ColorAlpha => settings
            .get_int(property.name.as_str())
            .map(|value| value.map(PropertyValue::Integer)),
        PropertyKind::Float { .. } => settings
            .get_double(property.name.as_str())
            .map(|value| value.map(PropertyValue::Float)),
        PropertyKind::Text { .. } | PropertyKind::Path { .. } => settings
            .get_string(property.name.as_str())
            .map(|value| value.map(PropertyValue::String)),
        PropertyKind::List { format, .. } => match format {
            ListFormat::String => settings
                .get_string(property.name.as_str())
                .map(|value| value.map(PropertyValue::String)),
            ListFormat::Int => settings
                .get_int(property.name.as_str())
                .map(|value| value.map(PropertyValue::Integer)),
            ListFormat::Float => settings
                .get_double(property.name.as_str())
                .map(|value| value.map(PropertyValue::Float)),
            ListFormat::Bool => settings
                .get_bool(property.name.as_str())
                .map(|value| value.map(PropertyValue::Boolean)),
            _ => Ok(None),
        },
        PropertyKind::FrameRate { .. } => read_frame_rate(settings, property.name.as_str()),
        PropertyKind::EditableList { .. } => read_editable_list(settings, property.name.as_str()),
        PropertyKind::Font => read_font(settings, property.name.as_str()),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::NumberControl;

    fn integer_property() -> PropertyMetadata {
        PropertyMetadata {
            name: "bitrate".into(),
            description: None,
            long_description: None,
            enabled: true,
            visible: true,
            kind: PropertyKind::Integer {
                min: 100,
                max: 10_000,
                step: 50,
                suffix: None,
                control: NumberControl::Scroller,
            },
        }
    }

    #[test]
    fn integer_validation_enforces_range_and_step() {
        let schema = SettingsSchema::new(vec![integer_property()]);
        assert!(schema
            .validate("bitrate", &PropertyValue::Integer(6_000))
            .is_ok());
        assert!(matches!(
            schema.validate("bitrate", &PropertyValue::Integer(6_025)),
            Err(ObsError::PropertyValueNotAllowed { .. })
        ));
        assert!(matches!(
            schema.validate("bitrate", &PropertyValue::Integer(20_000)),
            Err(ObsError::PropertyValueOutOfRange { .. })
        ));
    }

    #[test]
    fn only_checkable_groups_have_boolean_values() {
        let structural = PropertyMetadata {
            name: "advanced".into(),
            description: None,
            long_description: None,
            enabled: true,
            visible: true,
            kind: PropertyKind::Group {
                group_type: GroupType::Normal,
                properties: Vec::new(),
            },
        };
        let checkable = PropertyMetadata {
            name: "enabled_group".into(),
            description: None,
            long_description: None,
            enabled: true,
            visible: true,
            kind: PropertyKind::Group {
                group_type: GroupType::Checkable,
                properties: Vec::new(),
            },
        };
        let schema = SettingsSchema::new(vec![structural, checkable]);
        assert!(matches!(
            schema.validate("advanced", &PropertyValue::Boolean(true)),
            Err(ObsError::PropertyValueUnsupported { .. })
        ));
        assert!(schema
            .validate("enabled_group", &PropertyValue::Boolean(true))
            .is_ok());
    }

    #[test]
    fn frame_rate_named_options_must_be_advertised_by_obs() {
        let schema = SettingsSchema::new(vec![PropertyMetadata {
            name: "fps".into(),
            description: None,
            long_description: None,
            enabled: true,
            visible: true,
            kind: PropertyKind::FrameRate {
                options: vec![crate::capabilities::FrameRateOption {
                    name: Some("match-output".into()),
                    description: None,
                }],
                ranges: Vec::new(),
            },
        }]);
        let named = PropertyValue::FrameRate(FrameRateSetting {
            frame_rate: FrameRate {
                numerator: 0,
                denominator: 0,
            },
            option: Some("match-output".into()),
        });
        assert!(schema.validate("fps", &named).is_ok());

        let invalid = PropertyValue::FrameRate(FrameRateSetting {
            frame_rate: FrameRate {
                numerator: 0,
                denominator: 0,
            },
            option: Some("not-advertised".into()),
        });
        assert!(matches!(
            schema.validate("fps", &invalid),
            Err(ObsError::PropertyValueNotAllowed { .. })
        ));
    }
}
