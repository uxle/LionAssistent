//! The Code Generator itself (Development_plan Phase 29). Produces a
//! real, three-file plugin crate -- `Cargo.toml`, `manifest.toml`,
//! `src/lib.rs` -- from [`crate::templates`], implementing the real
//! `plugin_manager::Plugin` trait rather than the narrower `dlopen`
//! metadata-only mechanism (see `templates/lib.rs.template`'s own
//! header comment for why that distinction matters).
//!
//! "Generates a plugin" is proven, not just asserted:
//! `tests/generated_plugin_compiles.rs` runs the real, generated
//! `cargo build` (and `cargo test`) against actual output on disk, the
//! same standard `coding-agent`'s own scratch-crate tests already set
//! for "a generator/analyzer that touches real Cargo output must prove
//! it against a real `cargo` invocation, not just inspect the text it
//! produced."

use crate::templates::{
    render_template, CARGO_TOML_TEMPLATE, LIB_RS_TEMPLATE, MANIFEST_TOML_TEMPLATE,
};
use common::{AppError, AppResult};
use std::path::Path;
use tool_framework::PermissionLevel;

/// Everything the generator needs to produce one plugin crate.
///
/// Every field is `pub` and freely mutable after construction --
/// [`PluginSpec::new`] fills in reasonable, working defaults for
/// everything but the three fields a caller must actually choose
/// (`crate_name`, `description`, `author`), and the rest exist to be
/// overridden, not hidden behind private setters.
#[derive(Debug, Clone)]
pub struct PluginSpec {
    /// A valid Cargo package name (validated by [`PluginSpec::new`]),
    /// also used as the generated directory name.
    pub crate_name: String,
    /// The Rust struct implementing `Plugin`, derived from
    /// `crate_name` by default (`weather-plugin` -> `WeatherPlugin`).
    pub struct_name: String,
    /// The Rust struct implementing the example `Tool`, derived from
    /// `tool_name` by default.
    pub tool_struct_name: String,
    /// Plugins.md SS5's `id` field. Reverse-DNS style (`com.example.x`)
    /// is the spec's own convention but not one `PluginManifest`
    /// actually enforces at parse time -- `plugin_sdk::lint` checks
    /// for it as a soft warning, not this generator as a hard error,
    /// so this defaults to something workable (`local.<crate_name>`)
    /// rather than refusing to generate anything until a caller
    /// supplies a "properly formatted" one.
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub author: String,
    pub license: String,
    pub tool_name: String,
    pub tool_description: String,
    pub permission_level: PermissionLevel,
}

impl PluginSpec {
    /// Validates `crate_name` (the one field the generated code cannot
    /// tolerate being wrong -- an invalid Cargo package name fails
    /// before a single file is even written) and fills in working
    /// defaults for everything else, derived from `crate_name` where
    /// that makes sense.
    pub fn new(crate_name: &str, description: &str, author: &str) -> AppResult<Self> {
        validate_crate_name(crate_name)?;
        let struct_name = to_pascal_case(crate_name);
        let tool_name = format!("{}_action", crate_name.replace('-', "_"));
        let tool_struct_name = format!("{}Tool", to_pascal_case(&tool_name));

        Ok(Self {
            display_name: to_title_case(crate_name),
            id: format!("local.{}", crate_name.replace('-', "_")),
            description: description.to_string(),
            author: author.to_string(),
            license: "MIT".to_string(),
            tool_description: format!("An example tool contributed by {crate_name}. Replace this."),
            permission_level: PermissionLevel::ReadOnly,
            crate_name: crate_name.to_string(),
            struct_name,
            tool_struct_name,
            tool_name,
        })
    }
}

fn validate_crate_name(name: &str) -> AppResult<()> {
    let valid = !name.is_empty()
        && name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if valid {
        Ok(())
    } else {
        Err(AppError::Module {
            module: "plugin-sdk",
            message: format!(
                "{name:?} is not a usable crate name: must be non-empty, start with a \
                 lowercase letter, and contain only lowercase letters, digits, '-', or '_'"
            ),
        })
    }
}

fn to_pascal_case(s: &str) -> String {
    s.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn to_title_case(s: &str) -> String {
    s.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn permission_level_variant_name(level: PermissionLevel) -> &'static str {
    match level {
        PermissionLevel::ReadOnly => "ReadOnly",
        PermissionLevel::Safe => "Safe",
        PermissionLevel::Moderate => "Moderate",
        PermissionLevel::Destructive => "Destructive",
        PermissionLevel::Administrator => "Administrator",
    }
}

/// Generates a complete plugin crate at `target_dir` (created here;
/// this function refuses if it already exists rather than silently
/// overwriting whatever's there -- a generator surprising a caller by
/// clobbering an existing directory would be exactly the kind of
/// silent behavior this workspace avoids elsewhere, PRD SS12's
/// "no silent dangerous actions" applied in spirit even though this
/// isn't a destructive-tool-permission-level action).
///
/// Writes `Cargo.toml`, `manifest.toml`, and `src/lib.rs` --
/// [`crate::templates`] rendered against `spec`. Does not build or
/// test the output; that's `ToolRouter`/a caller's job, proven for
/// real in `tests/generated_plugin_compiles.rs` rather than assumed.
pub fn generate_plugin(spec: &PluginSpec, target_dir: &Path) -> AppResult<()> {
    if target_dir.exists() {
        return Err(AppError::Module {
            module: "plugin-sdk",
            message: format!(
                "{} already exists; refusing to overwrite it",
                target_dir.display()
            ),
        });
    }

    let substitutions: Vec<(&str, &str)> = vec![
        ("crate_name", &spec.crate_name),
        ("struct_name", &spec.struct_name),
        ("tool_struct_name", &spec.tool_struct_name),
        ("id", &spec.id),
        ("display_name", &spec.display_name),
        ("description", &spec.description),
        ("author", &spec.author),
        ("license", &spec.license),
        ("tool_name", &spec.tool_name),
        ("tool_description", &spec.tool_description),
        (
            "permission_level",
            permission_level_variant_name(spec.permission_level),
        ),
    ];

    let cargo_toml = render(CARGO_TOML_TEMPLATE, &substitutions);
    let manifest_toml = render(MANIFEST_TOML_TEMPLATE, &substitutions);
    let lib_rs = render(LIB_RS_TEMPLATE, &substitutions);

    std::fs::create_dir_all(target_dir.join("src"))?;
    std::fs::write(target_dir.join("Cargo.toml"), cargo_toml)?;
    std::fs::write(target_dir.join("manifest.toml"), manifest_toml)?;
    std::fs::write(target_dir.join("src/lib.rs"), lib_rs)?;

    Ok(())
}

/// Renders `template` and asserts every placeholder was actually
/// substituted -- a leftover `{{...}}` in generated output is a real,
/// silent bug (a template referencing a substitution key this module
/// never provides), and this is the one place positioned to catch it
/// before it reaches disk. `debug_assert` rather than a returned
/// error: this indicates a bug in this crate's own templates/spec
/// wiring, not a caller-supplied input problem, so it belongs in the
/// same category as other internal-invariant checks in this codebase.
fn render(template: &str, substitutions: &[(&str, &str)]) -> String {
    let rendered = render_template(template, substitutions);
    debug_assert!(
        !rendered.contains("{{"),
        "template still contains an unsubstituted placeholder: {rendered}"
    );
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_derives_sensible_names_from_crate_name() {
        let spec = PluginSpec::new("weather-plugin", "Adds weather lookups", "Ada").unwrap();
        assert_eq!(spec.struct_name, "WeatherPlugin");
        assert_eq!(spec.display_name, "Weather Plugin");
        assert_eq!(spec.id, "local.weather_plugin");
        assert_eq!(spec.tool_name, "weather_plugin_action");
        assert_eq!(spec.tool_struct_name, "WeatherPluginActionTool");
    }

    #[test]
    fn rejects_an_empty_crate_name() {
        assert!(PluginSpec::new("", "x", "x").is_err());
    }

    #[test]
    fn rejects_a_crate_name_starting_with_a_digit() {
        assert!(PluginSpec::new("1plugin", "x", "x").is_err());
    }

    #[test]
    fn rejects_uppercase_or_invalid_characters() {
        assert!(PluginSpec::new("Weather-Plugin", "x", "x").is_err());
        assert!(PluginSpec::new("weather plugin", "x", "x").is_err());
        assert!(PluginSpec::new("weather.plugin", "x", "x").is_err());
    }

    #[test]
    fn accepts_underscores_and_hyphens() {
        assert!(PluginSpec::new("weather_plugin", "x", "x").is_ok());
        assert!(PluginSpec::new("weather-plugin-2", "x", "x").is_ok());
    }

    #[test]
    fn generate_plugin_writes_all_three_real_files() {
        let dir = std::env::temp_dir().join(format!("plugin-sdk-gen-test-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let spec = PluginSpec::new("test-plugin", "A test", "Test Author").unwrap();

        generate_plugin(&spec, &dir).unwrap();

        assert!(dir.join("Cargo.toml").is_file());
        assert!(dir.join("manifest.toml").is_file());
        assert!(dir.join("src/lib.rs").is_file());

        let cargo_toml = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(cargo_toml.contains("name = \"test-plugin\""));
        assert!(
            !cargo_toml.contains("{{"),
            "unsubstituted placeholder in Cargo.toml"
        );

        let lib_rs = std::fs::read_to_string(dir.join("src/lib.rs")).unwrap();
        assert!(lib_rs.contains("struct TestPlugin"));
        assert!(
            !lib_rs.contains("{{"),
            "unsubstituted placeholder in lib.rs"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn refuses_to_overwrite_an_existing_directory() {
        let dir =
            std::env::temp_dir().join(format!("plugin-sdk-gen-exists-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let spec = PluginSpec::new("test-plugin", "A test", "Test Author").unwrap();

        let result = generate_plugin(&spec, &dir);
        assert!(result.is_err());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_custom_permission_level_is_reflected_in_generated_source() {
        let dir = std::env::temp_dir().join(format!("plugin-sdk-gen-perm-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let mut spec = PluginSpec::new("test-plugin", "A test", "Test Author").unwrap();
        spec.permission_level = PermissionLevel::Moderate;

        generate_plugin(&spec, &dir).unwrap();
        let lib_rs = std::fs::read_to_string(dir.join("src/lib.rs")).unwrap();
        assert!(lib_rs.contains("PermissionLevel::Moderate"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn to_pascal_case_handles_hyphens_and_underscores() {
        assert_eq!(to_pascal_case("weather-plugin"), "WeatherPlugin");
        assert_eq!(to_pascal_case("weather_plugin"), "WeatherPlugin");
        assert_eq!(to_pascal_case("a"), "A");
    }
}
