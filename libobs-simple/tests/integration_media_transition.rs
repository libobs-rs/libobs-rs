#[path = "support/ffmpeg_tools.rs"]
mod ffmpeg_tools;
#[path = "support/prepared_runtime.rs"]
mod prepared_runtime;

use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicI64, AtomicU32, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use libobs_simple::wrapper::{
    data::{object::ObsObjectTrait, ObsDataSetters},
    sources::{ManagedTransition, MediaState, ObsMediaSourceExt, ObsSourceTrait},
    sys,
    utils::{ObsError, ObsPath, StartupInfo, StartupPaths},
};

static TEST_MEDIA_POSITION_MS: AtomicI64 = AtomicI64::new(0);
// C enums are `i32` in the MSVC bindings and `u32` elsewhere; store them as `u32`.
#[allow(clippy::unnecessary_cast)]
const fn media_state_bits(state: sys::obs_media_state) -> u32 {
    state as u32
}

static TEST_MEDIA_STATE: AtomicU32 = AtomicU32::new(media_state_bits(
    sys::obs_media_state_OBS_MEDIA_STATE_STOPPED,
));
const TEST_MEDIA_SOURCE_ID: &[u8] = b"libobs_rs_test_media_source\0";
const TEST_MEDIA_SOURCE_NAME: &[u8] = b"libobs-rs controllable media fixture\0";

// SAFETY: libobs retains the source registration for the process lifetime; this returns a static
// NUL-terminated name.
unsafe extern "C" fn test_media_source_name(
    _type_data: *mut std::ffi::c_void,
) -> *const std::ffi::c_char {
    TEST_MEDIA_SOURCE_NAME.as_ptr().cast()
}

// SAFETY: ownership of the returned allocation is transferred to libobs until destroy.
unsafe extern "C" fn test_media_create(
    _settings: *mut sys::obs_data_t,
    _source: *mut sys::obs_source_t,
) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(())) as *mut std::ffi::c_void
}

// SAFETY: `data` is the allocation returned by `test_media_create` and is destroyed once.
unsafe extern "C" fn test_media_destroy(data: *mut std::ffi::c_void) {
    if !data.is_null() {
        // SAFETY: libobs is returning ownership of the allocation to this callback.
        unsafe { drop(Box::from_raw(data as *mut ())) };
    }
}

// SAFETY: callback arguments are owned/borrowed by libobs; this fixture stores only global test
// state and does not dereference `data`.
unsafe extern "C" fn test_media_play_pause(_data: *mut std::ffi::c_void, pause: bool) {
    TEST_MEDIA_STATE.store(
        media_state_bits(if pause {
            sys::obs_media_state_OBS_MEDIA_STATE_PAUSED
        } else {
            sys::obs_media_state_OBS_MEDIA_STATE_PLAYING
        }),
        Ordering::SeqCst,
    );
}

// SAFETY: see `test_media_play_pause`; no callback pointer is dereferenced.
unsafe extern "C" fn test_media_restart(_data: *mut std::ffi::c_void) {
    TEST_MEDIA_POSITION_MS.store(0, Ordering::SeqCst);
    TEST_MEDIA_STATE.store(
        media_state_bits(sys::obs_media_state_OBS_MEDIA_STATE_PLAYING),
        Ordering::SeqCst,
    );
}

// SAFETY: see `test_media_play_pause`; no callback pointer is dereferenced.
unsafe extern "C" fn test_media_stop(_data: *mut std::ffi::c_void) {
    TEST_MEDIA_STATE.store(
        media_state_bits(sys::obs_media_state_OBS_MEDIA_STATE_STOPPED),
        Ordering::SeqCst,
    );
}

// SAFETY: see `test_media_play_pause`; no callback pointer is dereferenced.
unsafe extern "C" fn test_media_duration(_data: *mut std::ffi::c_void) -> i64 {
    5_000
}

// SAFETY: see `test_media_play_pause`; no callback pointer is dereferenced.
unsafe extern "C" fn test_media_time(_data: *mut std::ffi::c_void) -> i64 {
    TEST_MEDIA_POSITION_MS.load(Ordering::SeqCst)
}

// SAFETY: see `test_media_play_pause`; no callback pointer is dereferenced.
unsafe extern "C" fn test_media_set_time(_data: *mut std::ffi::c_void, milliseconds: i64) {
    TEST_MEDIA_POSITION_MS.store(milliseconds, Ordering::SeqCst);
}

// SAFETY: see `test_media_play_pause`; no callback pointer is dereferenced.
#[allow(clippy::unnecessary_cast)]
unsafe extern "C" fn test_media_state(_data: *mut std::ffi::c_void) -> sys::obs_media_state {
    TEST_MEDIA_STATE.load(Ordering::SeqCst) as sys::obs_media_state
}

fn register_test_media_source() {
    // Test-only process-global type registration after OBS startup. All callback and string
    // pointers are process-lifetime statics, and libobs copies this source-info record.
    unsafe {
        let mut info: sys::obs_source_info = std::mem::zeroed();
        info.id = TEST_MEDIA_SOURCE_ID.as_ptr().cast();
        info.type_ = sys::obs_source_type_OBS_SOURCE_TYPE_INPUT;
        info.output_flags = sys::OBS_SOURCE_CONTROLLABLE_MEDIA;
        info.get_name = Some(test_media_source_name);
        info.create = Some(test_media_create);
        info.destroy = Some(test_media_destroy);
        info.media_play_pause = Some(test_media_play_pause);
        info.media_restart = Some(test_media_restart);
        info.media_stop = Some(test_media_stop);
        info.media_get_duration = Some(test_media_duration);
        info.media_get_time = Some(test_media_time);
        info.media_set_time = Some(test_media_set_time);
        info.media_get_state = Some(test_media_state);
        sys::obs_register_source_s(&info, std::mem::size_of::<sys::obs_source_info>());
    }
}

fn test_runtime() -> Option<PathBuf> {
    std::env::var_os("LIBOBS_TEST_RUNTIME_DIR").map(PathBuf::from)
}

fn startup_paths(root: &Path) -> StartupPaths {
    prepared_runtime::startup_paths(root)
}

fn start_context(label: &str) -> Option<libobs_simple::wrapper::context::ObsContext> {
    let root = test_runtime()?;
    let config =
        std::env::temp_dir().join(format!("libobs-rs-{label}-config-{}", std::process::id()));
    Some(
        StartupInfo::new()
            .set_startup_paths(startup_paths(&root))
            .set_module_config_path(ObsPath::new(config.to_string_lossy().as_ref()))
            .start()
            .unwrap_or_else(|error| panic!("prepared OBS runtime failed to initialize: {error}")),
    )
}

fn color_source(
    context: &libobs_simple::wrapper::context::ObsContext,
    name: &str,
    color: u32,
) -> libobs_simple::wrapper::sources::ObsSourceRef {
    let ty = context
        .source_type("color_source_v3")
        .expect("capability query")
        .expect("color source should be installed");
    let mut settings = ty.default_settings_mut().expect("color defaults");
    settings
        .set_int("width", 640)
        .unwrap()
        .set_int("height", 360)
        .unwrap()
        .set_int("color", i64::from(color))
        .unwrap();
    context
        .create_source(&ty, name, Some(settings))
        .expect("color source should be created")
}

fn wait_for_media_state<T: ObsMediaSourceExt + ?Sized>(
    source: &T,
    wanted: MediaState,
    timeout: Duration,
) {
    let deadline = Instant::now() + timeout;
    loop {
        let state = source.media_state().expect("media state query");
        if state == wanted {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "media state never became {wanted:?}; last state was {state:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn native_transition_completes_and_can_be_interrupted_without_blocking_force_stop() {
    let Some(mut context) = start_context("transition") else {
        eprintln!("skipping: LIBOBS_TEST_RUNTIME_DIR is not configured");
        return;
    };

    let first = color_source(&context, "Transition red", 0xFF0000FF);
    let second = color_source(&context, "Transition blue", 0xFFFF0000);
    let transition_type = context
        .source_type("fade_transition")
        .expect("transition capability query")
        .expect("OBS fade transition should be installed");
    let transition_source = context
        .create_source(&transition_type, "Managed fade", None)
        .expect("fade transition should be created");
    let transition = Arc::new(
        ManagedTransition::new(transition_source.clone())
            .expect("transition source should be accepted by managed controller"),
    );
    transition
        .set_current_source(first)
        .expect("initial transition source should be selected");

    // Keep the transition source active/rendered so libobs advances its video transition clock.
    let mut scene = context
        .scene("Transition output", Some(0))
        .expect("transition scene should be active");
    let _transition_item = scene
        .add(transition_source.clone())
        .expect("transition source should be rendered in the active scene");

    transition
        .start_to_with_timeout(
            second.clone(),
            Duration::from_millis(120),
            Duration::from_secs(2),
        )
        .expect("short native transition should complete");
    assert!(!transition.is_active().unwrap());

    let started = transition_source
        .signals()
        .on_transition_start()
        .expect("transition start signal should be subscribable");
    let interrupt_destination = color_source(&context, "Transition green", 0xFF00FF00);
    transition
        .begin_to(interrupt_destination.clone(), Duration::from_secs(5))
        .expect("non-blocking transition should start");
    started
        .recv_timeout(Duration::from_secs(1))
        .expect("long transition should emit its start signal");
    assert!(transition.is_active().unwrap());

    let forced_at = Instant::now();
    transition
        .force_stop()
        .expect("force_stop should interrupt a transition begun on the same application thread");
    assert!(
        forced_at.elapsed() < Duration::from_secs(1),
        "force-stop path should not wait for the original five-second duration"
    );
    assert!(!transition.is_active().unwrap());
    assert_eq!(
        transition.current_source().unwrap().unwrap().object_id(),
        interrupt_destination.object_id(),
        "force-stop should snap to and retain the requested destination"
    );
}

#[test]
fn native_ffmpeg_media_source_exposes_state_pause_restart_and_stop() {
    let Some(mut context) = start_context("media") else {
        eprintln!("skipping: LIBOBS_TEST_RUNTIME_DIR is not configured");
        return;
    };

    let fixture_dir =
        std::env::temp_dir().join(format!("libobs-rs-media-native-{}", std::process::id()));
    std::fs::create_dir_all(&fixture_dir).unwrap();
    let media_path = fixture_dir.join("controllable.mp4");
    let generated = ffmpeg_tools::ffmpeg()
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=purple:s=320x180:r=30:d=3",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=660:sample_rate=44100:duration=3",
            "-shortest",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&media_path)
        .status()
        .expect("ffmpeg should be available in the native-test environment");
    assert!(generated.success(), "ffmpeg fixture generation failed");

    let media_type = context
        .source_type("ffmpeg_source")
        .expect("media capability query")
        .expect("OBS ffmpeg media source should be installed");
    let mut settings = media_type.default_settings_mut().expect("media defaults");
    settings
        .set_bool("is_local_file", true)
        .unwrap()
        .set_string("local_file", media_path.to_string_lossy().as_ref())
        .unwrap()
        .set_bool("looping", true)
        .unwrap();
    let media = context
        .create_source(&media_type, "Managed media", Some(settings))
        .expect("media source should be created");

    let mut scene = context.scene("Media output", Some(0)).unwrap();
    let _item = scene.add(media.clone()).unwrap();
    media.media_restart().unwrap();
    wait_for_media_state(&media, MediaState::Playing, Duration::from_secs(2));
    assert!(
        media.media_duration().unwrap().unwrap_or_default() >= Duration::from_secs(2),
        "media duration should be reported after opening"
    );

    media.media_pause().unwrap();
    wait_for_media_state(&media, MediaState::Paused, Duration::from_secs(1));
    media.media_play().unwrap();
    wait_for_media_state(&media, MediaState::Playing, Duration::from_secs(1));
    media.media_restart().unwrap();
    wait_for_media_state(&media, MediaState::Playing, Duration::from_secs(1));
    media.media_stop().unwrap();
    wait_for_media_state(&media, MediaState::Stopped, Duration::from_secs(1));

    let plain = color_source(&context, "Not controllable media", 0xFFFFFFFF);
    assert!(matches!(
        plain.media_pause(),
        Err(ObsError::InvalidOperation(_))
    ));

    let _ = std::fs::remove_dir_all(fixture_dir);
}

#[test]
fn native_media_seek_reaches_the_registered_plugin_callback() {
    let Some(mut context) = start_context("media-seek") else {
        eprintln!("skipping: LIBOBS_TEST_RUNTIME_DIR is not configured");
        return;
    };

    register_test_media_source();
    TEST_MEDIA_POSITION_MS.store(0, Ordering::SeqCst);
    TEST_MEDIA_STATE.store(
        media_state_bits(sys::obs_media_state_OBS_MEDIA_STATE_STOPPED),
        Ordering::SeqCst,
    );
    let media_type = context
        .source_type("libobs_rs_test_media_source")
        .expect("test media capability query")
        .expect("test media source should be registered");
    let media = context
        .create_source(&media_type, "Deterministic media seek", None)
        .expect("test media source creation");

    let mut scene = context.scene("Media seek output", Some(0)).unwrap();
    let _item = scene.add(media.clone()).unwrap();
    media.media_restart().unwrap();
    wait_for_media_state(&media, MediaState::Playing, Duration::from_secs(1));

    media.media_seek(Duration::from_millis(3_125)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let position = media
            .media_position()
            .unwrap()
            .expect("test media source exposes a position");
        if position == Duration::from_millis(3_125) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "queued seek never reached media_set_time; last position was {position:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(TEST_MEDIA_POSITION_MS.load(Ordering::SeqCst), 3_125);
    assert_eq!(
        media.media_duration().unwrap(),
        Some(Duration::from_secs(5))
    );
}
