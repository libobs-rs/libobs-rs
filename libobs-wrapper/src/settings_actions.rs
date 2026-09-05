//! Instance-bound actions exposed by dynamic OBS property trees.
//!
//! OBS button properties are actions, not settings values. Invoking one requires the concrete
//! source/output/encoder/service instance whose property tree created the button. This module keeps
//! that distinction explicit and refreshes both property metadata and cached instance settings after
//! a successful invocation.

use std::ffi::{c_void, CStr};

use crate::{
    capabilities::PropertyMetadata,
    data::{object::ObsObjectTrait, ImmutableObsData},
    encoders::{audio::ObsAudioEncoder, video::ObsVideoEncoder},
    run_with_obs,
    services::ObsServiceRef,
    sources::{ObsFilterRef, ObsSourceRef},
    unsafe_send::{NativePointer, Sendable, SmartPointerSendable},
    utils::{ObsError, ObsString},
};

use crate::data::output::{ObsOutputRef, ObsReplayBufferOutputRef};

#[derive(Debug, Clone)]
pub struct PropertyActionRefresh {
    /// Refreshed instance property metadata after invoking the action.
    pub properties: Vec<PropertyMetadata>,
    /// Fresh settings read directly from the native instance after the action.
    pub settings: ImmutableObsData,
}

#[derive(Debug, Clone)]
pub enum PropertyActionOutcome {
    /// A plugin callback was invoked. `refresh_requested` is the bool returned by the native
    /// property callback; metadata/settings are refreshed regardless so callers never need to
    /// infer stale state from that advisory flag.
    Invoked {
        refresh_requested: bool,
        refresh: PropertyActionRefresh,
    },
    /// OBS URL buttons are surfaced instead of opening a browser as a side effect of a library
    /// call. The application may explicitly open this URL after applying its own trust policy.
    ExternalUrl {
        url: String,
        refresh: PropertyActionRefresh,
    },
}

impl PropertyActionOutcome {
    pub fn refresh(&self) -> &PropertyActionRefresh {
        match self {
            Self::Invoked { refresh, .. } | Self::ExternalUrl { refresh, .. } => refresh,
        }
    }
}

/// Instance-bound dynamic property actions.
///
/// A type-level descriptor is intentionally insufficient: button callbacks receive the owning OBS
/// object instance. Implementations therefore query the concrete object's property tree, verify the
/// named property is an enabled button, invoke it with that same instance, and refresh metadata.
pub trait ObsPropertyActions: ObsObjectTrait {
    fn invoke_property_button(&self, name: &str) -> Result<PropertyActionOutcome, ObsError>;
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
fn source_properties(source: *mut libobs::obs_source_t) -> *mut libobs::obs_properties_t {
    // SAFETY: `invoke_button` calls this only on the OBS actor while retaining the source lease.
    unsafe { libobs::obs_source_properties(source) }
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
fn source_settings(source: *mut libobs::obs_source_t) -> *mut libobs::obs_data_t {
    // SAFETY: `invoke_button` calls this only on the OBS actor while retaining the source lease.
    unsafe { libobs::obs_source_get_settings(source) }
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
fn output_properties(output: *mut libobs::obs_output_t) -> *mut libobs::obs_properties_t {
    // SAFETY: `invoke_button` calls this only on the OBS actor while retaining the output lease.
    unsafe { libobs::obs_output_properties(output) }
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
fn output_settings(output: *mut libobs::obs_output_t) -> *mut libobs::obs_data_t {
    // SAFETY: `invoke_button` calls this only on the OBS actor while retaining the output lease.
    unsafe { libobs::obs_output_get_settings(output) }
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
fn encoder_properties(encoder: *mut libobs::obs_encoder_t) -> *mut libobs::obs_properties_t {
    // SAFETY: `invoke_button` calls this only on the OBS actor while retaining the encoder lease.
    unsafe { libobs::obs_encoder_properties(encoder) }
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
fn encoder_settings(encoder: *mut libobs::obs_encoder_t) -> *mut libobs::obs_data_t {
    // SAFETY: `invoke_button` calls this only on the OBS actor while retaining the encoder lease.
    unsafe { libobs::obs_encoder_get_settings(encoder) }
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
fn service_properties(service: *mut libobs::obs_service_t) -> *mut libobs::obs_properties_t {
    // SAFETY: `invoke_button` calls this only on the OBS actor while retaining the service lease.
    unsafe { libobs::obs_service_properties(service) }
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
fn service_settings(service: *mut libobs::obs_service_t) -> *mut libobs::obs_data_t {
    // SAFETY: `invoke_button` calls this only on the OBS actor while retaining the service lease.
    unsafe { libobs::obs_service_get_settings(service) }
}

fn invoke_button<P, GP, GS, T>(
    target: &T,
    object: SmartPointerSendable<P>,
    name: &str,
    get_properties: GP,
    get_settings: GS,
) -> Result<PropertyActionOutcome, ObsError>
where
    P: NativePointer,
    T: ObsObjectTrait<Native = P> + ?Sized,
    GP: Fn(P) -> *mut libobs::obs_properties_t + Send + 'static,
    GS: Fn(P) -> *mut libobs::obs_data_t + Send + 'static,
{
    let runtime = target.runtime().clone();
    let property_name = ObsString::new(name);
    let object_for_actor = object.clone();
    let (refresh_requested, url, properties, settings_ptr) =
        run_with_obs!(runtime, (object_for_actor, property_name), move || unsafe {
            // Safety: the object lease is retained for the complete actor command. The property
            // tree is instance-specific and is destroyed before returning from this command.
            let properties = get_properties(object_for_actor.get_ptr());
            if properties.is_null() {
                return Err(ObsError::InvalidOperation(
                    "OBS instance returned no property tree".into(),
                ));
            }

            let property = libobs::obs_properties_get(properties, property_name.as_ptr().0);
            if property.is_null() {
                libobs::obs_properties_destroy(properties);
                return Err(ObsError::PropertyNotFound {
                    name: property_name.to_string(),
                });
            }
            if libobs::obs_property_get_type(property)
                != libobs::obs_property_type_OBS_PROPERTY_BUTTON
            {
                libobs::obs_properties_destroy(properties);
                return Err(ObsError::PropertyValueUnsupported {
                    name: property_name.to_string(),
                    property_type: "non-button property".into(),
                });
            }
            if !libobs::obs_property_enabled(property) {
                libobs::obs_properties_destroy(properties);
                return Err(ObsError::InvalidOperation(format!(
                    "OBS property '{}' is currently disabled",
                    property_name
                )));
            }

            let button_type = libobs::obs_property_button_type(property);
            let mut url = None;
            let refresh_requested = if button_type == libobs::obs_button_type_OBS_BUTTON_URL {
                let raw_url = libobs::obs_property_button_url(property);
                if !raw_url.is_null() {
                    url = Some(CStr::from_ptr(raw_url).to_string_lossy().into_owned());
                }
                false
            } else {
                // obs_property_button_clicked expects the owning obs_context_data object pointer,
                // i.e. the concrete source/output/encoder/service pointer used here.
                libobs::obs_property_button_clicked(
                    property,
                    object_for_actor.get_ptr().into_addr() as *mut c_void,
                )
            };
            libobs::obs_properties_destroy(properties);

            // Always rebuild after invocation. Plugins frequently mutate enabled/visible/list
            // metadata from the button callback even when its bool return is false.
            let refreshed = get_properties(object_for_actor.get_ptr());
            let properties = crate::capabilities::snapshot_owned_properties(refreshed);
            let settings = get_settings(object_for_actor.get_ptr());
            if settings.is_null() {
                return Err(ObsError::NullPointer(Some(
                    "failed to refresh instance settings after property action".into(),
                )));
            }
            Ok((refresh_requested, url, properties, Sendable(settings)))
        })??;

    let settings = ImmutableObsData::from_raw_pointer(settings_ptr, target.runtime().clone());
    target.__internal_replace_settings(settings.clone())?;
    let refresh = PropertyActionRefresh {
        properties,
        settings,
    };
    Ok(match url {
        Some(url) => PropertyActionOutcome::ExternalUrl { url, refresh },
        None => PropertyActionOutcome::Invoked {
            refresh_requested,
            refresh,
        },
    })
}

macro_rules! impl_source_actions {
    ($ty:ty) => {
        impl ObsPropertyActions for $ty {
            fn invoke_property_button(
                &self,
                name: &str,
            ) -> Result<PropertyActionOutcome, ObsError> {
                invoke_button(
                    self,
                    self.__native_handle(),
                    name,
                    source_properties,
                    source_settings,
                )
            }
        }
    };
}

impl_source_actions!(ObsSourceRef);
impl_source_actions!(ObsFilterRef);

macro_rules! impl_output_actions {
    ($ty:ty) => {
        impl ObsPropertyActions for $ty {
            fn invoke_property_button(
                &self,
                name: &str,
            ) -> Result<PropertyActionOutcome, ObsError> {
                invoke_button(
                    self,
                    self.__native_handle(),
                    name,
                    output_properties,
                    output_settings,
                )
            }
        }
    };
}

impl_output_actions!(ObsOutputRef);
impl_output_actions!(ObsReplayBufferOutputRef);

impl ObsPropertyActions for ObsVideoEncoder {
    fn invoke_property_button(&self, name: &str) -> Result<PropertyActionOutcome, ObsError> {
        invoke_button(
            self,
            self.__native_handle(),
            name,
            encoder_properties,
            encoder_settings,
        )
    }
}

impl ObsPropertyActions for ObsAudioEncoder {
    fn invoke_property_button(&self, name: &str) -> Result<PropertyActionOutcome, ObsError> {
        invoke_button(
            self,
            self.__native_handle(),
            name,
            encoder_properties,
            encoder_settings,
        )
    }
}

impl ObsPropertyActions for ObsServiceRef {
    fn invoke_property_button(&self, name: &str) -> Result<PropertyActionOutcome, ObsError> {
        invoke_button(
            self,
            self.__native_handle(),
            name,
            service_properties,
            service_settings,
        )
    }
}
