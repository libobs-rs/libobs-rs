use anyhow::bail;

pub fn linux_obs_system_install(skip_check: bool) -> anyhow::Result<()> {
    if !skip_check {
        // Check if system is Ubuntu/Debian based
        let os_release =
            std::fs::read_to_string("/etc/os-release").expect("Failed to read /etc/os-release");
        if !os_release.contains("ID=ubuntu") && !os_release.contains("ID=debian") {
            bail!("This installation script only supports Ubuntu/Debian based systems. Use flag '--skip-check' to skip this check.");
        }
    }

    let script = include_str!("install_obs_ubuntu.sh");
    std::fs::write("/tmp/install_obs.sh", script).expect("Failed to write install script");
    let status = std::process::Command::new("bash")
        .arg("/tmp/install_obs.sh")
        .status()
        .expect("Failed to execute install script");

    if !status.success() {
        bail!("OBS installation script failed");
    }

    Ok(())
}
