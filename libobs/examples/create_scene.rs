#![allow(unknown_lints)]
#![allow(ensure_obs_call_in_runtime)]
#![allow(require_safety_comments_on_unsafe)]

use std::{
    env::current_exe,
    ffi::{c_void, CStr, CString},
    ptr,
};

pub(crate) unsafe extern "C" fn main_crash_handler<V>(
    _format: *const i8,
    _args: *mut V,
    _params: *mut c_void,
) {
    println!("Some crash");
}

fn main() {
    // STARTUP
    unsafe {
        println!("Starting OBS initialization process");

        if libobs::obs_initialized() {
            panic!("error: obs already initialized");
        }
        println!("OBS not yet initialized, continuing");

        #[cfg(target_os = "windows")]
        libobs::obs_init_win32_crash_handler();

        libobs::base_set_crash_handler(Some(main_crash_handler), std::ptr::null_mut());
        println!("Setting log handler on Windows");
        libobs::base_set_log_handler(Some(log_handler), ptr::null_mut());
        println!("Log handler set successfully");

        println!("Retrieving libobs version string...");
        let version_ptr = libobs::obs_get_version_string();
        println!("Version string pointer: {:?}", version_ptr);

        println!(
            "libobs version: {}",
            CStr::from_ptr(version_ptr)
                .to_str()
                .unwrap_or("Failed to read version string")
        );

        println!("Starting OBS with locale 'en-US'");
        let locale = CString::new("en-US").unwrap();
        println!("Locale pointer: {:?}", locale.as_ptr());

        let startup_result = libobs::obs_startup(locale.as_ptr(), ptr::null(), ptr::null_mut());

        if !startup_result {
            panic!("error on libobs startup");
        }
        println!("OBS startup successful");

        let curr_exe = current_exe().unwrap();
        let curr_exe = curr_exe.parent().unwrap();

        println!("Adding module path");
        let module_bin_path =
            CString::new(curr_exe.join("./obs-plugins/64bit/").to_str().unwrap()).unwrap();
        let module_path = curr_exe.join("./data/obs-plugins/%module%/");
        let module_data_path = module_path.to_str().unwrap();
        let module_data_path = CString::new(module_data_path).unwrap();
        println!(
            "Module bin path pointer: {:?}",
            module_bin_path.to_str().unwrap()
        );
        println!(
            "Module data path pointer: {:?}",
            module_data_path.to_str().unwrap()
        );

        libobs::obs_add_module_path(module_bin_path.as_ptr(), module_data_path.as_ptr());

        println!("Module paths added successfully");
        let data_path = curr_exe.join("./data/libobs/");
        let data_path = data_path.to_str().unwrap();

        println!("Adding data path {}", data_path);
        let data_path = CString::new(data_path).unwrap();
        println!("Data path pointer: {:?}", data_path.as_ptr());
        libobs::obs_add_data_path(data_path.as_ptr());

        println!("Data path added successfully");

        // Audio settings
        println!("Configuring audio settings");
        let avi = libobs::obs_audio_info2 {
            samples_per_sec: 44100,
            speakers: libobs::speaker_layout_SPEAKERS_STEREO,
            max_buffering_ms: 0,
            fixed_buffering: false,
        };
        println!("Resetting audio system");
        let reset_audio_result = libobs::obs_reset_audio2(&avi);
        println!("Audio reset result: {}", reset_audio_result);

        // Video settings - scene rendering resolution
        println!("Configuring video settings");
        let main_width = 1920;
        let main_height = 1080;

        #[cfg(target_os = "windows")]
        let graphics_module = CString::new("libobs-d3d11.dll").unwrap();
        #[cfg(not(target_os = "windows"))]
        let graphics_module = CString::new("libobs-opengl").unwrap();

        println!("Graphics module: {:?}", graphics_module.as_ptr());

        let mut ovi = libobs::obs_video_info {
            adapter: 0,
            #[cfg(target_os = "windows")]
            graphics_module: graphics_module.as_ptr(),
            #[cfg(not(target_os = "windows"))]
            graphics_module: graphics_module.as_ptr(),
            fps_num: 60,
            fps_den: 1,
            base_width: main_width,
            base_height: main_height,
            output_width: main_width,
            output_height: main_height,
            output_format: libobs::video_format_VIDEO_FORMAT_NV12,
            gpu_conversion: true,
            colorspace: libobs::video_colorspace_VIDEO_CS_DEFAULT,
            range: libobs::video_range_type_VIDEO_RANGE_DEFAULT,
            scale_type: libobs::obs_scale_type_OBS_SCALE_BILINEAR,
        };

        println!("Resetting video system");
        let reset_video_code = libobs::obs_reset_video(&mut ovi);
        if reset_video_code != 0 {
            panic!("error on libobs reset video: {}", reset_video_code);
        }
        println!("Video reset successful");

        // Load modules
        println!("Loading all modules");
        libobs::obs_load_all_modules();
        println!("Logging loaded modules");
        libobs::obs_log_loaded_modules();
        println!("Post-loading modules");
        libobs::obs_post_load_modules();
        println!("Module loading complete");

        let scene = libobs::obs_scene_create(CString::new("Test scene").unwrap().as_ptr());
        let scene_source = libobs::obs_scene_get_source(scene);
        libobs::obs_source_release(scene_source);

        libobs::obs_scene_release(scene);

        // Clear sources
        libobs::obs_set_output_source(0, ptr::null_mut()); // 0 = VIDEO CHANNEL
        libobs::obs_set_output_source(1, ptr::null_mut()); // 1 = Audio channel
        libobs::obs_remove_data_path(data_path.as_ptr());
        libobs::obs_shutdown();

        println!(
            "OBS shutdown completed with {} memleaks",
            libobs::bnum_allocs()
        );
    }
}

pub(crate) unsafe extern "C" fn log_handler<V>(
    log_level: std::os::raw::c_int,
    msg: *const i8,
    args: *mut V,
    _params: *mut c_void,
) {
    // Simple logger that prints directly to console
    // In a real-world application, you would use vsnprintf to format the message properly
    let log_level = log_level as libobs::_bindgen_ty_1;
    let level_str = match log_level {
        libobs::LOG_ERROR => "ERROR",
        libobs::LOG_WARNING => "WARNING",
        libobs::LOG_INFO => "INFO",
        libobs::LOG_DEBUG => "DEBUG",
        _ => "UNKNOWN",
    };

    let formatted = vsprintf::vsprintf(msg, args as *mut _);
    if formatted.is_err() {
        eprintln!("Failed to format log message");
        return;
    }
    println!("[{}] {}", level_str, formatted.unwrap());
}
