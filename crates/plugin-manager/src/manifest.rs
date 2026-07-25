//! Plugin manifests (Plugins.md SS5 "Plugin Manifest") and version
//! compatibility (SS15).

use common::{AppError, AppResult};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginPermission {
    FileAccess,
    NetworkAccess,
    Notifications,
    Clipboard,
    Audio,
    PackageManagement,
    ProcessInformation,
}

impl PluginPermission {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FileAccess => "file_access",
            Self::NetworkAccess => "network_access",
            Self::Notifications => "notifications",
            Self::Clipboard => "clipboard",
            Self::Audio => "audio",
            Self::PackageManagement => "package_management",
            Self::ProcessInformation => "process_information",
        }
    }
}

impl std::str::FromStr for PluginPermission {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "file_access" => Self::FileAccess,
            "network_access" => Self::NetworkAccess,
            "notifications" => Self::Notifications,
            "clipboard" => Self::Clipboard,
            "audio" => Self::Audio,
            "package_management" => Self::PackageManagement,
            "process_information" => Self::ProcessInformation,
            other => return Err(format!("unknown plugin permission: {other:?}")),
        })
    }
}

/// Plugins.md SS5. `homepage`/`repository` are treated as optional in
/// practice even though the spec lists them alongside the rest --
/// requiring every plugin author to have both before their plugin can
/// even be parsed is stricter than the identity/security fields
/// genuinely need to be.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub license: String,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    pub minimum_sdk_version: String,
    pub supported_platforms: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl PluginManifest {
    pub fn from_toml(text: &str) -> AppResult<Self> {
        toml::from_str(text).map_err(|e| AppError::Module {
            module: "plugin-manager",
            message: format!("invalid manifest: {e}"),
        })
    }

    pub fn load(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|e| AppError::Module {
            module: "plugin-manager",
            message: format!("reading {}: {e}", path.display()),
        })?;
        Self::from_toml(&text)
    }

    /// Parsed, order-preserving permission set; unrecognized strings are
    /// dropped rather than failing manifest parsing entirely -- a
    /// forward-compatible plugin declaring a newer permission this
    /// version of the host doesn't know about shouldn't be unloadable
    /// outright, just unable to use that specific capability.
    pub fn permissions(&self) -> Vec<PluginPermission> {
        self.permissions
            .iter()
            .filter_map(|p| p.parse().ok())
            .collect()
    }

    pub fn supports_current_platform(&self) -> bool {
        self.supported_platforms.iter().any(|p| p == "linux")
    }

    /// Plugins.md SS15: "The Plugin Manager warns about incompatible
    /// versions." Simple major.minor.patch comparison -- no pre-release
    /// or build-metadata handling, which nothing here needs yet.
    pub fn is_compatible_with_sdk(&self, host_sdk_version: &str) -> bool {
        match (
            parse_version(&self.minimum_sdk_version),
            parse_version(host_sdk_version),
        ) {
            (Some(min), Some(host)) => host >= min,
            _ => false, // unparsable version string -- treat as incompatible rather than guessing
        }
    }
}

fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let mut parts = s.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_toml() -> &'static str {
        r#"
            id = "com.example.weather"
            name = "Weather Plugin"
            version = "1.0.0"
            author = "Example Author"
            description = "Adds weather lookup tools"
            license = "MIT"
            minimum_sdk_version = "0.1.0"
            supported_platforms = ["linux"]
            permissions = ["network_access", "notifications"]
        "#
    }

    #[test]
    fn parses_a_well_formed_manifest() {
        let manifest = PluginManifest::from_toml(sample_toml()).unwrap();
        assert_eq!(manifest.id, "com.example.weather");
        assert_eq!(
            manifest.permissions(),
            vec![
                PluginPermission::NetworkAccess,
                PluginPermission::Notifications
            ]
        );
        assert!(manifest.homepage.is_none());
    }

    #[test]
    fn missing_required_field_fails_to_parse() {
        let broken = r#"
            name = "Missing Id"
            version = "1.0.0"
        "#;
        assert!(PluginManifest::from_toml(broken).is_err());
    }

    #[test]
    fn unrecognized_permission_strings_are_dropped_not_fatal() {
        let toml = r#"
            id = "x"
            name = "x"
            version = "1.0.0"
            author = "x"
            description = "x"
            license = "MIT"
            minimum_sdk_version = "0.1.0"
            supported_platforms = ["linux"]
            permissions = ["network_access", "some_future_permission"]
        "#;
        let manifest = PluginManifest::from_toml(toml).unwrap();
        assert_eq!(
            manifest.permissions(),
            vec![PluginPermission::NetworkAccess]
        );
    }

    #[test]
    fn platform_support_check() {
        let manifest = PluginManifest::from_toml(sample_toml()).unwrap();
        assert!(manifest.supports_current_platform());
    }

    #[test]
    fn sdk_version_compatibility() {
        let manifest = PluginManifest::from_toml(sample_toml()).unwrap(); // minimum_sdk_version = 0.1.0
        assert!(manifest.is_compatible_with_sdk("0.1.0"));
        assert!(manifest.is_compatible_with_sdk("0.2.0"));
        assert!(manifest.is_compatible_with_sdk("1.0.0"));
        assert!(!manifest.is_compatible_with_sdk("0.0.9"));
    }

    #[test]
    fn unparsable_host_version_is_treated_as_incompatible() {
        let manifest = PluginManifest::from_toml(sample_toml()).unwrap();
        assert!(!manifest.is_compatible_with_sdk("not-a-version"));
    }
}
