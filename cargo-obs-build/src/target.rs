use anyhow::{bail, Context};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObsTargetOs {
    Windows,
    Linux,
    MacOs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObsTargetArch {
    X86_64,
    Aarch64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObsTarget {
    triple: String,
    pub(crate) os: ObsTargetOs,
    pub(crate) arch: ObsTargetArch,
}

impl ObsTarget {
    pub(crate) fn from_triple(triple: &str) -> anyhow::Result<Self> {
        let arch = match triple.split('-').next().unwrap_or_default() {
            "x86_64" => ObsTargetArch::X86_64,
            "aarch64" => ObsTargetArch::Aarch64,
            other => bail!("unsupported OBS target architecture `{other}` in `{triple}`"),
        };

        let os = if triple.contains("windows") {
            ObsTargetOs::Windows
        } else if triple.contains("linux") {
            ObsTargetOs::Linux
        } else if triple.contains("apple-darwin") {
            ObsTargetOs::MacOs
        } else {
            bail!("unsupported OBS target operating system in `{triple}`");
        };

        Ok(Self {
            triple: triple.to_owned(),
            os,
            arch,
        })
    }

    pub(crate) fn detect(explicit: Option<&str>) -> anyhow::Result<Self> {
        if let Some(triple) = explicit {
            return Self::from_triple(triple);
        }
        if let Ok(triple) = std::env::var("TARGET") {
            return Self::from_triple(&triple).context("parsing Cargo TARGET");
        }

        let arch = match std::env::consts::ARCH {
            "x86_64" => "x86_64",
            "aarch64" => "aarch64",
            other => bail!("unsupported host architecture `{other}`"),
        };
        let triple = match std::env::consts::OS {
            "windows" => format!("{arch}-pc-windows-msvc"),
            "linux" => format!("{arch}-unknown-linux-gnu"),
            "macos" => format!("{arch}-apple-darwin"),
            other => bail!("unsupported host operating system `{other}`"),
        };
        Self::from_triple(&triple)
    }

    pub(crate) fn triple(&self) -> &str {
        &self.triple
    }

    pub(crate) fn cache_key(&self) -> String {
        self.triple.replace(['/', '\\'], "_")
    }

    pub(crate) fn windows_asset_arch(&self) -> anyhow::Result<&'static str> {
        if self.os != ObsTargetOs::Windows {
            bail!(
                "prebuilt OBS archive download currently supports Windows targets only; got `{}`",
                self.triple
            );
        }
        Ok(match self.arch {
            ObsTargetArch::X86_64 => "x64",
            ObsTargetArch::Aarch64 => "arm64",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_apple_arm_from_windows_arm() {
        let apple = ObsTarget::from_triple("aarch64-apple-darwin").unwrap();
        let windows = ObsTarget::from_triple("aarch64-pc-windows-msvc").unwrap();
        assert_eq!(apple.os, ObsTargetOs::MacOs);
        assert_eq!(windows.os, ObsTargetOs::Windows);
        assert!(apple.windows_asset_arch().is_err());
        assert_eq!(windows.windows_asset_arch().unwrap(), "arm64");
    }

    #[test]
    fn parses_linux_x86_64() {
        let target = ObsTarget::from_triple("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(target.os, ObsTargetOs::Linux);
        assert_eq!(target.arch, ObsTargetArch::X86_64);
    }

    #[test]
    fn rejects_unknown_architecture() {
        assert!(ObsTarget::from_triple("riscv64gc-unknown-linux-gnu").is_err());
    }
}
