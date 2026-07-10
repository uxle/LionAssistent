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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub theme: String,
    pub language: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            language: "en".to_string(),
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
        cfg.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.ui.theme, "light");

        std::fs::remove_dir_all(&dir).ok();
    }
}
