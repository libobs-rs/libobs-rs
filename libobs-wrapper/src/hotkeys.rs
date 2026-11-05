//! Managed libobs hotkey registration and binding persistence.
//!
//! Native hotkey callbacks never run application code directly. They publish a small owned event
//! into the wrapper's bounded signal hub; applications consume events from a [`SignalReceiver`]
//! on their own thread and may then start/stop outputs, save replay buffers, etc. This keeps native
//! callback work bounded and avoids re-entering blocking wrapper APIs from an OBS callback.

use std::{
    ffi::{c_void, CStr, CString},
    sync::Arc,
};

use crate::{
    context::ObsContext,
    data::{object::ObsObjectTrait, output::ObsOutputTrait},
    encoders::{audio::ObsAudioEncoder, video::ObsVideoEncoder},
    run_with_obs,
    runtime::ObsRuntime,
    services::ObsServiceRef,
    signals::{SignalHub, SignalReceiver},
    sources::ObsSourceTrait,
    unsafe_send::SmartPointerSendable,
    utils::{ObsError, ObsString},
};

// C enums are `i32` in the MSVC bindings and `u32` elsewhere, so normalize the
// interaction flag values to the `u32` used by `obs_key_combination::modifiers`.
#[allow(clippy::unnecessary_cast)]
mod modifier_bits {
    pub(super) const SHIFT: u32 = libobs::obs_interaction_flags_INTERACT_SHIFT_KEY as u32;
    pub(super) const CONTROL: u32 = libobs::obs_interaction_flags_INTERACT_CONTROL_KEY as u32;
    pub(super) const ALT: u32 = libobs::obs_interaction_flags_INTERACT_ALT_KEY as u32;
    pub(super) const COMMAND: u32 = libobs::obs_interaction_flags_INTERACT_COMMAND_KEY as u32;
}

bitflags::bitflags! {
    /// Modifier bits accepted by libobs hotkey injection.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct HotkeyModifiers: u32 {
        const SHIFT = modifier_bits::SHIFT;
        const CONTROL = modifier_bits::CONTROL;
        const ALT = modifier_bits::ALT;
        const COMMAND = modifier_bits::COMMAND;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyEvent {
    pub id: usize,
    pub pressed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HotkeyBindingSnapshot {
    /// Each entry is one OBS binding object serialized through `obs_data_get_json`.
    pub entries_json: Vec<String>,
}

#[derive(Debug, Clone)]
enum HotkeyOwner {
    Frontend,
    Source {
        _lease: SmartPointerSendable<*mut libobs::obs_source_t>,
    },
    Output {
        _lease: SmartPointerSendable<*mut libobs::obs_output_t>,
    },
    Encoder {
        _lease: SmartPointerSendable<*mut libobs::obs_encoder_t>,
    },
    Service {
        _lease: SmartPointerSendable<*mut libobs::obs_service_t>,
    },
}

/// One registered libobs hotkey. Dropping it unregisters the native hotkey on the OBS actor.
#[derive(Debug)]
pub struct ObsHotkeyRegistration {
    id: usize,
    runtime: ObsRuntime,
    owner: HotkeyOwner,
    hub: Arc<SignalHub<HotkeyEvent>>,
}

impl ObsHotkeyRegistration {
    pub fn id(&self) -> usize {
        self.id
    }

    /// Subscribes to bounded press/release events. Slow subscribers drop new events rather than
    /// blocking libobs's hotkey callback.
    pub fn events(&self) -> SignalReceiver<HotkeyEvent> {
        self.hub.subscribe()
    }

    /// Saves the native binding array as owned JSON objects suitable for later `load_bindings`.
    pub fn save_bindings(&self) -> Result<HotkeyBindingSnapshot, ObsError> {
        let id = self.id;
        run_with_obs!(self.runtime, move || unsafe {
            // Safety: id remains registered for the lifetime of `self`; the returned array is
            // released before leaving the actor command and every item reference is balanced.
            let array = libobs::obs_hotkey_save(id);
            if array.is_null() {
                return Ok(HotkeyBindingSnapshot::default());
            }
            let mut entries_json = Vec::with_capacity(libobs::obs_data_array_count(array));
            for index in 0..libobs::obs_data_array_count(array) {
                let item = libobs::obs_data_array_item(array, index);
                if item.is_null() {
                    continue;
                }
                let json = libobs::obs_data_get_json(item);
                if !json.is_null() {
                    entries_json.push(CStr::from_ptr(json).to_string_lossy().into_owned());
                }
                libobs::obs_data_release(item);
            }
            libobs::obs_data_array_release(array);
            Ok(HotkeyBindingSnapshot { entries_json })
        })?
    }

    /// Replaces the native bindings for this registration from a prior snapshot.
    pub fn load_bindings(&self, snapshot: &HotkeyBindingSnapshot) -> Result<(), ObsError> {
        let id = self.id;
        let entries = snapshot
            .entries_json
            .iter()
            .map(|json| CString::new(json.as_str()).map_err(|_| ObsError::JsonParseError))
            .collect::<Result<Vec<_>, _>>()?;
        run_with_obs!(self.runtime, (entries), move || unsafe {
            // Safety: all temporary data/array objects are created, consumed and released on the
            // OBS actor. `obs_data_array_push_back` increments each item's reference.
            let array = libobs::obs_data_array_create();
            if array.is_null() {
                return Err(ObsError::NullPointer(Some(
                    "failed to allocate OBS hotkey binding array".into(),
                )));
            }
            for json in &entries {
                let item = libobs::obs_data_create_from_json(json.as_ptr());
                if item.is_null() {
                    libobs::obs_data_array_release(array);
                    return Err(ObsError::JsonParseError);
                }
                libobs::obs_data_array_push_back(array, item);
                libobs::obs_data_release(item);
            }
            libobs::obs_hotkey_load(id, array);
            libobs::obs_data_array_release(array);
            Ok(())
        })?
    }
}

#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
impl Drop for ObsHotkeyRegistration {
    fn drop(&mut self) {
        let id = self.id;
        let owner = self.owner.clone();
        let hub = self.hub.clone();
        self.runtime.defer_obs_cleanup(move || unsafe {
            // Safety: callback userdata and any object owner lease remain alive through native
            // unregistration. Dropping them only after unregister prevents callback-after-free.
            libobs::obs_hotkey_unregister(id);
            drop(owner);
            drop(hub);
        });
    }
}

/// # Safety
/// libobs must call this only while the corresponding registration is alive. `data` must be the
/// `Arc::as_ptr` value for the `SignalHub` retained through native hotkey unregistration.
unsafe extern "C" fn hotkey_handler(
    data: *mut c_void,
    id: libobs::obs_hotkey_id,
    _hotkey: *mut libobs::obs_hotkey_t,
    pressed: bool,
) {
    let Some(hub) = (data as *const SignalHub<HotkeyEvent>).as_ref() else {
        return;
    };
    hub.publish(HotkeyEvent { id, pressed });
}

fn finish_registration(
    runtime: ObsRuntime,
    owner: HotkeyOwner,
    hub: Arc<SignalHub<HotkeyEvent>>,
    id: usize,
) -> Result<ObsHotkeyRegistration, ObsError> {
    if id == usize::MAX {
        return Err(ObsError::InvalidOperation(
            "libobs rejected hotkey registration".into(),
        ));
    }
    Ok(ObsHotkeyRegistration {
        id,
        runtime,
        owner,
        hub,
    })
}

fn names(name: &str, description: &str) -> (ObsString, ObsString) {
    (ObsString::new(name), ObsString::new(description))
}

pub trait ObsContextHotkeyExt {
    fn register_frontend_hotkey(
        &self,
        name: &str,
        description: &str,
    ) -> Result<ObsHotkeyRegistration, ObsError>;

    /// Injects one libobs hotkey press/release event by OBS key name (for example
    /// `"OBS_KEY_F9"`). This is useful for application-level testing and virtual input; it does
    /// not synthesize an OS keyboard event.
    fn inject_hotkey_event(
        &self,
        key_name: &str,
        modifiers: HotkeyModifiers,
        pressed: bool,
    ) -> Result<(), ObsError>;
}

impl ObsContextHotkeyExt for ObsContext {
    fn register_frontend_hotkey(
        &self,
        name: &str,
        description: &str,
    ) -> Result<ObsHotkeyRegistration, ObsError> {
        let (name, description) = names(name, description);
        let hub = Arc::new(SignalHub::new());
        let callback_hub = hub.clone();
        let runtime = self.runtime().clone();
        let id = run_with_obs!(runtime, (name, description, callback_hub), move || unsafe {
            // SAFETY: strings and callback state are retained for the actor call and registration.
            libobs::obs_hotkey_register_frontend(
                name.as_ptr().0,
                description.as_ptr().0,
                Some(hotkey_handler),
                Arc::as_ptr(&callback_hub) as *mut c_void,
            )
        })?;
        finish_registration(runtime, HotkeyOwner::Frontend, hub, id)
    }

    fn inject_hotkey_event(
        &self,
        key_name: &str,
        modifiers: HotkeyModifiers,
        pressed: bool,
    ) -> Result<(), ObsError> {
        let key_name = ObsString::new(key_name);
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (key_name), move || unsafe {
            // Safety: key_name is retained for the actor call; the combination is a plain value.
            let key = libobs::obs_key_from_name(key_name.as_ptr().0);
            if key == libobs::obs_key_OBS_KEY_NONE {
                return Err(ObsError::InvalidOperation(format!(
                    "unknown OBS hotkey name '{}'",
                    key_name
                )));
            }
            libobs::obs_hotkey_inject_event(
                libobs::obs_key_combination_t {
                    modifiers: modifiers.bits(),
                    key,
                },
                pressed,
            );
            Ok(())
        })?
    }
}

pub trait ObsSourceHotkeyExt: ObsSourceTrait {
    fn register_hotkey(
        &self,
        name: &str,
        description: &str,
    ) -> Result<ObsHotkeyRegistration, ObsError> {
        let (name, description) = names(name, description);
        let hub = Arc::new(SignalHub::new());
        let callback_hub = hub.clone();
        let source = self.__native_handle();
        let source_for_actor = source.clone();
        let runtime = self.runtime().clone();
        let id = run_with_obs!(
            runtime,
            (source_for_actor, name, description, callback_hub),
            move || unsafe {
                // SAFETY: owner lease, strings, and callback state are retained through registration.
                libobs::obs_hotkey_register_source(
                    source_for_actor.get_ptr(),
                    name.as_ptr().0,
                    description.as_ptr().0,
                    Some(hotkey_handler),
                    Arc::as_ptr(&callback_hub) as *mut c_void,
                )
            }
        )?;
        finish_registration(runtime, HotkeyOwner::Source { _lease: source }, hub, id)
    }
}
impl<T: ObsSourceTrait + ?Sized> ObsSourceHotkeyExt for T {}

pub trait ObsOutputHotkeyExt: ObsOutputTrait {
    fn register_hotkey(
        &self,
        name: &str,
        description: &str,
    ) -> Result<ObsHotkeyRegistration, ObsError> {
        let (name, description) = names(name, description);
        let hub = Arc::new(SignalHub::new());
        let callback_hub = hub.clone();
        let output = self.__native_handle();
        let output_for_actor = output.clone();
        let runtime = self.runtime().clone();
        let id = run_with_obs!(
            runtime,
            (output_for_actor, name, description, callback_hub),
            move || unsafe {
                // SAFETY: owner lease, strings, and callback state are retained through registration.
                libobs::obs_hotkey_register_output(
                    output_for_actor.get_ptr(),
                    name.as_ptr().0,
                    description.as_ptr().0,
                    Some(hotkey_handler),
                    Arc::as_ptr(&callback_hub) as *mut c_void,
                )
            }
        )?;
        finish_registration(runtime, HotkeyOwner::Output { _lease: output }, hub, id)
    }
}
impl<T: ObsOutputTrait + ?Sized> ObsOutputHotkeyExt for T {}

pub trait ObsEncoderHotkeyExt: ObsObjectTrait<Native = *mut libobs::obs_encoder_t> {
    fn register_hotkey(
        &self,
        name: &str,
        description: &str,
    ) -> Result<ObsHotkeyRegistration, ObsError> {
        let (name, description) = names(name, description);
        let hub = Arc::new(SignalHub::new());
        let callback_hub = hub.clone();
        let encoder = self.__native_handle();
        let encoder_for_actor = encoder.clone();
        let runtime = self.runtime().clone();
        let id = run_with_obs!(
            runtime,
            (encoder_for_actor, name, description, callback_hub),
            move || unsafe {
                // SAFETY: owner lease, strings, and callback state are retained through registration.
                libobs::obs_hotkey_register_encoder(
                    encoder_for_actor.get_ptr(),
                    name.as_ptr().0,
                    description.as_ptr().0,
                    Some(hotkey_handler),
                    Arc::as_ptr(&callback_hub) as *mut c_void,
                )
            }
        )?;
        finish_registration(runtime, HotkeyOwner::Encoder { _lease: encoder }, hub, id)
    }
}
impl ObsEncoderHotkeyExt for ObsAudioEncoder {}
impl ObsEncoderHotkeyExt for ObsVideoEncoder {}

pub trait ObsServiceHotkeyExt {
    fn register_hotkey(
        &self,
        name: &str,
        description: &str,
    ) -> Result<ObsHotkeyRegistration, ObsError>;
}

impl ObsServiceHotkeyExt for ObsServiceRef {
    fn register_hotkey(
        &self,
        name: &str,
        description: &str,
    ) -> Result<ObsHotkeyRegistration, ObsError> {
        let (name, description) = names(name, description);
        let hub = Arc::new(SignalHub::new());
        let callback_hub = hub.clone();
        let service = self.__native_handle();
        let service_for_actor = service.clone();
        let runtime = self.runtime().clone();
        let id = run_with_obs!(
            runtime,
            (service_for_actor, name, description, callback_hub),
            move || unsafe {
                // SAFETY: owner lease, strings, and callback state are retained through registration.
                libobs::obs_hotkey_register_service(
                    service_for_actor.get_ptr(),
                    name.as_ptr().0,
                    description.as_ptr().0,
                    Some(hotkey_handler),
                    Arc::as_ptr(&callback_hub) as *mut c_void,
                )
            }
        )?;
        finish_registration(runtime, HotkeyOwner::Service { _lease: service }, hub, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_snapshot_is_owned_and_roundtrippable_without_runtime() {
        let snapshot = HotkeyBindingSnapshot {
            entries_json: vec![r#"{"key":"OBS_KEY_F9"}"#.into()],
        };
        let cloned = snapshot.clone();
        assert_eq!(snapshot, cloned);
    }
}
