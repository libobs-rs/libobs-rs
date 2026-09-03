use std::collections::HashSet;
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=headers/wrapper.h");
    println!("cargo:rerun-if-changed=headers/display_capture.h");
    println!("cargo:rerun-if-changed=headers/game_capture.h");
    println!("cargo:rerun-if-changed=headers/vec4.c");
    println!("cargo:rerun-if-changed=headers/window_capture.h");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=OBS_VERSION");
    println!("cargo:rerun-if-env-changed=LIBOBS_PATH");

    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if let Ok(path) = env::var("LIBOBS_PATH") {
        println!("cargo:rustc-link-search=native={}", path);
        println!("cargo:rustc-link-lib=dylib=obs");
    } else if target_family == "windows" {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        println!("cargo:rustc-link-search=native={}", manifest_dir);
        println!("cargo:rustc-link-lib=dylib=obs");
    } else if target_os == "linux" {
        let version = include_str!("OBS_VERSION").trim();
        pkg_config::Config::new()
            .atleast_version(version)
            .probe("libobs")
            .unwrap_or_else(|_| {
                panic!(
                    "Could not find libobs via pkg-config. Requires >= {}. See build guide.",
                    version
                )
            });
    } else {
        // Fallback: assume dynamic libobs available via system linker path
        println!("cargo:rustc-link-lib=dylib=obs");
    }

    let feature_generate_bindings = env::var_os("CARGO_FEATURE_GENERATE_BINDINGS").is_some();
    let should_generate_bindings = feature_generate_bindings || target_family != "windows";

    if should_generate_bindings {
        generate_bindings(&target_os);
    }
}

// --- bindings support (previously gated by cfg) ---

#[derive(Debug)]
struct IgnoreMacros(HashSet<String>);

impl bindgen::callbacks::ParseCallbacks for IgnoreMacros {
    fn will_parse_macro(&self, name: &str) -> bindgen::callbacks::MacroParsingBehavior {
        if self.0.contains(name) {
            bindgen::callbacks::MacroParsingBehavior::Ignore
        } else {
            bindgen::callbacks::MacroParsingBehavior::Default
        }
    }
}

fn get_ignored_macros() -> IgnoreMacros {
    let mut ignored = HashSet::new();
    ignored.insert("FE_INVALID".into());
    ignored.insert("FE_DIVBYZERO".into());
    ignored.insert("FE_OVERFLOW".into());
    ignored.insert("FE_UNDERFLOW".into());
    ignored.insert("FE_INEXACT".into());
    ignored.insert("FE_TONEAREST".into());
    ignored.insert("FE_DOWNWARD".into());
    ignored.insert("FE_UPWARD".into());
    ignored.insert("FE_TOWARDZERO".into());
    ignored.insert("FP_NORMAL".into());
    ignored.insert("FP_SUBNORMAL".into());
    ignored.insert("FP_ZERO".into());
    ignored.insert("FP_INFINITE".into());
    ignored.insert("FP_NAN".into());
    IgnoreMacros(ignored)
}

fn generate_bindings(target_os: &str) {
    let include_win_bindings = env::var_os("CARGO_FEATURE_INCLUDE_WIN_BINDINGS").is_some();

    let mut builder = bindgen::builder()
        .header("headers/wrapper.h")
        .blocklist_function("^_.*")
        // Block C runtime symbols that trigger `suspicious_runtime_symbol_definitions`
        // when bindgen emits them with `c_ulonglong` instead of `usize` on Windows
        // (c_ulonglong == u64 == usize on x86_64 win, but lint expects usize).
        // The lint is now allowed in `src/lib.rs` as fallback, but blocking avoids
        // pulling unrelated CRT declarations into the bindings.
        .blocklist_function("memcmp")
        .blocklist_function("memcpy")
        .blocklist_function("memmove")
        .blocklist_function("memset")
        .blocklist_function("strlen")
        .size_t_is_usize(true)
        .clang_arg(format!("-I{}", "headers/obs"));

    // Apply previous windows/MSVC blocklists when not Linux and feature not enabled.
    if target_os != "linux" && !include_win_bindings {
        builder = builder
            .blocklist_function("blogva")
            .blocklist_function("^ms_.*")
            .blocklist_file(".*windows\\.h")
            .blocklist_file(".*winuser\\.h")
            .blocklist_file(".*wingdi\\.h")
            .blocklist_file(".*winnt\\.h")
            .blocklist_file(".*winbase\\.h")
            .blocklist_file(".*Windows Kits.*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/][^v].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]v[^a].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]va[^d].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]vad[^e].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]vade[^f].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]vadef[^s].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]vadefs[^.].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]vadefs\.[^h].*");
    }

    let bindings = builder
        .parse_callbacks(Box::new(get_ignored_macros()))
        .derive_copy(true)
        .derive_debug(true)
        .derive_default(false)
        .derive_partialeq(false)
        .derive_eq(false)
        .derive_partialord(false)
        .derive_ord(false)
        .merge_extern_blocks(true)
        .generate()
        .expect("Error generating bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    let bindings_path = out_path.join("bindings.rs");
    let bindings_str = bindings.to_string();

    let processed = bindings_str
        .lines()
        .map(|line| {
            if line.trim().starts_with("#[doc") {
                if let (Some(start), Some(end)) = (line.find('"'), line.rfind('"')) {
                    let doc = &line[start + 1..end];
                    let doc = doc.replace("[", "\\\\[").replace("]", "\\\\]");
                    format!("#[doc = \"{}\"]", doc)
                } else {
                    line.to_string()
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    std::fs::write(&bindings_path, processed).expect("Couldn't write bindings");
}
