//! The two platforms a release carries and the names each one's artifact set uses. Windows keeps
//! the names it has always had; macOS gets its own (plan 11 #macos-release-artifact).
//! plan_ref: docs/plan/11_testing_and_release.md#release-attachment-set

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Platform {
    WindowsX64,
    MacOsArm64,
}

impl Platform {
    pub const ALL: [Self; 2] = [Self::WindowsX64, Self::MacOsArm64];

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "windows-x64" => Ok(Self::WindowsX64),
            "macos-arm64" => Ok(Self::MacOsArm64),
            _ => Err(format!(
                "unknown platform `{value}`; expected windows-x64 or macos-arm64"
            )),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::WindowsX64 => "windows-x64",
            Self::MacOsArm64 => "macos-arm64",
        }
    }

    /// The host crate whose normal dependency graph the package's notices list.
    pub const fn host_package(self) -> &'static str {
        match self {
            Self::WindowsX64 => "stickymd-win",
            Self::MacOsArm64 => "stickymd-mac",
        }
    }

    pub const fn target(self) -> &'static str {
        match self {
            Self::WindowsX64 => "x86_64-pc-windows-msvc",
            Self::MacOsArm64 => "aarch64-apple-darwin",
        }
    }

    pub fn archive_name(self, version: &str) -> String {
        format!("StickyMD-{version}-{}-portable.zip", self.label())
    }

    pub const fn checksum_file(self) -> &'static str {
        match self {
            Self::WindowsX64 => "SHA256SUMS.txt",
            Self::MacOsArm64 => "SHA256SUMS-macos-arm64.txt",
        }
    }

    pub const fn sbom_file(self) -> &'static str {
        match self {
            Self::WindowsX64 => "SBOM.spdx.json",
            Self::MacOsArm64 => "SBOM-macos-arm64.spdx.json",
        }
    }

    /// The release workflow that builds this platform's set, as GitHub names the run.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub const fn workflow(self) -> &'static str {
        match self {
            Self::WindowsX64 => "release",
            Self::MacOsArm64 => "release-macos",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_windows_names_are_the_ones_already_published() {
        let windows = Platform::WindowsX64;
        assert_eq!(
            windows.archive_name("0.2.0"),
            "StickyMD-0.2.0-windows-x64-portable.zip"
        );
        assert_eq!(windows.checksum_file(), "SHA256SUMS.txt");
        assert_eq!(windows.sbom_file(), "SBOM.spdx.json");
        assert_eq!(windows.workflow(), "release");
    }

    #[test]
    fn the_macos_names_are_distinct_from_every_windows_name() {
        let macos = Platform::MacOsArm64;
        assert_eq!(
            macos.archive_name("0.3.0"),
            "StickyMD-0.3.0-macos-arm64-portable.zip"
        );
        assert_eq!(macos.checksum_file(), "SHA256SUMS-macos-arm64.txt");
        assert_eq!(macos.sbom_file(), "SBOM-macos-arm64.spdx.json");
        for platform in Platform::ALL {
            assert_eq!(Platform::parse(platform.label()), Ok(platform));
        }
        assert!(Platform::parse("macos").is_err());
    }
}
