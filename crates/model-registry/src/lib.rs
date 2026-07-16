//! `model-registry` -- tracks locally available models (Mms SS6 "Model
//! Management", SS16 "Model Registry"). Deliberately has zero ML
//! dependencies: it only deals with metadata and file paths, so it can be
//! built and tested anywhere, independent of whether the actual inference
//! backend (`llm-runtime`, Candle-based) can compile on a given machine.

use std::path::{Path, PathBuf};

/// Metadata for one locally available model (Mms SS6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    pub name: String,
    pub family: String,
    pub path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub context_length: u32,
    pub quantization: Option<String>,
}

/// A registry of installed models, discovered by filesystem convention: a
/// model named `foo` under directory `dir` is considered installed when
/// both `dir/foo.gguf` and `dir/foo.tokenizer.json` exist.
#[derive(Debug, Default)]
pub struct ModelRegistry {
    models: Vec<ModelInfo>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a model, replacing any existing entry with the same name.
    pub fn register(&mut self, info: ModelInfo) {
        self.models.retain(|m| m.name != info.name);
        self.models.push(info);
    }

    pub fn get(&self, name: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.name == name)
    }

    pub fn list(&self) -> &[ModelInfo] {
        &self.models
    }

    /// Looks for `{dir}/{name}.gguf` and `{dir}/{name}.tokenizer.json`;
    /// registers and returns `true` if both exist, otherwise leaves the
    /// registry untouched and returns `false` (PRD 6.6: fail visibly
    /// rather than guessing).
    pub fn discover(&mut self, dir: &Path, name: &str, family: &str, context_length: u32) -> bool {
        let path = dir.join(format!("{name}.gguf"));
        let tokenizer_path = dir.join(format!("{name}.tokenizer.json"));
        if path.exists() && tokenizer_path.exists() {
            self.register(ModelInfo {
                name: name.to_string(),
                family: family.to_string(),
                path,
                tokenizer_path,
                context_length,
                quantization: None,
            });
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("model-registry-test-{label}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn discover_returns_false_when_files_missing() {
        let dir = temp_dir("missing");
        let mut registry = ModelRegistry::new();
        assert!(!registry.discover(&dir, "gemma-3-1b", "gemma", 8192));
        assert!(registry.get("gemma-3-1b").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn discover_registers_when_both_files_exist() {
        let dir = temp_dir("present");
        std::fs::write(dir.join("gemma-3-1b.gguf"), b"fake").unwrap();
        std::fs::write(dir.join("gemma-3-1b.tokenizer.json"), b"{}").unwrap();

        let mut registry = ModelRegistry::new();
        assert!(registry.discover(&dir, "gemma-3-1b", "gemma", 8192));
        let info = registry.get("gemma-3-1b").unwrap();
        assert_eq!(info.family, "gemma");
        assert_eq!(info.context_length, 8192);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn register_replaces_existing_entry_with_same_name() {
        let mut registry = ModelRegistry::new();
        registry.register(ModelInfo {
            name: "m".into(),
            family: "gemma".into(),
            path: "a".into(),
            tokenizer_path: "b".into(),
            context_length: 1,
            quantization: None,
        });
        registry.register(ModelInfo {
            name: "m".into(),
            family: "gemma".into(),
            path: "c".into(),
            tokenizer_path: "d".into(),
            context_length: 2,
            quantization: None,
        });
        assert_eq!(registry.list().len(), 1);
        assert_eq!(registry.get("m").unwrap().context_length, 2);
    }
}
