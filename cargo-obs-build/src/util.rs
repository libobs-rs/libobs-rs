use std::{fs, path::Path};

use walkdir::WalkDir;

pub fn copy_to_dir(src: &Path, out: &Path, except_dir: Option<&Path>) -> anyhow::Result<()> {
    if std::env::consts::OS == "macos" && except_dir.is_none() {
        return copy_to_dir_with_ditto(src, out);
    }

    for entry in WalkDir::new(src) {
        if entry.is_err() {
            continue;
        }

        let entry = entry?;
        let path = entry.path();

        if except_dir.is_some_and(|e| path.starts_with(e)) {
            continue;
        }

        let copy_to = out.join(path.strip_prefix(src)?);
        if path.is_dir() {
            fs::create_dir_all(&copy_to)?;
            continue;
        }

        fs::copy(entry.path(), copy_to)?;
    }

    Ok(())
}

pub fn delete_all_except(src: &Path, except_dir: Option<&Path>) -> anyhow::Result<()> {
    for entry in fs::read_dir(src)? {
        if entry.is_err() {
            continue;
        }

        let entry = entry?;
        let path = entry.path();

        if except_dir.is_some_and(|e| path.starts_with(e)) {
            continue;
        }

        if path.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}

fn copy_to_dir_with_ditto(src: &Path, out: &Path) -> anyhow::Result<()> {
    use std::process::Command;

    fs::create_dir_all(out)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let dest = out.join(entry.file_name());
        if dest.symlink_metadata().is_ok() {
            if dest.is_dir() && !dest.is_symlink() {
                fs::remove_dir_all(&dest)?;
            } else {
                fs::remove_file(&dest)?;
            }
        }
        let output = Command::new("ditto")
            .arg(entry.path())
            .arg(&dest)
            .output()?;
        if !output.status.success() {
            anyhow::bail!(
                "ditto failed for {}: {}",
                entry.path().display(),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    Ok(())
}
