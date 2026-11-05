#[path = "support/prepared_runtime.rs"]
mod prepared_runtime;

use std::{
    ffi::{c_char, c_void},
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use libobs_simple::wrapper::{
    hotkeys::{HotkeyBindingSnapshot, HotkeyModifiers, ObsContextHotkeyExt},
    settings_actions::{ObsPropertyActions, PropertyActionOutcome},
    sys,
    utils::{ObsPath, StartupInfo},
};

static PROPERTY_ACTION_CLICKS: AtomicUsize = AtomicUsize::new(0);
const PROPERTY_ACTION_SOURCE_ID: &[u8] = b"libobs_rs_test_action_filter\0";
const PROPERTY_ACTION_SOURCE_NAME: &[u8] = b"libobs-rs property-action test filter\0";
const PROPERTY_ACTION_BUTTON: &[u8] = b"do_action\0";
const PROPERTY_ACTION_BUTTON_TEXT: &[u8] = b"Do action\0";
const PROPERTY_ACTION_BOOL: &[u8] = b"not_button\0";
const PROPERTY_ACTION_BOOL_TEXT: &[u8] = b"Not a button\0";

// SAFETY: libobs calls this for the process-lifetime test source registration. The returned
// pointer references a static NUL-terminated byte string.
unsafe extern "C" fn property_action_source_name(_type_data: *mut c_void) -> *const c_char {
    PROPERTY_ACTION_SOURCE_NAME.as_ptr().cast()
}

// SAFETY: libobs owns the returned private-data pointer until it invokes
// `property_action_source_destroy` exactly once for the source instance.
unsafe extern "C" fn property_action_source_create(
    _settings: *mut sys::obs_data_t,
    _source: *mut sys::obs_source_t,
) -> *mut c_void {
    Box::into_raw(Box::new(())) as *mut c_void
}

// SAFETY: `data` is either null or the allocation returned from
// `property_action_source_create`, and libobs calls this at most once for that allocation.
unsafe extern "C" fn property_action_source_destroy(data: *mut c_void) {
    if !data.is_null() {
        // SAFETY: ownership of this allocation is transferred back by the destroy callback.
        unsafe { drop(Box::from_raw(data as *mut ())) };
    }
}

// SAFETY: all pointers are callback-borrowed from libobs. This fixture deliberately needs no
// object data; it only records that libobs routed the click through the registered callback.
unsafe extern "C" fn property_action_clicked(
    _properties: *mut sys::obs_properties_t,
    _property: *mut sys::obs_property_t,
    _data: *mut c_void,
) -> bool {
    PROPERTY_ACTION_CLICKS.fetch_add(1, Ordering::SeqCst);
    true
}

// SAFETY: libobs calls this while building the concrete source's property tree. The returned
// property set transfers ownership to libobs/the wrapper, and all names are static C strings.
unsafe extern "C" fn property_action_source_properties(
    _data: *mut c_void,
) -> *mut sys::obs_properties_t {
    // SAFETY: this callback runs after OBS startup and uses only the property-construction API.
    unsafe {
        let properties = sys::obs_properties_create();
        sys::obs_properties_add_button(
            properties,
            PROPERTY_ACTION_BUTTON.as_ptr().cast(),
            PROPERTY_ACTION_BUTTON_TEXT.as_ptr().cast(),
            Some(property_action_clicked),
        );
        sys::obs_properties_add_bool(
            properties,
            PROPERTY_ACTION_BOOL.as_ptr().cast(),
            PROPERTY_ACTION_BOOL_TEXT.as_ptr().cast(),
        );
        properties
    }
}

fn register_property_action_test_source() {
    // This is a test-only source registration rather than a managed-object operation. libobs
    // accepts registrations after `obs_startup`; all callbacks and string pointers below are
    // process-lifetime statics, and `obs_register_source_s` copies the source-info record.
    unsafe {
        let mut info: sys::obs_source_info = std::mem::zeroed();
        info.id = PROPERTY_ACTION_SOURCE_ID.as_ptr().cast();
        info.type_ = sys::obs_source_type_OBS_SOURCE_TYPE_FILTER;
        info.output_flags = 0;
        info.get_name = Some(property_action_source_name);
        info.create = Some(property_action_source_create);
        info.destroy = Some(property_action_source_destroy);
        info.get_properties = Some(property_action_source_properties);
        sys::obs_register_source_s(&info, std::mem::size_of::<sys::obs_source_info>());
    }
}

fn runtime_dir() -> Option<PathBuf> {
    std::env::var_os("LIBOBS_TEST_RUNTIME_DIR").map(PathBuf::from)
}

fn start_context() -> Option<libobs_simple::wrapper::context::ObsContext> {
    let root = runtime_dir()?;
    let paths = prepared_runtime::startup_paths(&root);
    let config = std::env::temp_dir().join(format!(
        "libobs-rs-managed-controls-config-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&config).expect("test module config directory");
    Some(
        StartupInfo::new()
            .set_startup_paths(paths)
            .set_module_config_path(ObsPath::new(config.to_string_lossy().as_ref()))
            .start()
            .expect("prepared OBS runtime should initialize"),
    )
}

#[test]
fn frontend_hotkey_binding_persists_and_injected_events_reach_receiver() {
    let Some(context) = start_context() else {
        eprintln!("skipping: LIBOBS_TEST_RUNTIME_DIR is not configured");
        return;
    };
    let registration = context
        .register_frontend_hotkey("libobs-rs.test.f9", "libobs-rs integration F9")
        .expect("frontend hotkey registration");
    let receiver = registration.events();
    registration
        .load_bindings(&HotkeyBindingSnapshot {
            entries_json: vec![r#"{"key":"OBS_KEY_F9"}"#.into()],
        })
        .expect("binding load");

    let saved = registration.save_bindings().expect("binding save");
    assert_eq!(saved.entries_json.len(), 1);
    assert!(saved.entries_json[0].contains("OBS_KEY_F9"));

    context
        .inject_hotkey_event("OBS_KEY_F9", HotkeyModifiers::empty(), true)
        .expect("press injection");
    let press = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("press callback");
    assert_eq!(press.id, registration.id());
    assert!(press.pressed);

    context
        .inject_hotkey_event("OBS_KEY_F9", HotkeyModifiers::empty(), false)
        .expect("release injection");
    let release = receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("release callback");
    assert_eq!(release.id, registration.id());
    assert!(!release.pressed);

    assert!(registration
        .load_bindings(&HotkeyBindingSnapshot {
            entries_json: vec!["not-json".into()],
        })
        .is_err());
    drop(registration);
}

#[test]
fn instance_bound_property_button_invocation_refreshes_metadata() {
    let Some(mut context) = start_context() else {
        eprintln!("skipping: LIBOBS_TEST_RUNTIME_DIR is not configured");
        return;
    };
    register_property_action_test_source();
    PROPERTY_ACTION_CLICKS.store(0, Ordering::SeqCst);
    let action_type = context
        .source_type("libobs_rs_test_action_filter")
        .expect("source capability query")
        .expect("test action filter should be registered");
    let filter = context
        .create_filter(&action_type, "Managed property action", None)
        .expect("test action filter creation");

    let outcome = filter
        .invoke_property_button("do_action")
        .expect("registered property button callback");
    let PropertyActionOutcome::Invoked {
        refresh_requested,
        refresh,
    } = outcome
    else {
        panic!("test action must be a plugin callback, not a URL button");
    };
    assert!(refresh_requested);
    assert_eq!(PROPERTY_ACTION_CLICKS.load(Ordering::SeqCst), 1);
    assert!(refresh
        .properties
        .iter()
        .any(|property| property.name == "do_action"));
    assert!(filter
        .invoke_property_button("definitely_missing_button")
        .is_err());
    assert!(filter.invoke_property_button("not_button").is_err());
}
