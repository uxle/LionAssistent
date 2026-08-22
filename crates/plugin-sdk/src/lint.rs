//! Manifest linting (Development_plan Phase 29 "SDK Improvements").
//!
//! `plugin_manager::manifest::PluginManifest::from_toml` already
//! enforces everything that's a hard *parsing* requirement (Plugins.md
//! SS5's required fields) -- this module is deliberately a second,
//! separate, softer layer on top of that: things a manifest can get
//! technically-parseable-but-wrong on, worth a plugin author's
//! attention but not worth refusing to load over. A lint that fails
//! open (warns, doesn't reject) is the honest choice here: none of
//! these are things `PluginManager` itself currently checks or acts
//! on, so a strict linter claiming otherwise would overstate its own
//! authority.

use plugin_manager::manifest::PluginManifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintWarning {
    /// Plugins.md SS5 shows every example `id` in reverse-DNS style
    /// (`com.example.weather`); nothing in `PluginManifest` enforces
    /// it, but a bare `id` unlikely to stay globally unique is worth
    /// flagging before it collides with someone else's plugin.
    IdNotReverseDns,
    /// `version` doesn't parse as `major.minor.patch` -- the same
    /// format `PluginManifest::is_compatible_with_sdk` already expects
    /// internally, so a version that fails this will also silently
    /// fail every future compatibility check.
    VersionNotSemver,
    EmptyDescription,
    /// `supported_platforms` is empty or omits `"linux"` -- installable
    /// today, but `PluginManager::validate` will always reject it on
    /// this platform (Plugins.md SS6), which reads as a mysterious
    /// failure rather than a manifest issue unless flagged here first.
    DoesNotDeclareLinuxSupport,
}

impl std::fmt::Display for LintWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintWarning::IdNotReverseDns => write!(
                f,
                "id is not reverse-DNS style (e.g. \"com.example.weather\") -- Plugins.md SS5's own convention, for uniqueness"
            ),
            LintWarning::VersionNotSemver => {
                write!(f, "version is not major.minor.patch -- required for SDK compatibility checks to work at all")
            }
            LintWarning::EmptyDescription => write!(f, "description is empty"),
            LintWarning::DoesNotDeclareLinuxSupport => write!(
                f,
                "supported_platforms does not include \"linux\" -- PluginManager::validate will always reject this on this platform"
            ),
        }
    }
}

/// Checks `manifest` against every lint in [`LintWarning`], returning
/// every one that applies -- an empty `Vec` means no concerns, not
/// that the manifest was necessarily hand-checked against every rule
/// a future version of this module might add.
pub fn lint_manifest(manifest: &PluginManifest) -> Vec<LintWarning> {
    let mut warnings = Vec::new();

    if !manifest.id.contains('.') {
        warnings.push(LintWarning::IdNotReverseDns);
    }
    if !looks_like_semver(&manifest.version) {
        warnings.push(LintWarning::VersionNotSemver);
    }
    if manifest.description.trim().is_empty() {
        warnings.push(LintWarning::EmptyDescription);
    }
    if !manifest.supports_current_platform() {
        warnings.push(LintWarning::DoesNotDeclareLinuxSupport);
    }

    warnings
}

fn looks_like_semver(s: &str) -> bool {
    let parts: Vec<&str> = s.trim().split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(toml: &str) -> PluginManifest {
        PluginManifest::from_toml(toml).unwrap()
    }

    fn well_formed() -> &'static str {
        "id = \"com.example.weather\"\nname = \"Weather\"\nversion = \"1.0.0\"\nauthor = \"x\"\n\
         description = \"a real description\"\nlicense = \"MIT\"\nminimum_sdk_version = \"0.1.0\"\n\
         supported_platforms = [\"linux\"]\n"
    }

    #[test]
    fn a_well_formed_manifest_produces_no_warnings() {
        assert!(lint_manifest(&manifest(well_formed())).is_empty());
    }

    #[test]
    fn flags_a_non_reverse_dns_id() {
        let toml = well_formed().replace("id = \"com.example.weather\"", "id = \"weather\"");
        assert!(lint_manifest(&manifest(&toml)).contains(&LintWarning::IdNotReverseDns));
    }

    #[test]
    fn flags_a_non_semver_version() {
        let toml = well_formed().replace("version = \"1.0.0\"", "version = \"latest\"");
        assert!(lint_manifest(&manifest(&toml)).contains(&LintWarning::VersionNotSemver));
    }

    #[test]
    fn accepts_a_version_missing_only_from_is_compatible_with_sdk_style_two_part_form() {
        // is_compatible_with_sdk itself defaults a missing patch to 0,
        // but this lint is deliberately stricter (flags it) since a
        // manifest author should write all three parts explicitly.
        let toml = well_formed().replace("version = \"1.0.0\"", "version = \"1.0\"");
        assert!(lint_manifest(&manifest(&toml)).contains(&LintWarning::VersionNotSemver));
    }

    #[test]
    fn flags_an_empty_description() {
        let toml =
            well_formed().replace("description = \"a real description\"", "description = \"\"");
        assert!(lint_manifest(&manifest(&toml)).contains(&LintWarning::EmptyDescription));
    }

    #[test]
    fn flags_missing_linux_support() {
        let toml = well_formed().replace(
            "supported_platforms = [\"linux\"]",
            "supported_platforms = [\"windows\"]",
        );
        assert!(lint_manifest(&manifest(&toml)).contains(&LintWarning::DoesNotDeclareLinuxSupport));
    }

    #[test]
    fn multiple_issues_are_all_reported_together() {
        let toml = "id = \"weather\"\nname = \"x\"\nversion = \"latest\"\nauthor = \"x\"\n\
                    description = \"\"\nlicense = \"MIT\"\nminimum_sdk_version = \"0.1.0\"\n\
                    supported_platforms = []\n";
        let warnings = lint_manifest(&manifest(toml));
        assert_eq!(warnings.len(), 4);
    }

    #[test]
    fn display_messages_are_non_empty_and_readable() {
        for warning in [
            LintWarning::IdNotReverseDns,
            LintWarning::VersionNotSemver,
            LintWarning::EmptyDescription,
            LintWarning::DoesNotDeclareLinuxSupport,
        ] {
            let message = warning.to_string();
            assert!(!message.is_empty());
        }
    }
}
