use anyhow::bail;
use cargo_obs_build::get_meta_info;

use crate::args::InstallArgs;

pub fn linux_obs_system_install(opts: InstallArgs) -> anyhow::Result<()> {
    let mut tag = opts.tag;
    get_meta_info(&mut None, &mut tag)?;

    if !opts.skip_check {
        // Check if system is Ubuntu/Debian based
        let os_release =
            std::fs::read_to_string("/etc/os-release").expect("Failed to read /etc/os-release");
        if !os_release.contains("ID=ubuntu") && !os_release.contains("ID=debian") {
            bail!("This installation script only supports Ubuntu/Debian based systems. Use flag '--skip-check' to skip this check.");
        }
    }

    let script = include_str!("install_obs_ubuntu.sh");
    std::fs::write("/tmp/install_obs.sh", script).expect("Failed to write install script");
    let mut cmd = std::process::Command::new("bash");
    cmd.arg("/tmp/install_obs.sh");

    cmd.env(
        "OBS_GIT_REPO",
        format!("https://github.com/{}.git", opts.repo_id),
    );
    if let Some(tag) = &tag {
        cmd.env("OBS_BUILD_TAG", tag);
    }

    let status = cmd.status().expect("Failed to execute install script");

    if !status.success() {
        bail!("OBS installation script failed");
    }

    Ok(())
}
