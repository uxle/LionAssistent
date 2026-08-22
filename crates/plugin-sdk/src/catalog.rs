//! A local Plugin Marketplace (Development_plan Phase 29).
//!
//! "Local" is a deliberate, named scope, not an oversight: there is no
//! real, hosted plugin marketplace this from-scratch project could
//! honestly connect to yet, and PRD SS3's own non-goal ("no cloud
//! dependency for the core assistant") makes local-first the right
//! default even once one might exist. What's here is real and useful
//! today -- indexing, searching, and browsing whatever plugin
//! manifests already exist in a local directory (the same
//! `plugins/installed/` layout `plugin_manager::PluginManager` already
//! scans) -- and is written so a later, real remote source could be
//! added as a second implementation without redesigning this one: see
//! [`CatalogSource`].
//!
//! Deliberately reuses `plugin_manager::PluginManager::discover_manifests`
//! for the actual directory scan rather than re-implementing it --
//! that function already exists, is already tested, and this crate
//! adds a browsing/search layer on top of it, not a competing scanner.

use plugin_manager::manifest::PluginManifest;
use plugin_manager::PluginManager;
use std::path::Path;

/// Where a [`PluginCatalog`]'s entries came from. Only
/// [`CatalogSource::LocalDirectory`] is implemented this phase --
/// [`CatalogSource::Bundled`] exists for the one concrete, curated
/// entry [`PluginCatalog::with_bundled_examples`] adds (this
/// workspace's own real, compiled `example-plugin`, Phase 11) rather
/// than being a placeholder for something unimplemented. A real
/// `CatalogSource::Remote(url)` variant is the natural next step once
/// an actual marketplace exists to query -- deliberately not added
/// speculatively ahead of that, the same reasoning `mcp`'s "no connect
/// tool" and `remote-control`'s "no TLS" documented their own
/// deferrals with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogSource {
    LocalDirectory,
    Bundled,
}

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub manifest: PluginManifest,
    pub source: CatalogSource,
}

/// A searchable index over one or more [`CatalogEntry`]s.
#[derive(Debug, Clone, Default)]
pub struct PluginCatalog {
    entries: Vec<CatalogEntry>,
}

impl PluginCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Scans `dir` for `*/manifest.toml` files (via
    /// `PluginManager::discover_manifests`) and indexes whatever
    /// parses successfully. An empty or missing directory yields an
    /// empty catalog, not an error -- a fresh install with no plugins
    /// installed yet is a normal state, not a failure.
    pub fn from_directory(dir: &Path) -> Self {
        let entries = PluginManager::discover_manifests(dir)
            .into_iter()
            .map(|manifest| CatalogEntry {
                manifest,
                source: CatalogSource::LocalDirectory,
            })
            .collect();
        Self { entries }
    }

    /// Adds this workspace's own real `example-plugin` (Phase 11) as a
    /// bundled, always-available catalog entry -- a genuine, existing
    /// plugin, not a fabricated example, proving the catalog can
    /// represent something real end to end.
    pub fn with_bundled_examples(mut self) -> Self {
        let manifest = PluginManifest::from_toml(
            "id = \"dev.local-assistant.example-plugin\"\n\
             name = \"Example Plugin\"\n\
             version = \"0.1.0\"\n\
             author = \"Local Linux AI Assistant project\"\n\
             description = \"A minimal real .so plugin proving plugin-manager's dynamic loader works (Phase 11).\"\n\
             license = \"MIT\"\n\
             minimum_sdk_version = \"0.1.0\"\n\
             supported_platforms = [\"linux\"]\n\
             tags = [\"example\", \"starter\"]\n",
        )
        .expect("bundled manifest is a fixed, known-good literal");
        self.entries.push(CatalogEntry {
            manifest,
            source: CatalogSource::Bundled,
        });
        self
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn all(&self) -> &[CatalogEntry] {
        &self.entries
    }

    /// Case-insensitive substring match against name, description, and
    /// tags -- deliberately simple (no ranking, no fuzzy matching):
    /// with a handful of locally-installed plugins, a plain substring
    /// scan already answers "do I have something like X installed?"
    /// without needing a real search index.
    pub fn search(&self, query: &str) -> Vec<&CatalogEntry> {
        let query = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.manifest.name.to_lowercase().contains(&query)
                    || e.manifest.description.to_lowercase().contains(&query)
                    || e.manifest
                        .tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query))
            })
            .collect()
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&CatalogEntry> {
        self.entries
            .iter()
            .filter(|e| e.manifest.tags.iter().any(|t| t == tag))
            .collect()
    }

    pub fn find_by_id(&self, id: &str) -> Option<&CatalogEntry> {
        self.entries.iter().find(|e| e.manifest.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_directory_yields_an_empty_catalog_not_an_error() {
        let catalog = PluginCatalog::from_directory(Path::new("/definitely/not/a/real/dir"));
        assert!(catalog.is_empty());
    }

    #[test]
    fn indexes_real_manifest_files_from_a_real_directory() {
        let dir =
            std::env::temp_dir().join(format!("plugin-sdk-catalog-test-{}", std::process::id()));
        let plugin_dir = dir.join("weather-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("manifest.toml"),
            "id = \"com.example.weather\"\nname = \"Weather\"\nversion = \"1.0.0\"\nauthor = \"x\"\n\
             description = \"weather lookups\"\nlicense = \"MIT\"\nminimum_sdk_version = \"0.1.0\"\n\
             supported_platforms = [\"linux\"]\ntags = [\"weather\", \"network\"]\n",
        )
        .unwrap();

        let catalog = PluginCatalog::from_directory(&dir);
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog.all()[0].source, CatalogSource::LocalDirectory);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_matches_name_description_and_tags_case_insensitively() {
        let catalog = PluginCatalog::new().with_bundled_examples();
        assert_eq!(catalog.search("EXAMPLE").len(), 1);
        assert_eq!(catalog.search("dynamic loader").len(), 1);
        assert_eq!(catalog.search("starter").len(), 1);
        assert!(catalog.search("nonexistent-plugin-xyz").is_empty());
    }

    #[test]
    fn by_tag_finds_exact_tag_matches_only() {
        let catalog = PluginCatalog::new().with_bundled_examples();
        assert_eq!(catalog.by_tag("starter").len(), 1);
        assert!(
            catalog.by_tag("start").is_empty(),
            "should not substring-match tags"
        );
    }

    #[test]
    fn find_by_id_locates_the_bundled_example() {
        let catalog = PluginCatalog::new().with_bundled_examples();
        let found = catalog.find_by_id("dev.local-assistant.example-plugin");
        assert!(found.is_some());
        assert_eq!(found.unwrap().source, CatalogSource::Bundled);
    }

    #[test]
    fn find_by_id_returns_none_for_an_unknown_id() {
        let catalog = PluginCatalog::new().with_bundled_examples();
        assert!(catalog.find_by_id("nothing.like.this").is_none());
    }

    #[test]
    fn combining_a_local_directory_and_bundled_examples_indexes_both() {
        let dir = std::env::temp_dir().join(format!(
            "plugin-sdk-catalog-combined-{}",
            std::process::id()
        ));
        let plugin_dir = dir.join("weather-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("manifest.toml"),
            "id = \"com.example.weather\"\nname = \"Weather\"\nversion = \"1.0.0\"\nauthor = \"x\"\n\
             description = \"x\"\nlicense = \"MIT\"\nminimum_sdk_version = \"0.1.0\"\n\
             supported_platforms = [\"linux\"]\n",
        )
        .unwrap();

        let catalog = PluginCatalog::from_directory(&dir).with_bundled_examples();
        assert_eq!(catalog.len(), 2);

        std::fs::remove_dir_all(&dir).ok();
    }
}
