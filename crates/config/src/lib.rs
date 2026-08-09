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

/// Top-level configuration matching the fields enumerated in TDS SS6 and
/// Apis SS11. Any field missing from the TOML file on disk falls back to
/// its default rather than failing to load (PRD 6.7 settings panel).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub model: ModelConfig,
    pub ui: UiConfig,
    pub enabled_tools: Vec<String>,
    pub enabled_plugins: Vec<String>,
    pub logging_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: ModelConfig::default(),
            ui: UiConfig::default(),
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
}
