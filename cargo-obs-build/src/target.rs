use anyhow::{anyhow, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObsTargetOs {
    Windows,
    Macos,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObsTargetArch {
    X86_64,
    Aarch64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ObsBuildTarget {
    pub(crate) os: ObsTargetOs,
    pub(crate) arch: ObsTargetArch,
}

impl ObsBuildTarget {
    pub(crate) fn detect() -> anyhow::Result<Self> {
        let triple = std::env::var("TARGET")
            .ok()
            .or_else(|| std::env::var("CARGO_BUILD_TARGET").ok());

        let os = std::env::var("OBS_BUILD_TARGET_OS")
            .ok()
            .or_else(|| std::env::var("CARGO_CFG_TARGET_OS").ok())
            .or_else(|| {
                triple
                    .as_deref()
                    .and_then(os_from_triple)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| std::env::consts::OS.to_owned());
        let arch = std::env::var("OBS_BUILD_TARGET_ARCH")
            .ok()
            .or_else(|| std::env::var("CARGO_CFG_TARGET_ARCH").ok())
            .or_else(|| {
                triple
                    .as_deref()
                    .and_then(arch_from_triple)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| std::env::consts::ARCH.to_owned());

        Self::parse(&os, &arch)
    }

    pub(crate) fn parse(os: &str, arch: &str) -> anyhow::Result<Self> {
        let os = match os {
            "windows" => ObsTargetOs::Windows,
            "macos" | "darwin" => ObsTargetOs::Macos,
            "linux" => ObsTargetOs::Linux,
            other => bail!("Unsupported OBS target operating system: {other}"),
        };
        let arch = match arch {
            "x86_64" | "x64" | "amd64" => ObsTargetArch::X86_64,
            "aarch64" | "arm64" => ObsTargetArch::Aarch64,
            other => bail!("Unsupported OBS target architecture: {other}"),
        };
        Ok(Self { os, arch })
    }

    pub(crate) const fn display_name(self) -> &'static str {
        match self.os {
            ObsTargetOs::Windows => "Windows",
            ObsTargetOs::Macos => "macOS",
            ObsTargetOs::Linux => "Linux",
        }
    }

    pub(crate) fn require_native_macos_host(self) -> anyhow::Result<()> {
        if self.os == ObsTargetOs::Macos && std::env::consts::OS != "macos" {
            return Err(anyhow!(
                "Preparing official macOS OBS DMGs requires a macOS host (hdiutil/ditto). \
                 Set up the binaries on macOS, or use a pre-populated target directory."
            ));
        }
        Ok(())
    }
}

fn os_from_triple(triple: &str) -> Option<&'static str> {
    if triple.contains("windows") {
        Some("windows")
    } else if triple.contains("apple-darwin") {
        Some("macos")
    } else if triple.contains("linux") {
        Some("linux")
    } else {
        None
    }
}

fn arch_from_triple(triple: &str) -> Option<&'static str> {
    if triple.starts_with("x86_64-") {
        Some("x86_64")
    } else if triple.starts_with("aarch64-") {
        Some("aarch64")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_targets() {
        assert_eq!(
            ObsBuildTarget::parse("macos", "aarch64").unwrap(),
            ObsBuildTarget {
                os: ObsTargetOs::Macos,
                arch: ObsTargetArch::Aarch64,
            }
        );
        assert_eq!(
            ObsBuildTarget::parse("windows", "x86_64").unwrap().os,
            ObsTargetOs::Windows
        );
        assert_eq!(
            ObsBuildTarget::parse("linux", "amd64").unwrap().arch,
            ObsTargetArch::X86_64
        );
    }

    #[test]
    fn triple_detection_helpers_cover_supported_platforms() {
        assert_eq!(os_from_triple("aarch64-apple-darwin"), Some("macos"));
        assert_eq!(os_from_triple("x86_64-unknown-linux-gnu"), Some("linux"));
        assert_eq!(arch_from_triple("aarch64-apple-darwin"), Some("aarch64"));
    }
}
