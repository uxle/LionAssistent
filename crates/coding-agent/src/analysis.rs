//! Project Analysis (Development_plan Phase 24). Real directory
//! traversal plus real parsing of whichever manifest file it finds --
//! no heuristics about file *content* beyond that, no code execution
//! anywhere in this module (contrast [`crate::diagnostics`], which does
//! run code and is scoped and permissioned accordingly).

use common::{AppError, AppResult};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Directories whose contents are near-certainly build output or
/// third-party code, not the project itself -- walked past, not into,
/// the same "skip generated/vendored content" judgment call
/// `search::file_search` already makes for its own indexing.
const SKIP_DIRS: &[&str] = &[
    "target",
    "node_modules",
    ".git",
    ".venv",
    "venv",
    "__pycache__",
    "dist",
    "build",
    ".cargo",
];

/// A hard ceiling on how many files a single `analyze_project` call will
/// walk (TCLIS SS7 "Resource limits" -- validation rules apply to how
/// much work a tool can be made to do, not just to its inputs' shape).
/// High enough that no real project in this workspace gets anywhere
/// near it (the whole `ai-assistant` source tree, `target/` excluded, is
/// a few hundred files), low enough that pointing this at an
/// unexpectedly huge directory fails predictably instead of running for
/// an unbounded amount of time. The default `analyze_project` uses --
/// `config::CodingAgentConfig::max_files_walked` is this same value,
/// overridable per the usual "settings surface a caller wires through
/// explicitly" relationship `config`'s own doc comments describe.
pub const DEFAULT_MAX_FILES_WALKED: usize = 20_000;

#[derive(Debug, Clone, PartialEq)]
pub struct ManifestInfo {
    /// `"cargo"`, `"npm"`, or `"python"` -- an open string rather than a
    /// closed enum, the same reasoning `diagnostics::Diagnostic::level`
    /// documents: better to under-promise on exhaustiveness than to
    /// silently drop a manifest kind a future caller cares about.
    pub kind: String,
    pub name: Option<String>,
    pub dependency_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectInfo {
    pub root: PathBuf,
    pub file_count: usize,
    /// Extension (without the dot, lowercased) -> count. Files with no
    /// extension are bucketed under `""`.
    pub language_breakdown: BTreeMap<String, usize>,
    pub manifests_found: Vec<String>,
    pub primary_manifest: Option<ManifestInfo>,
    /// True if `MAX_FILES_WALKED` was hit -- `file_count` and
    /// `language_breakdown` are a real prefix of the project, not a lie,
    /// but not the *whole* project either. Said explicitly rather than
    /// silently returned as if it were complete.
    pub truncated: bool,
}

fn module_err(context: impl std::fmt::Display) -> AppError {
    AppError::Module {
        module: "coding-agent",
        message: context.to_string(),
    }
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default()
}

fn walk(
    dir: &Path,
    max_files: usize,
    file_count: &mut usize,
    breakdown: &mut BTreeMap<String, usize>,
    manifests: &mut Vec<String>,
) -> AppResult<bool> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| module_err(format!("reading directory {}: {e}", dir.display())))?;

    for entry in entries {
        if *file_count >= max_files {
            return Ok(true); // truncated
        }
        let entry = entry.map_err(|e| module_err(format!("walking {}: {e}", dir.display())))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|e| module_err(format!("reading file type for {}: {e}", path.display())))?;

        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if SKIP_DIRS.contains(&name.as_ref()) {
                continue;
            }
            if walk(&path, max_files, file_count, breakdown, manifests)? {
                return Ok(true);
            }
        } else if file_type.is_file() {
            *file_count += 1;
            *breakdown.entry(extension_of(&path)).or_insert(0) += 1;

            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if matches!(name, "Cargo.toml" | "package.json" | "pyproject.toml") {
                    manifests.push(
                        path.strip_prefix(dir)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .to_string(),
                    );
                }
            }
        }
    }
    Ok(false)
}

/// Cargo.toml has one canonical dependency table shape -- straightforward
/// to parse exactly, unlike Python's more varied packaging landscape
/// (see `parse_pyproject_manifest`).
fn parse_cargo_manifest(path: &Path) -> AppResult<ManifestInfo> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| module_err(format!("reading {}: {e}", path.display())))?;
    let value: toml::Value =
        toml::from_str(&raw).map_err(|e| module_err(format!("parsing {}: {e}", path.display())))?;

    let name = value
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_string);
    let dependency_count = value
        .get("dependencies")
        .and_then(|d| d.as_table())
        .map_or(0, |t| t.len());

    Ok(ManifestInfo {
        kind: "cargo".to_string(),
        name,
        dependency_count,
    })
}

fn parse_npm_manifest(path: &Path) -> AppResult<ManifestInfo> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| module_err(format!("reading {}: {e}", path.display())))?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| module_err(format!("parsing {}: {e}", path.display())))?;

    let name = value
        .get("name")
        .and_then(|n| n.as_str())
        .map(str::to_string);
    let dependency_count = ["dependencies", "devDependencies"]
        .iter()
        .filter_map(|key| value.get(key).and_then(|d| d.as_object()).map(|o| o.len()))
        .sum();

    Ok(ManifestInfo {
        kind: "npm".to_string(),
        name,
        dependency_count,
    })
}

/// Python packaging doesn't have one canonical dependency table the way
/// Cargo.toml/package.json do -- PEP 621 (`[project.dependencies]`, an
/// array) and Poetry (`[tool.poetry.dependencies]`, a table) are both
/// real, both common, and structurally different from each other. This
/// checks both and reports whichever it finds rather than picking one
/// and silently missing projects using the other -- a best-effort
/// heuristic, said as one, not asserted as exact the way the Cargo/npm
/// paths are.
fn parse_pyproject_manifest(path: &Path) -> AppResult<ManifestInfo> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| module_err(format!("reading {}: {e}", path.display())))?;
    let value: toml::Value =
        toml::from_str(&raw).map_err(|e| module_err(format!("parsing {}: {e}", path.display())))?;

    let name = value
        .get("project")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .or_else(|| {
            value
                .get("tool")
                .and_then(|t| t.get("poetry"))
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
        })
        .map(str::to_string);

    let pep621_count = value
        .get("project")
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_array())
        .map(|a| a.len());
    let poetry_count = value
        .get("tool")
        .and_then(|t| t.get("poetry"))
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_table())
        .map(|t| t.len());

    Ok(ManifestInfo {
        kind: "python".to_string(),
        name,
        dependency_count: pep621_count.or(poetry_count).unwrap_or(0),
    })
}

/// Walks `root`, classifies files by extension, and parses whichever
/// manifest it finds first (Cargo.toml, then package.json, then
/// pyproject.toml -- an arbitrary but documented tie-break for the rare
/// case a directory has more than one). Uses [`DEFAULT_MAX_FILES_WALKED`]
/// as the walk limit -- see [`analyze_project_with_limit`] to override it.
pub fn analyze_project(root: &Path) -> AppResult<ProjectInfo> {
    analyze_project_with_limit(root, DEFAULT_MAX_FILES_WALKED)
}

/// Same as [`analyze_project`], with an explicit file-walk ceiling
/// rather than [`DEFAULT_MAX_FILES_WALKED`].
pub fn analyze_project_with_limit(root: &Path, max_files: usize) -> AppResult<ProjectInfo> {
    if !root.is_dir() {
        return Err(module_err(format!("{} is not a directory", root.display())));
    }

    let mut file_count = 0;
    let mut language_breakdown = BTreeMap::new();
    let mut manifests_found = Vec::new();
    let truncated = walk(
        root,
        max_files,
        &mut file_count,
        &mut language_breakdown,
        &mut manifests_found,
    )?;

    let primary_manifest = ["Cargo.toml", "package.json", "pyproject.toml"]
        .iter()
        .find(|name| manifests_found.iter().any(|m| m == *name))
        .and_then(|name| {
            let path = root.join(name);
            let result = match *name {
                "Cargo.toml" => parse_cargo_manifest(&path),
                "package.json" => parse_npm_manifest(&path),
                "pyproject.toml" => parse_pyproject_manifest(&path),
                _ => unreachable!(),
            };
            result.ok()
        });

    Ok(ProjectInfo {
        root: root.to_path_buf(),
        file_count,
        language_breakdown,
        manifests_found,
        primary_manifest,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyzes_this_very_workspaces_cache_crate_for_real() {
        // No fixture needed -- this crate's own sibling `cache` crate
        // (Phase 18) is a real, known, already-committed Rust crate with
        // a real Cargo.toml right here in this checkout.
        let cache_crate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cache");
        let info = analyze_project(&cache_crate).unwrap();

        assert!(info.file_count > 0);
        assert!(
            info.language_breakdown.get("rs").copied().unwrap_or(0) > 0,
            "expected at least one .rs file, got {:?}",
            info.language_breakdown
        );
        assert!(info.manifests_found.iter().any(|m| m == "Cargo.toml"));
        let manifest = info
            .primary_manifest
            .expect("Cargo.toml should have parsed");
        assert_eq!(manifest.kind, "cargo");
        assert_eq!(manifest.name.as_deref(), Some("cache"));
        assert!(!info.truncated);
    }

    #[test]
    fn nonexistent_directory_fails_cleanly() {
        assert!(analyze_project(Path::new("/nonexistent/not-a-real-dir")).is_err());
    }

    #[test]
    fn skips_junk_directories_like_target_and_node_modules() {
        let dir = std::env::temp_dir().join("coding-agent-test-skip-dirs");
        std::fs::create_dir_all(dir.join("target/deep/nested")).unwrap();
        std::fs::write(dir.join("target/deep/nested/junk.rs"), "").unwrap();
        std::fs::write(dir.join("real.rs"), "fn main() {}").unwrap();

        let info = analyze_project(&dir).unwrap();
        assert_eq!(
            info.file_count, 1,
            "should only count real.rs, not target/**"
        );
        assert_eq!(info.language_breakdown.get("rs").copied(), Some(1));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_custom_file_limit_is_actually_honored_and_reported_as_truncated() {
        let dir = std::env::temp_dir().join("coding-agent-test-file-limit");
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..10 {
            std::fs::write(dir.join(format!("file{i}.txt")), "").unwrap();
        }

        let info = analyze_project_with_limit(&dir, 3).unwrap();
        assert!(info.truncated);
        assert_eq!(
            info.file_count, 3,
            "should stop exactly at the configured limit"
        );

        let untruncated = analyze_project_with_limit(&dir, 100).unwrap();
        assert!(!untruncated.truncated);
        assert_eq!(untruncated.file_count, 10);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parses_a_real_package_json() {
        let dir = std::env::temp_dir().join("coding-agent-test-npm");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"name": "example-app", "dependencies": {"react": "^18.0.0"}, "devDependencies": {"vite": "^5.0.0", "eslint": "^9.0.0"}}"#,
        )
        .unwrap();

        let info = analyze_project(&dir).unwrap();
        let manifest = info.primary_manifest.unwrap();
        assert_eq!(manifest.kind, "npm");
        assert_eq!(manifest.name.as_deref(), Some("example-app"));
        assert_eq!(manifest.dependency_count, 3);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parses_a_real_pep_621_pyproject() {
        let dir = std::env::temp_dir().join("coding-agent-test-pep621");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("pyproject.toml"),
            "[project]\nname = \"example-py\"\ndependencies = [\"requests\", \"click\"]\n",
        )
        .unwrap();

        let info = analyze_project(&dir).unwrap();
        let manifest = info.primary_manifest.unwrap();
        assert_eq!(manifest.kind, "python");
        assert_eq!(manifest.name.as_deref(), Some("example-py"));
        assert_eq!(manifest.dependency_count, 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parses_a_real_poetry_pyproject() {
        let dir = std::env::temp_dir().join("coding-agent-test-poetry");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("pyproject.toml"),
            "[tool.poetry]\nname = \"example-poetry\"\n[tool.poetry.dependencies]\npython = \"^3.11\"\nrequests = \"^2.31\"\n",
        )
        .unwrap();

        let info = analyze_project(&dir).unwrap();
        let manifest = info.primary_manifest.unwrap();
        assert_eq!(manifest.kind, "python");
        assert_eq!(manifest.name.as_deref(), Some("example-poetry"));
        assert_eq!(manifest.dependency_count, 2);

        std::fs::remove_dir_all(&dir).ok();
    }
}
