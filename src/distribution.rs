use crate::target::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    TarGz,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NodePlatform {
    name: &'static str,
    archive_format: ArchiveFormat,
    pub(crate) index_file: &'static str,
}

impl NodePlatform {
    pub(crate) fn current() -> Result<Self, String> {
        let environment = if cfg!(target_env = "gnu") { "gnu" } else { "" };
        Self::for_platform(std::env::consts::OS, std::env::consts::ARCH, environment)
    }

    pub(crate) fn for_platform(
        os: &str,
        architecture: &str,
        environment: &str,
    ) -> Result<Self, String> {
        let (name, archive_format, index_file) = match (os, architecture, environment) {
            ("windows", "x86_64", _) => ("win-x64", ArchiveFormat::Zip, "win-x64-zip"),
            ("windows", "aarch64", _) => ("win-arm64", ArchiveFormat::Zip, "win-arm64-zip"),
            ("macos", "x86_64", _) => ("darwin-x64", ArchiveFormat::TarGz, "osx-x64-tar"),
            ("macos", "aarch64", _) => ("darwin-arm64", ArchiveFormat::TarGz, "osx-arm64-tar"),
            ("linux", "x86_64", "gnu") => ("linux-x64", ArchiveFormat::TarGz, "linux-x64"),
            ("linux", "aarch64", "gnu") => ("linux-arm64", ArchiveFormat::TarGz, "linux-arm64"),
            _ => {
                return Err(format!(
                    "unsupported Node.js distribution platform: os={os}, architecture={architecture}, environment={environment}"
                ));
            }
        };
        Ok(Self {
            name,
            archive_format,
            index_file,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeDistribution {
    pub archive_format: ArchiveFormat,
    pub archive_filename: String,
    pub archive_url: String,
    pub checksum_url: String,
}

impl NodeDistribution {
    pub fn for_current_build(version: Version) -> Result<Self, String> {
        Ok(Self::for_node_platform(version, NodePlatform::current()?))
    }

    #[cfg(test)]
    fn for_platform(
        version: Version,
        os: &str,
        architecture: &str,
        environment: &str,
    ) -> Result<Self, String> {
        Ok(Self::for_node_platform(
            version,
            NodePlatform::for_platform(os, architecture, environment)?,
        ))
    }

    fn for_node_platform(version: Version, platform: NodePlatform) -> Self {
        let archive_format = platform.archive_format;
        let extension = match archive_format {
            ArchiveFormat::Zip => "zip",
            ArchiveFormat::TarGz => "tar.gz",
        };
        let archive_filename = format!("node-v{version}-{}.{extension}", platform.name);
        let base_url = format!("https://nodejs.org/dist/v{version}/");
        Self {
            archive_format,
            archive_url: format!("{base_url}{archive_filename}"),
            checksum_url: format!("{base_url}SHASUMS256.txt"),
            archive_filename,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ArchiveFormat, NodeDistribution};
    use crate::target::{Target, Version};

    #[test]
    fn supported_platforms_select_exact_official_archives_and_urls() {
        let version = "node@22.0.0".parse::<Target>().unwrap().version;
        for (os, architecture, environment, filename, archive_format) in [
            (
                "windows",
                "x86_64",
                "msvc",
                "node-v22.0.0-win-x64.zip",
                ArchiveFormat::Zip,
            ),
            (
                "windows",
                "aarch64",
                "msvc",
                "node-v22.0.0-win-arm64.zip",
                ArchiveFormat::Zip,
            ),
            (
                "macos",
                "x86_64",
                "",
                "node-v22.0.0-darwin-x64.tar.gz",
                ArchiveFormat::TarGz,
            ),
            (
                "macos",
                "aarch64",
                "",
                "node-v22.0.0-darwin-arm64.tar.gz",
                ArchiveFormat::TarGz,
            ),
            (
                "linux",
                "x86_64",
                "gnu",
                "node-v22.0.0-linux-x64.tar.gz",
                ArchiveFormat::TarGz,
            ),
            (
                "linux",
                "aarch64",
                "gnu",
                "node-v22.0.0-linux-arm64.tar.gz",
                ArchiveFormat::TarGz,
            ),
        ] {
            let distribution =
                NodeDistribution::for_platform(version, os, architecture, environment).unwrap();
            assert_eq!(distribution.archive_format, archive_format);
            assert_eq!(distribution.archive_filename, filename);
            assert_eq!(
                distribution.archive_url,
                format!("https://nodejs.org/dist/v22.0.0/{filename}")
            );
            assert_eq!(
                distribution.checksum_url,
                "https://nodejs.org/dist/v22.0.0/SHASUMS256.txt"
            );
        }
    }

    #[test]
    fn unsupported_platforms_return_errors_without_fallback() {
        let version = "node@22.0.0".parse::<Target>().unwrap().version;
        for (os, architecture, environment) in [
            ("linux", "x86_64", "musl"),
            ("linux", "aarch64", "musl"),
            ("linux", "x86_64", ""),
            ("linux", "arm", "gnu"),
            ("linux", "riscv64", "gnu"),
            ("windows", "x86", "msvc"),
            ("macos", "x86", ""),
            ("freebsd", "x86_64", ""),
        ] {
            assert_eq!(
                NodeDistribution::for_platform(version, os, architecture, environment),
                Err(format!(
                    "unsupported Node.js distribution platform: os={os}, architecture={architecture}, environment={environment}"
                ))
            );
        }
    }

    #[test]
    fn version_components_are_rendered_without_resolution_or_fallback() {
        for version in [
            Version {
                major: 0,
                minor: 0,
                patch: 0,
            },
            Version {
                major: u32::MAX,
                minor: u32::MAX,
                patch: u32::MAX,
            },
        ] {
            let distribution =
                NodeDistribution::for_platform(version, "windows", "aarch64", "msvc").unwrap();
            assert_eq!(
                distribution.archive_filename,
                format!("node-v{version}-win-arm64.zip")
            );
            assert_eq!(
                distribution.archive_url,
                format!("https://nodejs.org/dist/v{version}/node-v{version}-win-arm64.zip")
            );
            assert_eq!(
                distribution.checksum_url,
                format!("https://nodejs.org/dist/v{version}/SHASUMS256.txt")
            );
        }
    }

    #[test]
    fn current_build_selection_uses_compile_time_platform() {
        let version = "node@22.0.0".parse::<Target>().unwrap().version;
        let environment = if cfg!(target_env = "gnu") { "gnu" } else { "" };
        assert_eq!(
            NodeDistribution::for_current_build(version),
            NodeDistribution::for_platform(
                version,
                std::env::consts::OS,
                std::env::consts::ARCH,
                environment,
            )
        );
    }
}
