//! `config` -- application configuration loading & persistence
//! (TDS SS6, Apis SS11, DBDMS "Settings" table).

use common::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelConfig {
    pub active_model: String,
    pub model_path: String,
    pub context_length: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub max_tokens: u32,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            active_model: "gemma-3-1b".to_string(),
            model_path: "models/gemma/".to_string(),
            context_length: 8192,
            temperature: 0.7,
            top_p: 0.9,
            max_tokens: 1024,
        }
    }
}

/// UI-UX SS13 "adjustable text scaling" -- bounded so a bad or
/// hand-edited config value can't render the app unreadably tiny or
/// unusably huge. `0.8`..`2.0` covers "a bit smaller" through "double
/// size" without either extreme.
pub const MIN_TEXT_SCALE: f32 = 0.8;
pub const MAX_TEXT_SCALE: f32 = 2.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub theme: String,
    pub language: String,
    /// UI-UX SS15 "reduced motion option": when true, `ui`'s Slint side
    /// (Phase 17) treats every `animate` duration as `0ms`.
    pub reduced_motion: bool,
    /// UI-UX SS13 "high contrast support".
    pub high_contrast: bool,
    /// Not necessarily already in the safe `0.8..=2.0` range -- e.g. a
    /// config file hand-edited, or written by a future version of this
    /// app with a different valid range. See
    /// [`UiConfig::effective_text_scale`] for what actually gets applied.
    pub text_scale: f32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            language: "en".to_string(),
            reduced_motion: false,
            high_contrast: false,
            text_scale: 1.0,
        }
    }
}

impl UiConfig {
    /// `text_scale`, clamped to `MIN_TEXT_SCALE..=MAX_TEXT_SCALE` (or the
    /// default, `1.0`, if it's not even a real number). Doesn't mutate
    /// `text_scale` itself -- only what's actually applied is bounded,
    /// so an out-of-range value already on disk isn't silently rewritten
    /// out from under whatever wrote it there.
    pub fn effective_text_scale(&self) -> f32 {
        if self.text_scale.is_nan() {
            return UiConfig::default().text_scale;
        }
        self.text_scale.clamp(MIN_TEXT_SCALE, MAX_TEXT_SCALE)
    }
}

/// Development_plan Phase 18 "Caching"/"Memory Optimization": tunable
/// cache sizes so a memory-constrained machine -- or a power user who
/// just wants tighter or looser caching -- doesn't need to recompile
/// anything to change them (PRD 6.7 settings panel; Archd SS11
/// configuration stores "memory" explicitly). `embeddings::CachingEmbedder`
/// and `memory::MemoryStore`'s search-result cache are the two real
/// consumers today; more caches can read from the same struct later
/// without another settings-panel section.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceConfig {
    /// Max entries in the embedding cache (`embeddings::CachingEmbedder`).
    pub embedding_cache_capacity: usize,
    /// Max entries in `memory::MemoryStore`'s search-result cache.
    pub search_cache_capacity: usize,
    /// How long a cached entry stays valid, in seconds, before it's
    /// treated as a miss even if it's still present. `0` means "no
    /// expiry" -- capacity-based eviction only.
    pub cache_ttl_seconds: u64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            embedding_cache_capacity: 256,
            search_cache_capacity: 64,
            cache_ttl_seconds: 300,
        }
    }
}

impl PerformanceConfig {
    /// `cache_ttl_seconds` as an `Option<Duration>` the way
    /// `cache::CacheConfig` actually wants it -- `0` (or a value that
    /// doesn't fit in a `Duration`'s constructor cleanly, which a plain
    /// `u64` of seconds never can) becomes `None`, meaning "no expiry."
    pub fn cache_ttl(&self) -> Option<std::time::Duration> {
        if self.cache_ttl_seconds == 0 {
            None
        } else {
            Some(std::time::Duration::from_secs(self.cache_ttl_seconds))
        }
    }
}

/// Development_plan Phase 23 "Vision Support" settings (PRD 6.7 settings
/// panel). Like `ModelConfig`/`UiConfig`/`PerformanceConfig`, this is a
/// documented, round-trip-tested settings *surface* -- it isn't read
/// directly by `screenshot`/`ocr`/`vision`'s own `Tool::execute`
/// implementations, since `Tool::execute` only receives `&ToolArgs`
/// (TCLIS SS5), not a `Config`. Turning a settings value into a real
/// tool call's arguments is `assistant-core`'s job when it builds a
/// request's `ToolArgs`, not something an individual tool crate does for
/// itself -- the same not-yet-self-enforcing relationship
/// `enabled_tools`/`enabled_plugins` below already have with the tools
/// they name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VisionConfig {
    /// Tesseract language code `ocr::extract_text` expects (see that
    /// crate's `validate_language` for the accepted shape: lowercase
    /// letters, optionally `+`-joined, e.g. `"eng"` or `"eng+fra"`).
    pub ocr_language: String,
    /// Where `screenshot::capture_full_screen` writes captures by
    /// default. Matches `screenshot::tools::CaptureScreenshotTool`'s own
    /// hardcoded fallback (`default_output_dir`) exactly -- kept in sync
    /// by convention for now, the same way `PerformanceConfig`'s
    /// defaults and its consumers' own fallbacks aren't structurally
    /// tied together either.
    pub screenshot_dir: String,
}

impl Default for VisionConfig {
    fn default() -> Self {
        Self {
            ocr_language: "eng".to_string(),
            screenshot_dir: "data/screenshots".to_string(),
        }
    }
}

/// Top-level configuration matching the fields enumerated in TDS SS6 and
/// Apis SS11. Any field missing from the TOML file on disk falls back to
/// its default rather than failing to load (PRD 6.7 settings panel).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub model: ModelConfig,
    pub ui: UiConfig,
    pub performance: PerformanceConfig,
    pub vision: VisionConfig,
    pub enabled_tools: Vec<String>,
    pub enabled_plugins: Vec<String>,
    pub logging_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: ModelConfig::default(),
            ui: UiConfig::default(),
            performance: PerformanceConfig::default(),
            vision: VisionConfig::default(),
            enabled_tools: vec![
                "system_info".to_string(),
                "open_application".to_string(),
                "list_directory".to_string(),
            ],
            enabled_plugins: Vec::new(),
            logging_level: "info".to_string(),
        }
    }
}

impl Config {
    /// Loads configuration from a TOML file, falling back to defaults for
    /// any field the file does not specify. Returns an error only if the
    /// file exists but cannot be parsed (Apis SS11 `load_config`).
    pub fn load(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)?;
        toml::from_str(&raw).map_err(|e| AppError::Config(e.to_string()))
    }

    /// Persists configuration to a TOML file (Apis SS11 `save_config`).
    pub fn save(&self, path: impl AsRef<Path>) -> AppResult<()> {
        let raw = toml::to_string_pretty(self).map_err(|e| AppError::Config(e.to_string()))?;
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, raw)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_sane() {
        let cfg = Config::default();
        assert_eq!(cfg.model.active_model, "gemma-3-1b");
        assert_eq!(cfg.ui.theme, "dark");
        assert!(!cfg.ui.reduced_motion);
        assert!(!cfg.ui.high_contrast);
        assert_eq!(cfg.ui.effective_text_scale(), 1.0);
    }

    #[test]
    fn text_scale_within_range_is_used_unchanged() {
        let ui = UiConfig {
            text_scale: 1.4,
            ..UiConfig::default()
        };
        assert_eq!(ui.effective_text_scale(), 1.4);
    }

    #[test]
    fn text_scale_above_the_max_is_clamped_without_mutating_the_stored_value() {
        let ui = UiConfig {
            text_scale: 5.0,
            ..UiConfig::default()
        };
        assert_eq!(ui.effective_text_scale(), MAX_TEXT_SCALE);
        assert_eq!(ui.text_scale, 5.0, "raw stored value should be untouched");
    }

    #[test]
    fn text_scale_below_the_min_is_clamped() {
        let ui = UiConfig {
            text_scale: 0.1,
            ..UiConfig::default()
        };
        assert_eq!(ui.effective_text_scale(), MIN_TEXT_SCALE);
    }

    #[test]
    fn nan_text_scale_falls_back_to_the_default_rather_than_propagating() {
        let ui = UiConfig {
            text_scale: f32::NAN,
            ..UiConfig::default()
        };
        assert_eq!(ui.effective_text_scale(), UiConfig::default().text_scale);
    }

    #[test]
    fn missing_file_yields_default() {
        let cfg = Config::load("/nonexistent/path/default.toml").unwrap();
        assert_eq!(cfg.logging_level, "info");
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("ai-assistant-test-{}", common::Id::new()));
        let path = dir.join("config.toml");

        let mut cfg = Config::default();
        cfg.ui.theme = "light".to_string();
        cfg.ui.reduced_motion = true;
        cfg.ui.text_scale = 1.5;
        cfg.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.ui.theme, "light");
        assert!(loaded.ui.reduced_motion);
        assert_eq!(loaded.ui.text_scale, 1.5);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_config_file_written_before_phase_17_still_loads_with_sane_defaults() {
        // Simulates a real, pre-existing config.toml that predates
        // reduced_motion/high_contrast/text_scale entirely -- the whole
        // point of `#[serde(default)]` at the container level (every
        // absent field falls back individually, not just the whole
        // struct at once).
        let dir = std::env::temp_dir().join(format!("ai-assistant-test-{}", common::Id::new()));
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "[ui]\ntheme = \"light\"\nlanguage = \"en\"\n").unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.ui.theme, "light");
        assert!(!loaded.ui.reduced_motion);
        assert_eq!(loaded.ui.effective_text_scale(), 1.0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn default_performance_config_is_sane() {
        let cfg = PerformanceConfig::default();
        assert!(cfg.embedding_cache_capacity > 0);
        assert!(cfg.search_cache_capacity > 0);
        assert_eq!(cfg.cache_ttl(), Some(std::time::Duration::from_secs(300)));
    }

    #[test]
    fn zero_ttl_seconds_means_no_expiry() {
        let cfg = PerformanceConfig {
            cache_ttl_seconds: 0,
            ..PerformanceConfig::default()
        };
        assert_eq!(cfg.cache_ttl(), None);
    }

    #[test]
    fn performance_config_round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("ai-assistant-test-{}", common::Id::new()));
        let path = dir.join("config.toml");

        let mut cfg = Config::default();
        cfg.performance.embedding_cache_capacity = 512;
        cfg.performance.search_cache_capacity = 16;
        cfg.performance.cache_ttl_seconds = 60;
        cfg.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.performance.embedding_cache_capacity, 512);
        assert_eq!(loaded.performance.search_cache_capacity, 16);
        assert_eq!(loaded.performance.cache_ttl_seconds, 60);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_config_file_written_before_phase_18_still_loads_with_sane_performance_defaults() {
        // Same backward-compatibility guarantee Phase 17 already proved
        // for `[ui]`'s new fields, now for a section that didn't exist
        // in the file at all -- `[performance]` is entirely absent here.
        let dir = std::env::temp_dir().join(format!("ai-assistant-test-{}", common::Id::new()));
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "[ui]\ntheme = \"dark\"\n").unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(
            loaded.performance.embedding_cache_capacity,
            PerformanceConfig::default().embedding_cache_capacity
        );
        assert_eq!(
            loaded.performance.search_cache_capacity,
            PerformanceConfig::default().search_cache_capacity
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn default_vision_config_is_sane() {
        let cfg = VisionConfig::default();
        assert_eq!(cfg.ocr_language, "eng");
        assert_eq!(cfg.screenshot_dir, "data/screenshots");
    }

    #[test]
    fn vision_config_round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("ai-assistant-test-{}", common::Id::new()));
        let path = dir.join("config.toml");

        let mut cfg = Config::default();
        cfg.vision.ocr_language = "eng+fra".to_string();
        cfg.vision.screenshot_dir = "data/custom-screenshots".to_string();
        cfg.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.vision.ocr_language, "eng+fra");
        assert_eq!(loaded.vision.screenshot_dir, "data/custom-screenshots");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_config_file_written_before_phase_23_still_loads_with_sane_vision_defaults() {
        // Same backward-compatibility guarantee as the Phase 18 test
        // above, now for `[vision]` -- absent here entirely, same as
        // every pre-Phase-23 config.toml on disk actually is.
        let dir = std::env::temp_dir().join(format!("ai-assistant-test-{}", common::Id::new()));
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "[ui]\ntheme = \"dark\"\n[performance]\n").unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(
            loaded.vision.ocr_language,
            VisionConfig::default().ocr_language
        );
        assert_eq!(
            loaded.vision.screenshot_dir,
            VisionConfig::default().screenshot_dir
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
