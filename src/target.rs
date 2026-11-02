use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Node,
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node => f.write_str("node"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub tool: Tool,
    pub version: Version,
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.tool, self.version)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VersionRequest {
    target: Target,
    components: usize,
}

impl VersionRequest {
    pub(crate) fn exact(&self) -> Option<Target> {
        (self.components == 3).then_some(self.target)
    }

    pub(crate) fn matches(&self, target: &Target) -> bool {
        self.target.tool == target.tool
            && self.target.version.major == target.version.major
            && (self.components < 2 || self.target.version.minor == target.version.minor)
            && (self.components < 3 || self.target.version.patch == target.version.patch)
    }
}

impl From<Target> for VersionRequest {
    fn from(target: Target) -> Self {
        Self {
            target,
            components: 3,
        }
    }
}

impl fmt::Display for VersionRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let version = self.target.version;
        write!(f, "{}@{}", self.target.tool, version.major)?;
        if self.components >= 2 {
            write!(f, ".{}", version.minor)?;
        }
        if self.components == 3 {
            write!(f, ".{}", version.patch)?;
        }
        Ok(())
    }
}

impl FromStr for VersionRequest {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (tool, version) = input
            .split_once('@')
            .ok_or_else(|| ParseTargetError::InvalidStructure.to_string())?;
        if tool.is_empty() || version.is_empty() || version.contains('@') {
            return Err(ParseTargetError::InvalidStructure.to_string());
        }
        let components = version.split('.').count();
        let target = match components {
            1 => format!("{input}.0.0").parse::<Target>(),
            2 => format!("{input}.0").parse::<Target>(),
            _ => input.parse::<Target>(),
        }
        .map_err(|error| match error {
            ParseTargetError::ComponentCount =>
                "invalid version: expected major, major.minor, or major.minor.patch; example: node@18.1".to_owned(),
            _ => error.to_string(),
        })?;
        Ok(Self { target, components })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseTargetError {
    InvalidStructure,
    UnsupportedTool,
    ComponentCount,
    NonAsciiDigits,
    LeadingZeros,
    OutOfRange,
}

impl fmt::Display for ParseTargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidStructure => "invalid target: expected <tool>@<version> with exactly one @ and non-empty parts; example: node@22.0.0",
            Self::UnsupportedTool => "unsupported tool: only node is supported",
            Self::ComponentCount => "invalid version: expected major.minor.patch; example: node@22.0.0",
            Self::NonAsciiDigits => "invalid version: components must contain only ASCII digits; example: node@22.0.0",
            Self::LeadingZeros => "invalid version: leading zeros are not allowed; example: node@22.0.0",
            Self::OutOfRange => "invalid version: components must be within 0..=4294967295; example: node@22.0.0",
        })
    }
}

impl std::error::Error for ParseTargetError {}

impl FromStr for Target {
    type Err = ParseTargetError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (tool, version) = input
            .split_once('@')
            .ok_or(ParseTargetError::InvalidStructure)?;
        if tool.is_empty() || version.is_empty() || version.contains('@') {
            return Err(ParseTargetError::InvalidStructure);
        }
        if tool != "node" {
            return Err(ParseTargetError::UnsupportedTool);
        }

        let mut parts = version.split('.');
        let components = match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(major), Some(minor), Some(patch), None) => [major, minor, patch],
            _ => return Err(ParseTargetError::ComponentCount),
        };
        if components
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return Err(ParseTargetError::NonAsciiDigits);
        }
        if components
            .iter()
            .any(|part| part.len() > 1 && part.starts_with('0'))
        {
            return Err(ParseTargetError::LeadingZeros);
        }

        let [major, minor, patch] = components.map(str::parse::<u32>);
        Ok(Self {
            tool: Tool::Node,
            version: Version {
                major: major.map_err(|_| ParseTargetError::OutOfRange)?,
                minor: minor.map_err(|_| ParseTargetError::OutOfRange)?,
                patch: patch.map_err(|_| ParseTargetError::OutOfRange)?,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ParseTargetError, Target, Tool, Version, VersionRequest};

    #[test]
    fn version_requests_accept_canonical_full_and_partial_versions() {
        for input in [
            "node@18",
            "node@18.1",
            "node@18.1.2",
            "node@0",
            "node@0.0",
            "node@0.0.0",
            "node@4294967295",
            "node@4294967295.4294967295",
            "node@4294967295.4294967295.4294967295",
        ] {
            let request = input.parse::<VersionRequest>().unwrap();
            assert_eq!(request.to_string(), input);
        }
    }

    #[test]
    fn version_requests_match_supplied_components_exactly() {
        let target = "node@18.1.10".parse::<Target>().unwrap();
        for (input, expected) in [
            ("node@18", true),
            ("node@18.1", true),
            ("node@18.1.10", true),
            ("node@1", false),
            ("node@18.10", false),
            ("node@18.1.1", false),
            ("node@19", false),
        ] {
            let request = input.parse::<VersionRequest>().unwrap();
            assert_eq!(request.matches(&target), expected, "{input}");
        }
    }

    #[test]
    fn partial_version_requests_preserve_canonical_component_rules() {
        for input in [
            "",
            "node",
            "@18",
            "node@",
            "node@@18",
            "node@18@",
            "Node@18",
            "nodejs@18.1",
            "node@latest",
            "node@lts",
            "node@*",
            "node@v18",
            "node@18.",
            "node@.1",
            "node@18..1",
            "node@18.1.2.3",
            "node@018",
            "node@18.01",
            "node@4294967296",
            "node@18.4294967296",
            "node@１８",
            "node@18.１",
            "node@ 18",
            "node@18.1 ",
            "node@-18",
            "node@+18",
            "node@18-beta",
            "node@18.1+build",
            "node@18/1",
            "node@18\\1",
            "node@../18",
            "node@18\0",
        ] {
            assert!(input.parse::<VersionRequest>().is_err(), "{input:?}");
        }
    }

    #[test]
    fn full_versions_return_numeric_data_and_canonical_names() {
        for (input, components) in [
            ("node@22.0.0", [22, 0, 0]),
            ("node@1.23.456", [1, 23, 456]),
            ("node@0.0.0", [0, 0, 0]),
            ("node@4294967295.4294967295.4294967295", [u32::MAX; 3]),
        ] {
            let target = input.parse::<Target>().unwrap();
            let [major, minor, patch] = components;
            assert_eq!(target.tool, Tool::Node);
            assert_eq!(
                target.version,
                Version {
                    major,
                    minor,
                    patch
                }
            );
            assert_eq!(target.to_string(), input);
        }
    }

    #[test]
    fn malformed_targets_report_the_first_failed_rule() {
        use ParseTargetError::*;

        let cases: &[(ParseTargetError, &[&str])] = &[
            (
                InvalidStructure,
                &[
                    "",
                    "node",
                    "node22.0.0",
                    "@",
                    "@22.0.0",
                    "node@",
                    "Node@",
                    "node@@22.0.0",
                    "node@22.0.0@",
                    "Node@22@0",
                ],
            ),
            (
                UnsupportedTool,
                &[
                    "Node@22.0.0",
                    "nodejs@22.0.0",
                    "python@3.12.0",
                    "Node@22",
                    " node@22.0.0",
                    "node @22.0.0",
                    "../node@22.0.0",
                    "..\\node@22.0.0",
                ],
            ),
            (
                ComponentCount,
                &[
                    "node@22",
                    "node@22.1",
                    "node@1.2.3.4",
                    "node@latest",
                    "node@lts",
                    "node@*",
                    "node@../22.0.0",
                    "node@1.2.3/..",
                    "node@1.2.3\\..",
                ],
            ),
            (
                NonAsciiDigits,
                &[
                    "node@.1.2",
                    "node@1..2",
                    "node@1.2.",
                    "node@v22.0.0",
                    "node@1.2.3-beta",
                    "node@1.2.3+build",
                    "node@1.2.*",
                    "node@^1.2.3",
                    "node@~1.2.3",
                    "node@-1.2.3",
                    "node@+1.2.3",
                    "node@１.2.3",
                    "node@1.٢.3",
                    "node@ 1.2.3",
                    "node@1.2.3 ",
                    "node@1.\t2.3",
                    "node@1.2.3\n",
                    "node@1.2.3\0",
                    "node@1/2.3.4",
                    "node@1\\2.3.4",
                    "node@01.x.0",
                    "node@4294967296.01.x",
                ],
            ),
            (
                LeadingZeros,
                &[
                    "node@01.2.3",
                    "node@1.02.3",
                    "node@1.2.03",
                    "node@00.0.0",
                    "node@4294967296.01.0",
                ],
            ),
            (
                OutOfRange,
                &[
                    "node@4294967296.0.0",
                    "node@0.4294967296.0",
                    "node@0.0.4294967296",
                    "node@999999999999999999999999999999999999.0.0",
                ],
            ),
        ];
        for (expected, inputs) in cases {
            for input in *inputs {
                assert_eq!(input.parse::<Target>(), Err(*expected), "{input:?}");
            }
        }
    }

    #[test]
    fn diagnostics_match_the_specification() {
        use ParseTargetError::*;

        for (error, message) in [
            (
                InvalidStructure,
                "invalid target: expected <tool>@<version> with exactly one @ and non-empty parts; example: node@22.0.0",
            ),
            (UnsupportedTool, "unsupported tool: only node is supported"),
            (
                ComponentCount,
                "invalid version: expected major.minor.patch; example: node@22.0.0",
            ),
            (
                NonAsciiDigits,
                "invalid version: components must contain only ASCII digits; example: node@22.0.0",
            ),
            (
                LeadingZeros,
                "invalid version: leading zeros are not allowed; example: node@22.0.0",
            ),
            (
                OutOfRange,
                "invalid version: components must be within 0..=4294967295; example: node@22.0.0",
            ),
        ] {
            assert_eq!(error.to_string(), message);
        }
    }
}
