//! The Plugin Manager (Development_plan Phase 11; Plugins.md SS3).
//! Ties manifests, lifecycle state, permission policy, and in-process
//! `Plugin` instances together.

use crate::lifecycle::PluginState;
use crate::manifest::PluginManifest;
use crate::plugin::Plugin;
use crate::policy::PluginPermissionPolicy;
use common::{AppError, AppResult};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

struct PluginEntry {
    manifest: PluginManifest,
    state: PluginState,
    instance: Option<Box<dyn Plugin>>,
}

#[derive(Default)]
pub struct PluginManager {
    plugins: HashMap<String, PluginEntry>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a plugin's manifest and in-process instance, starting
    /// in the `Discovered` state.
    pub fn register(
        &mut self,
        manifest: PluginManifest,
        instance: Box<dyn Plugin>,
    ) -> AppResult<()> {
        if self.plugins.contains_key(&manifest.id) {
            return Err(AppError::Module {
                module: "plugin-manager",
                message: format!("plugin {:?} is already registered", manifest.id),
            });
        }
        let id = manifest.id.clone();
        self.plugins.insert(
            id,
            PluginEntry {
                manifest,
                state: PluginState::Discovered,
                instance: Some(instance),
            },
        );
        Ok(())
    }

    /// Scans `dir` for `*/manifest.toml` (Plugins.md SS4 layout) and
    /// returns whatever manifests parse successfully. Doesn't register
    /// them: discovering a manifest doesn't produce an in-process
    /// `Plugin` instance on its own (see `plugin.rs`'s module docs for
    /// why that's this crate's design) -- a real embedder would match
    /// discovered manifests up with statically-compiled implementations
    /// it knows about, or use `dynamic.rs` for the narrower metadata-only
    /// case.
    pub fn discover_manifests(dir: &Path) -> Vec<PluginManifest> {
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return Vec::new(); // no plugins directory yet -- not an error
        };
        read_dir
            .flatten()
            .filter_map(|entry| {
                let manifest_path = entry.path().join("manifest.toml");
                manifest_path
                    .is_file()
                    .then(|| PluginManifest::load(&manifest_path).ok())
                    .flatten()
            })
            .collect()
    }

    /// Checks platform support and declared permissions against `policy`
    /// (Plugins.md SS6 "Permission Validation"). Transitions to
    /// `Validated` on success or `Failed` on refusal -- refusal is a
    /// normal, expected outcome, not a bug, so this returns `Ok(false)`
    /// rather than an error.
    pub fn validate(&mut self, id: &str, policy: &PluginPermissionPolicy) -> AppResult<bool> {
        let entry = self.entry_mut(id)?;
        if !entry.manifest.supports_current_platform() || !policy.all_approved(&entry.manifest) {
            entry.state = PluginState::Failed;
            return Ok(false);
        }
        entry.state = PluginState::Validated;
        Ok(true)
    }

    /// Calls the plugin's `initialize()`. Requires `Validated` first --
    /// permission validation is not optional or implicit.
    pub fn activate(&mut self, id: &str) -> AppResult<()> {
        let entry = self.entry_mut(id)?;
        if entry.state != PluginState::Validated {
            return Err(AppError::Module {
                module: "plugin-manager",
                message: format!(
                    "{id} must be Validated before activation (currently {:?})",
                    entry.state
                ),
            });
        }
        let Some(instance) = entry.instance.as_mut() else {
            entry.state = PluginState::Failed;
            return Err(AppError::Module {
                module: "plugin-manager",
                message: format!("{id} has no in-process instance"),
            });
        };

        // Plugins.md SS16: a plugin's failure must not crash the host.
        // AssertUnwindSafe is sound here because we mark this plugin
        // Failed and never touch it again on any panic -- there's no
        // remaining code path that could observe torn state through it.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| instance.initialize())) {
            Ok(Ok(())) => {
                entry.state = PluginState::Active;
                Ok(())
            }
            Ok(Err(e)) => {
                entry.state = PluginState::Failed;
                Err(e)
            }
            Err(_) => {
                entry.state = PluginState::Failed;
                Err(AppError::Module {
                    module: "plugin-manager",
                    message: format!("{id} panicked during initialize()"),
                })
            }
        }
    }

    pub fn deactivate(&mut self, id: &str) -> AppResult<()> {
        let entry = self.entry_mut(id)?;
        if entry.state != PluginState::Active {
            return Err(AppError::Module {
                module: "plugin-manager",
                message: format!("{id} is not Active (currently {:?})", entry.state),
            });
        }
        let Some(instance) = entry.instance.as_mut() else {
            entry.state = PluginState::Deactivated;
            return Ok(());
        };

        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| instance.shutdown())) {
            Ok(Ok(())) => {
                entry.state = PluginState::Deactivated;
                Ok(())
            }
            Ok(Err(e)) => {
                entry.state = PluginState::Failed;
                Err(e)
            }
            Err(_) => {
                entry.state = PluginState::Failed;
                Err(AppError::Module {
                    module: "plugin-manager",
                    message: format!("{id} panicked during shutdown()"),
                })
            }
        }
    }

    pub fn state(&self, id: &str) -> Option<PluginState> {
        self.plugins.get(id).map(|e| e.state)
    }

    pub fn manifest(&self, id: &str) -> Option<&PluginManifest> {
        self.plugins.get(id).map(|e| &e.manifest)
    }

    pub fn list(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.plugins.keys().map(|s| s.as_str()).collect();
        ids.sort_unstable();
        ids
    }

    /// Tools from every currently-`Active` plugin (Plugins.md SS9),
    /// ready for the host to register with its own
    /// `tool_framework::ToolRegistry` -- the exact same registry
    /// built-in tools go into, so plugin tools go through identical
    /// validation, permission checks, and timeout enforcement.
    pub fn collect_tools(&self) -> Vec<Arc<dyn tool_framework::Tool>> {
        self.plugins
            .values()
            .filter(|e| e.state == PluginState::Active)
            .filter_map(|e| e.instance.as_ref())
            .flat_map(|p| p.register_tools())
            .collect()
    }

    fn entry_mut(&mut self, id: &str) -> AppResult<&mut PluginEntry> {
        self.plugins.get_mut(id).ok_or_else(|| AppError::Module {
            module: "plugin-manager",
            message: format!("no plugin registered with id {id:?}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::PluginMetadata;

    struct TestPlugin {
        should_fail_init: bool,
    }

    impl Plugin for TestPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                id: "test.plugin".into(),
                name: "Test".into(),
                version: "1.0.0".into(),
            }
        }
        fn initialize(&mut self) -> AppResult<()> {
            if self.should_fail_init {
                Err(AppError::Module {
                    module: "test",
                    message: "deliberate failure".into(),
                })
            } else {
                Ok(())
            }
        }
        fn shutdown(&mut self) -> AppResult<()> {
            Ok(())
        }
    }

    fn manifest_with_permissions(id: &str, permissions: &[&str]) -> PluginManifest {
        let perms = permissions
            .iter()
            .map(|p| format!("{p:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        PluginManifest::from_toml(&format!(
            "id = {id:?}\nname = \"Test\"\nversion = \"1.0.0\"\nauthor = \"test\"\ndescription = \"test\"\n\
             license = \"MIT\"\nminimum_sdk_version = \"0.1.0\"\nsupported_platforms = [\"linux\"]\npermissions = [{perms}]\n"
        ))
        .unwrap()
    }

    #[test]
    fn full_lifecycle_from_register_to_deactivate() {
        let mut manager = PluginManager::new();
        manager
            .register(
                manifest_with_permissions("test.plugin", &[]),
                Box::new(TestPlugin {
                    should_fail_init: false,
                }),
            )
            .unwrap();
        assert_eq!(manager.state("test.plugin"), Some(PluginState::Discovered));

        assert!(manager
            .validate("test.plugin", &PluginPermissionPolicy::new())
            .unwrap());
        assert_eq!(manager.state("test.plugin"), Some(PluginState::Validated));

        manager.activate("test.plugin").unwrap();
        assert_eq!(manager.state("test.plugin"), Some(PluginState::Active));

        manager.deactivate("test.plugin").unwrap();
        assert_eq!(manager.state("test.plugin"), Some(PluginState::Deactivated));
    }

    #[test]
    fn undeclared_permission_fails_validation() {
        let mut manager = PluginManager::new();
        manager
            .register(
                manifest_with_permissions("test.plugin", &["network_access"]),
                Box::new(TestPlugin {
                    should_fail_init: false,
                }),
            )
            .unwrap();

        assert!(!manager
            .validate("test.plugin", &PluginPermissionPolicy::new())
            .unwrap());
        assert_eq!(manager.state("test.plugin"), Some(PluginState::Failed));
    }

    #[test]
    fn cannot_activate_without_validation() {
        let mut manager = PluginManager::new();
        manager
            .register(
                manifest_with_permissions("test.plugin", &[]),
                Box::new(TestPlugin {
                    should_fail_init: false,
                }),
            )
            .unwrap();
        assert!(manager.activate("test.plugin").is_err());
    }

    #[test]
    fn a_plugin_that_fails_to_initialize_is_marked_failed_not_crashed() {
        let mut manager = PluginManager::new();
        manager
            .register(
                manifest_with_permissions("test.plugin", &[]),
                Box::new(TestPlugin {
                    should_fail_init: true,
                }),
            )
            .unwrap();
        manager
            .validate("test.plugin", &PluginPermissionPolicy::new())
            .unwrap();

        assert!(manager.activate("test.plugin").is_err());
        assert_eq!(manager.state("test.plugin"), Some(PluginState::Failed));
    }

    #[test]
    fn duplicate_registration_is_an_error() {
        let mut manager = PluginManager::new();
        manager
            .register(
                manifest_with_permissions("test.plugin", &[]),
                Box::new(TestPlugin {
                    should_fail_init: false,
                }),
            )
            .unwrap();
        let result = manager.register(
            manifest_with_permissions("test.plugin", &[]),
            Box::new(TestPlugin {
                should_fail_init: false,
            }),
        );
        assert!(result.is_err());
    }

    #[test]
    fn active_plugins_contribute_their_tools() {
        struct ToolPlugin;
        impl Plugin for ToolPlugin {
            fn metadata(&self) -> PluginMetadata {
                PluginMetadata {
                    id: "test.tools".into(),
                    name: "x".into(),
                    version: "1.0.0".into(),
                }
            }
            fn initialize(&mut self) -> AppResult<()> {
                Ok(())
            }
            fn shutdown(&mut self) -> AppResult<()> {
                Ok(())
            }
            fn register_tools(&self) -> Vec<Arc<dyn tool_framework::Tool>> {
                vec![Arc::new(tool_framework::stubs::EchoTool)]
            }
        }

        let mut manager = PluginManager::new();
        manager
            .register(
                manifest_with_permissions("test.tools", &[]),
                Box::new(ToolPlugin),
            )
            .unwrap();
        assert!(manager.collect_tools().is_empty()); // not active yet

        manager
            .validate("test.tools", &PluginPermissionPolicy::new())
            .unwrap();
        manager.activate("test.tools").unwrap();

        let tools = manager.collect_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "echo");
    }

    #[test]
    fn unsupported_platform_fails_validation() {
        let mut manager = PluginManager::new();
        let manifest = PluginManifest::from_toml(
            "id = \"test.windows-only\"\nname = \"x\"\nversion = \"1.0.0\"\nauthor = \"x\"\ndescription = \"x\"\n\
             license = \"MIT\"\nminimum_sdk_version = \"0.1.0\"\nsupported_platforms = [\"windows\"]\n",
        )
        .unwrap();
        manager
            .register(
                manifest,
                Box::new(TestPlugin {
                    should_fail_init: false,
                }),
            )
            .unwrap();
        assert!(!manager
            .validate("test.windows-only", &PluginPermissionPolicy::new())
            .unwrap());
    }

    #[test]
    fn discover_manifests_on_a_missing_directory_returns_empty_not_error() {
        assert!(
            PluginManager::discover_manifests(Path::new("/definitely/not/a/real/plugins/dir"))
                .is_empty()
        );
    }

    #[test]
    fn discover_manifests_finds_real_manifest_files() {
        let dir = std::env::temp_dir().join("plugin-manager-discover-test");
        let plugin_dir = dir.join("weather-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("manifest.toml"),
            "id = \"com.example.weather\"\nname = \"Weather\"\nversion = \"1.0.0\"\nauthor = \"x\"\ndescription = \"x\"\n\
             license = \"MIT\"\nminimum_sdk_version = \"0.1.0\"\nsupported_platforms = [\"linux\"]\n",
        )
        .unwrap();

        let found = PluginManager::discover_manifests(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "com.example.weather");

        std::fs::remove_dir_all(&dir).ok();
    }
}
