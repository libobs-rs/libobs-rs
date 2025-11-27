fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // This should be the whole directory, but cargo would have to check the whole directory with a lot of files which takes long
    println!("cargo:rerun-if-changed=headers/wrapper.h");
    println!("cargo:rerun-if-changed=headers/display_capture.h");
    println!("cargo:rerun-if-changed=headers/game_capture.h");
    println!("cargo:rerun-if-changed=headers/vec4.c");
    println!("cargo:rerun-if-changed=headers/window_capture.h");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-env-changed=LIBOBS_PATH");

    // For development, you can set LIBOBS_PATH to point to your custom libobs
    if let Ok(path) = std::env::var("LIBOBS_PATH") {
        println!("cargo:rustc-link-search=native={}", path);
        println!("cargo:rustc-link-lib=dylib=obs");
    } else {
        // On Linux, try to link against system libobs
        // On Windows, look for obs.dll in the manifest directory
        #[cfg(target_family = "windows")]
        {
            println!(
                "cargo:rustc-link-search=native={}",
                env!("CARGO_MANIFEST_DIR")
            );
            println!("cargo:rustc-link-lib=dylib=obs");
        }

        #[cfg(target_os = "linux")]
        {
            /*
            let header = include_str!("./headers/obs/obs-config.h");
                        let mut major = "";
                        let mut minor = "";
                        let mut patch = "";
                        for line in header.lines() {
                            if line.starts_with("#define LIBOBS_API_MAJOR_VER") {
                                major = line.split_whitespace().last().unwrap();
                            } else if line.starts_with("#define LIBOBS_API_MINOR_VER") {
                                minor = line.split_whitespace().last().unwrap();
                            } else if line.starts_with("#define LIBOBS_API_PATCH_VER") {
                                patch = line.split_whitespace().last().unwrap();
                            }
                        }

                        let version = format!("{}.{}.{}", major, minor, patch);
                        */

            let version = "30.0.0"; // Manually set for now, update when updating obs-studio version
            pkg_config::Config::new()
                .atleast_version(version)
                .probe("libobs")
                .unwrap_or_else(|_| panic!("Could not find libobs via pkg-config. Make sure you have installed obs-studio to the system. A build/installation guide can be found at https://github.com/obsproject/obs-studio/wiki/Build-Instructions-For-Linux. If you are using Ubuntu, you can also run 'cargo obs-build install'. The version must be at least {}", version));
        }
    }

    #[cfg(any(feature = "generate_bindings", not(target_family = "windows")))]
    bindings::generate_bindings();
}

#[cfg(any(feature = "generate_bindings", not(target_family = "windows")))]
mod bindings {
    use std::{collections::HashSet, path::PathBuf};

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
        ignored.insert("FE_INVALID".to_string());
        ignored.insert("FE_DIVBYZERO".to_string());
        ignored.insert("FE_OVERFLOW".to_string());
        ignored.insert("FE_UNDERFLOW".to_string());
        ignored.insert("FE_INEXACT".to_string());
        ignored.insert("FE_TONEAREST".to_string());
        ignored.insert("FE_DOWNWARD".to_string());
        ignored.insert("FE_UPWARD".to_string());
        ignored.insert("FE_TOWARDZERO".to_string());
        ignored.insert("FP_NORMAL".to_string());
        ignored.insert("FP_SUBNORMAL".to_string());
        ignored.insert("FP_ZERO".to_string());
        ignored.insert("FP_INFINITE".to_string());
        ignored.insert("FP_NAN".to_string());
        IgnoreMacros(ignored)
    }

    pub fn generate_bindings() {
        let builder = bindgen::builder()
            .header("headers/wrapper.h")
            .blocklist_function("^_.*")
            .clang_arg(format!("-I{}", "headers/obs"));

        #[cfg(all(not(target_os = "linux"), not(feature = "include_win_bindings")))]
        let builder = builder
            .blocklist_function("blogva")
            .blocklist_function("^ms_.*")
            .blocklist_file(".*windows\\.h")
            .blocklist_file(".*winuser\\.h")
            .blocklist_file(".*wingdi\\.h")
            .blocklist_file(".*winnt\\.h")
            .blocklist_file(".*winbase\\.h")
            .blocklist_file(".*Windows Kits.*")
            // Block all MSVC headers except vadefs.h
            .blocklist_file(r".*MSVC.*[\\/]include[\\/][^v].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]v[^a].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]va[^d].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]vad[^e].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]vade[^f].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]vadef[^s].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]vadefs[^.].*")
            .blocklist_file(r".*MSVC.*[\\/]include[\\/]vadefs\.[^h].*");
        //.blocklist_function("_bindgen_ty_2")
        //.ignore_functions()
        //.ignore_methods();
        /*         let builder = to_include.lines().fold(builder, |builder, line| {
            let item = line.trim();
            if item.is_empty() || item.starts_with("//") {
                builder
            } else {
                builder.allowlist_item(item)
            }
        }); */

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

        let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let bindings_path = out_path.join("bindings.rs");
        let bindings = bindings.to_string();
        let lines = bindings.lines().map(|line| {
            if line.trim().starts_with("#[doc") {
                let start_pos = line.find('"').unwrap() + 1;
                let end_pos = line.rfind('"').unwrap();
                let doc = &line[start_pos..end_pos];
                let doc = doc.replace("[", "\\\\[").replace("]", "\\\\]");

                format!("#[doc = \"{}\"]", doc)
            } else {
                line.to_string()
            }
        });

        let bindings = lines.collect::<Vec<_>>().join("\n");
        std::fs::write(&bindings_path, bindings).expect("Couldn't write bindings!");
    }
}
