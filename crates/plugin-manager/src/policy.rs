//! Which plugin-declared permissions the user has approved (Plugins.md
//! SS13: "Users can review, grant, revoke, or disable plugin permissions
//! at any time").
//!
//! A simpler, capability-based cousin of Phase 9's tool-level
//! `permission_manager::PermissionManager`, not a replacement for it:
//! this only gates whether a plugin is allowed to *activate* at all.
//! Once an active plugin's tools are registered and actually called,
//! they go through the real `PermissionManager` exactly like any
//! built-in tool (via `tool_framework::ToolRouter`) -- see
//! `docs/ROADMAP.md` Phase 11 for how the two connect.

use crate::manifest::{PluginManifest, PluginPermission};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub struct PluginPermissionPolicy {
    approved: HashMap<String, HashSet<PluginPermission>>,
}

impl PluginPermissionPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn approve(&mut self, plugin_id: &str, permission: PluginPermission) {
        self.approved
            .entry(plugin_id.to_string())
            .or_default()
            .insert(permission);
    }

    pub fn revoke(&mut self, plugin_id: &str, permission: PluginPermission) {
        if let Some(set) = self.approved.get_mut(plugin_id) {
            set.remove(&permission);
        }
    }

    pub fn is_approved(&self, plugin_id: &str, permission: PluginPermission) -> bool {
        self.approved
            .get(plugin_id)
            .is_some_and(|set| set.contains(&permission))
    }

    /// `true` only if every permission the manifest declares has been
    /// approved -- a plugin declaring five capabilities and being
    /// approved for four isn't allowed to activate at all (Plugins.md
    /// SS14: "Plugins must not... Access undeclared capabilities" cuts
    /// both ways -- a plugin also shouldn't run *without* everything it
    /// said it needs, silently degraded).
    pub fn all_approved(&self, manifest: &PluginManifest) -> bool {
        manifest
            .permissions()
            .iter()
            .all(|p| self.is_approved(&manifest.id, *p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unapproved_permission_is_denied_by_default() {
        let policy = PluginPermissionPolicy::new();
        assert!(!policy.is_approved("plugin.x", PluginPermission::NetworkAccess));
    }

    #[test]
    fn approving_grants_it() {
        let mut policy = PluginPermissionPolicy::new();
        policy.approve("plugin.x", PluginPermission::NetworkAccess);
        assert!(policy.is_approved("plugin.x", PluginPermission::NetworkAccess));
    }

    #[test]
    fn revoking_removes_it() {
        let mut policy = PluginPermissionPolicy::new();
        policy.approve("plugin.x", PluginPermission::NetworkAccess);
        policy.revoke("plugin.x", PluginPermission::NetworkAccess);
        assert!(!policy.is_approved("plugin.x", PluginPermission::NetworkAccess));
    }

    #[test]
    fn all_approved_requires_every_declared_permission() {
        let manifest = PluginManifest::from_toml(
            r#"
            id = "plugin.x"
            name = "x"
            version = "1.0.0"
            author = "x"
            description = "x"
            license = "MIT"
            minimum_sdk_version = "0.1.0"
            supported_platforms = ["linux"]
            permissions = ["network_access", "clipboard"]
        "#,
        )
        .unwrap();

        let mut policy = PluginPermissionPolicy::new();
        assert!(!policy.all_approved(&manifest));

        policy.approve("plugin.x", PluginPermission::NetworkAccess);
        assert!(!policy.all_approved(&manifest)); // clipboard still missing

        policy.approve("plugin.x", PluginPermission::Clipboard);
        assert!(policy.all_approved(&manifest));
    }

    #[test]
    fn a_manifest_with_no_permissions_is_trivially_all_approved() {
        let manifest = PluginManifest::from_toml(
            r#"
            id = "plugin.y"
            name = "y"
            version = "1.0.0"
            author = "y"
            description = "y"
            license = "MIT"
            minimum_sdk_version = "0.1.0"
            supported_platforms = ["linux"]
        "#,
        )
        .unwrap();
        assert!(PluginPermissionPolicy::new().all_approved(&manifest));
    }
}
