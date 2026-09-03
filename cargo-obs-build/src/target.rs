use anyhow::{anyhow, bail, Context};

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

// Backwards compatibility alias for PR 187 which used `ObsTarget`
#[allow(dead_code)]
pub(crate) type ObsTarget = ObsBuildTarget;

impl ObsBuildTarget {
    /// Parse a full Cargo target triple (e.g. `x86_64-pc-windows-msvc`)
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
            ObsTargetOs::Macos
        } else {
            bail!("unsupported OBS target operating system in `{triple}`");
        };

        Ok(Self { os, arch })
    }

    /// Detect the build target. When `explicit` is Some, it is treated as a
    /// Cargo triple and takes precedence over environment variables. This keeps
    /// `ObsBuildConfig.target` (PR 187) working while also honoring the
    /// environment-variable based detection from main (OBS_BUILD_TARGET_OS/ARCH,
    /// CARGO_CFG_TARGET_OS/ARCH, TARGET, CARGO_BUILD_TARGET).
    #[allow(dead_code)]
    pub(crate) fn detect() -> anyhow::Result<Self> {
        Self::detect_with_explicit(None)
    }

    pub(crate) fn detect_with_explicit(explicit: Option<&str>) -> anyhow::Result<Self> {
        if let Some(triple) = explicit {
            return Self::from_triple(triple).context("parsing explicit target triple");
        }
        // Allow explicit OS/arch overrides (main's contract)
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
                "Preparing official macOS OBS DMGs requires a macOS host (hdiutil/ditto).                  Set up the binaries on macOS, or use a pre-populated target directory."
            ));
        }
        Ok(())
    }

    pub(crate) fn triple(&self) -> String {
        let arch = match self.arch {
            ObsTargetArch::X86_64 => "x86_64",
            ObsTargetArch::Aarch64 => "aarch64",
        };
        let os_part = match self.os {
            ObsTargetOs::Windows => "pc-windows-msvc",
            ObsTargetOs::Linux => "unknown-linux-gnu",
            ObsTargetOs::Macos => "apple-darwin",
        };
        format!("{arch}-{os_part}")
    }

    pub(crate) fn cache_key(&self) -> String {
        self.triple().replace(['/', '\\'], "_")
    }

    #[allow(dead_code)]
    pub(crate) fn windows_asset_arch(&self) -> anyhow::Result<&'static str> {
        if self.os != ObsTargetOs::Windows {
            bail!(
                "prebuilt OBS archive download currently supports Windows targets only; got `{}`",
                self.triple()
            );
        }
        Ok(match self.arch {
            ObsTargetArch::X86_64 => "x64",
            ObsTargetArch::Aarch64 => "arm64",
        })
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
    fn distinguishes_apple_arm_from_windows_arm() {
        let apple = ObsBuildTarget::from_triple("aarch64-apple-darwin").unwrap();
        let windows = ObsBuildTarget::from_triple("aarch64-pc-windows-msvc").unwrap();
        assert_eq!(apple.os, ObsTargetOs::Macos);
        assert_eq!(windows.os, ObsTargetOs::Windows);
        assert!(apple.windows_asset_arch().is_err());
        assert_eq!(windows.windows_asset_arch().unwrap(), "arm64");
    }

    #[test]
    fn parses_linux_x86_64() {
        let target = ObsBuildTarget::from_triple("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(target.os, ObsTargetOs::Linux);
        assert_eq!(target.arch, ObsTargetArch::X86_64);
    }

    #[test]
    fn rejects_unknown_architecture() {
        assert!(ObsBuildTarget::from_triple("riscv64gc-unknown-linux-gnu").is_err());
    }

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

    #[test]
    fn cache_key_is_triple_based() {
        let t = ObsBuildTarget::parse("windows", "x86_64").unwrap();
        assert_eq!(t.cache_key(), "x86_64-pc-windows-msvc");
        let m = ObsBuildTarget::parse("macos", "aarch64").unwrap();
        assert_eq!(m.cache_key(), "aarch64-apple-darwin");
    }
}
