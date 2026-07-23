//! `plugin-manager` -- manifests, lifecycle, permission policy, and
//! loading for third-party plugins (Development_plan Phase 11;
//! Plugins.md).
//!
//! Two loading mechanisms, each honest about what it actually is:
//! - [`PluginManager::register`]: in-process registration of a
//!   [`Plugin`] implementation. The primary, safe mechanism -- see
//!   `plugin.rs`'s module docs for why.
//! - [`dynamic::load_dynamic_plugin_info`]: real `dlopen`-based loading
//!   of a compiled `.so`, across a plain C ABI boundary. Proves the
//!   loading mechanism itself genuinely works; deliberately narrower
//!   than the full `Plugin` trait -- see `dynamic.rs`'s module docs.

pub mod dynamic;
pub mod lifecycle;
pub mod manager;
pub mod manifest;
pub mod plugin;
pub mod policy;

pub use dynamic::{load_dynamic_plugin_info, DynamicPluginInfo};
pub use lifecycle::PluginState;
pub use manager::PluginManager;
pub use manifest::{PluginManifest, PluginPermission};
pub use plugin::{Plugin, PluginMetadata};
pub use policy::PluginPermissionPolicy;
