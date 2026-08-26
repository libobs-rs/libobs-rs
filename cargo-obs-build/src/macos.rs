use crate::util::copy_to_dir;
use anyhow::{anyhow, bail};
use log::{debug, info};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

struct DmgMount {
    path: PathBuf,
    attached: bool,
}

impl DmgMount {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            attached: false,
        }
    }

    fn mark_attached(&mut self) {
        self.attached = true;
    }

    fn detach(&mut self) -> anyhow::Result<()> {
        if self.attached {
            let detached = Command::new("hdiutil")
                .arg("detach")
                .arg(&self.path)
                .output()?;
            if !detached.status.success() {
                let forced = Command::new("hdiutil")
                    .args(["detach", "-force"])
                    .arg(&self.path)
                    .output()?;
                if !forced.status.success() {
                    bail!(
                        "Failed to detach OBS DMG at {}: {}; forced detach also failed: {}",
                        self.path.display(),
                        String::from_utf8_lossy(&detached.stderr),
                        String::from_utf8_lossy(&forced.stderr)
                    );
                }
            }
            self.attached = false;
        }

        if self.path.exists() {
            fs::remove_dir(&self.path)?;
        }
        Ok(())
    }
}

impl Drop for DmgMount {
    fn drop(&mut self) {
        if let Err(error) = self.detach() {
            log::error!(
                "Failed to clean up OBS DMG mount {}: {error:#}",
                self.path.display()
            );
        }
    }
}

pub(crate) fn extract_dmg(dmg_path: &Path, output_dir: &Path) -> anyhow::Result<()> {
    if std::env::consts::OS != "macos" {
        bail!("DMG extraction requires macOS (hdiutil and ditto)");
    }

    let mount_path = std::env::temp_dir().join(format!(
        "cargo-obs-build-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    // Fail rather than reusing an existing path. The guard tracks whether this
    // process actually attached the DMG, so attach failures never detach an
    // unrelated volume and all later errors still unmount our volume.
    fs::create_dir(&mount_path)?;
    let mut mount = DmgMount::new(mount_path.clone());

    let mounted = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-readonly", "-mountpoint"])
        .arg(&mount_path)
        .arg(dmg_path)
        .output()?;
    if !mounted.status.success() {
        bail!(
            "Failed to mount OBS DMG: {}",
            String::from_utf8_lossy(&mounted.stderr)
        );
    }
    mount.mark_attached();

    let extraction_result = (|| -> anyhow::Result<()> {
        let contents = mount_path.join("OBS.app/Contents");
        if !contents.is_dir() {
            bail!("Mounted DMG does not contain OBS.app/Contents");
        }
        fs::create_dir_all(output_dir)?;

        let frameworks = contents.join("Frameworks");
        if frameworks.is_dir() {
            copy_to_dir(&frameworks, output_dir, None)?;
            let libobs_resources = frameworks.join("libobs.framework/Versions/A/Resources");
            if libobs_resources.is_dir() {
                copy_to_dir(&libobs_resources, &output_dir.join("data/libobs"), None)?;
            }
        }

        let plugins = contents.join("PlugIns");
        if plugins.is_dir() {
            let plugin_out = output_dir.join("obs-plugins");
            copy_to_dir(&plugins, &plugin_out, None)?;
            let data_out = output_dir.join("data/obs-plugins");
            for entry in fs::read_dir(&plugins)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|x| x.to_str()) != Some("plugin") {
                    continue;
                }
                let resources = path.join("Contents/Resources");
                if resources.is_dir() {
                    let name = safe_plugin_stem(&path)?;
                    copy_to_dir(&resources, &data_out.join(name), None)?;
                }
            }
        }

        let resources = contents.join("Resources");
        if resources.is_dir() {
            copy_to_dir(&resources, &output_dir.join("data"), None)?;
        }

        let macos = contents.join("MacOS");
        if macos.is_dir() {
            for entry in fs::read_dir(&macos)? {
                let entry = entry?;
                if entry.file_name() == "OBS" {
                    continue;
                }
                copy_item_preserving_macos_metadata(
                    &entry.path(),
                    &output_dir.join(entry.file_name()),
                )?;
            }
        }

        Ok(())
    })();

    let detach_result = mount.detach();
    match (extraction_result, detach_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(detach_error)) => Err(detach_error),
        (Err(error), Err(detach_error)) => Err(anyhow!(
            "{error:#}; additionally failed to detach OBS DMG: {detach_error:#}"
        )),
    }
}

fn safe_plugin_stem(path: &Path) -> anyhow::Result<&std::ffi::OsStr> {
    let stem = path
        .file_stem()
        .ok_or_else(|| anyhow!("Invalid plugin bundle name: {}", path.display()))?;
    if stem.is_empty() || stem == "." || stem == ".." {
        bail!("Unsafe plugin bundle name: {}", path.display());
    }
    Ok(stem)
}

pub(crate) fn setup_files(output_dir: &Path) -> anyhow::Result<()> {
    if std::env::consts::OS != "macos" {
        return Ok(());
    }

    // OBS's graphics module loader asks for .so module names even though macOS ships dylibs.
    for dylib in ["libobs-opengl.dylib", "libobs-metal.dylib"] {
        let target = output_dir.join(dylib);
        if !target.exists() {
            continue;
        }
        let so = output_dir.join(dylib.replace(".dylib", ".so"));
        if so.symlink_metadata().is_ok() {
            fs::remove_file(&so)?;
        }
        create_symlink(Path::new(dylib), &so)?;
    }

    // Helpers are launched from target/{profile} and target/{profile}/deps. Keep a local
    // Frameworks view at the former so their @rpath resolution works outside OBS.app.
    let frameworks = output_dir.join("Frameworks");
    fs::create_dir_all(&frameworks)?;
    for entry in fs::read_dir(output_dir)? {
        let entry = entry?;
        let path = entry.path();
        let is_framework = path.extension().and_then(|x| x.to_str()) == Some("framework");
        let is_dylib = path.extension().and_then(|x| x.to_str()) == Some("dylib");
        if !is_framework && !is_dylib {
            continue;
        }
        let name = entry.file_name();
        let link = frameworks.join(&name);
        if link.symlink_metadata().is_ok() {
            if link.is_dir() && !link.is_symlink() {
                fs::remove_dir_all(&link)?;
            } else {
                fs::remove_file(&link)?;
            }
        }
        create_symlink(&Path::new("..").join(&name), &link)?;
    }

    let helper = output_dir.join("obs-ffmpeg-mux");
    if helper.is_file() {
        // Official helpers are signed for the app bundle. Once moved out of the bundle, add
        // local rpaths and re-sign ad-hoc so macOS accepts the changed Mach-O binary.
        for rpath in [
            "@executable_path",
            "@executable_path/..",
            "@loader_path",
            "@loader_path/..",
        ] {
            let output = Command::new("install_name_tool")
                .args(["-add_rpath", rpath])
                .arg(&helper)
                .output()?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("would duplicate path") || stderr.contains("already exists") {
                    debug!("rpath {rpath} is already present on {}", helper.display());
                } else {
                    bail!(
                        "Failed to add rpath {rpath} to {}: {}",
                        helper.display(),
                        stderr
                    );
                }
            }
        }
        let status = Command::new("codesign")
            .args(["--force", "--sign", "-"])
            .arg(&helper)
            .status()?;
        if !status.success() {
            bail!("Failed to ad-hoc sign {}", helper.display());
        }
        info!("Prepared macOS OBS helper {}", helper.display());
    }

    Ok(())
}

pub(crate) fn is_inside_signed_bundle(path: &Path) -> bool {
    path.parent().is_some_and(|parent| {
        parent.ancestors().any(|ancestor| {
            matches!(
                ancestor.extension().and_then(|x| x.to_str()),
                Some("framework" | "plugin")
            )
        })
    })
}

fn copy_item_preserving_macos_metadata(src: &Path, dst: &Path) -> anyhow::Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let output = Command::new("ditto").arg(src).arg(dst).output()?;
    if !output.status.success() {
        bail!("ditto failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(())
}

#[cfg(unix)]
fn create_symlink(src: &Path, dst: &Path) -> anyhow::Result<()> {
    std::os::unix::fs::symlink(src, dst)?;
    Ok(())
}

#[cfg(not(unix))]
fn create_symlink(_src: &Path, _dst: &Path) -> anyhow::Result<()> {
    bail!("macOS OBS setup requires a Unix host")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_bundle_descendants_are_not_cleanup_candidates() {
        assert!(is_inside_signed_bundle(Path::new(
            "/tmp/libobs.framework/Versions/A/Resources/defaults"
        )));
        assert!(is_inside_signed_bundle(Path::new(
            "/tmp/mac-capture.plugin/Contents/MacOS/mac-capture"
        )));
        assert!(!is_inside_signed_bundle(Path::new("/tmp/libobs.framework")));
        assert!(!is_inside_signed_bundle(Path::new("/tmp/obs-ffmpeg-mux")));
    }

    #[test]
    fn plugin_resource_destination_rejects_parent_components() {
        assert_eq!(
            safe_plugin_stem(Path::new("mac-capture.plugin")).unwrap(),
            "mac-capture"
        );
        assert!(safe_plugin_stem(Path::new("...plugin")).is_err());
    }
}
