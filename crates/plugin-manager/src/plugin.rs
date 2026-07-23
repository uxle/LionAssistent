//! The plugin SDK interface (Plugins.md SS8 "SDK Interfaces").
//!
//! Implementing this trait and handing an instance to
//! [`crate::PluginManager::register`] is this crate's primary loading
//! mechanism: the plugin runs in-process, as ordinary Rust code compiled
//! into (or linked with) the host. This sidesteps a real problem with
//! genuinely dynamic loading: Rust has no stable ABI across compiler
//! versions (or even different dependency graphs of the same version),
//! so passing a Rust trait object across a `dlopen`ed boundary isn't
//! safely guaranteed to work unless the plugin and host were built by
//! the exact same toolchain against the exact same type definitions --
//! fragile in a way that's a poor fit for "third-party plugins" (Phase
//! 11's actual deliverable) as a general capability.
//!
//! [`crate::dynamic`] separately proves that loading a real `.so` file
//! *does* work in this sandbox, using a plain C ABI boundary instead of
//! Rust types -- see that module's docs for how the two relate.

use common::AppResult;
use std::sync::Arc;
use tool_framework::Tool;

#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
}

/// Plugins.md SS8: `initialize`/`shutdown`/`metadata`/`capabilities`/
/// `health_check`/`settings` are the required interface; `register_*`
/// methods are optional (default: contributes nothing of that kind).
/// Only `register_tools` is implemented so far, since `tool_framework`
/// is the one extension point that already exists end-to-end (Phases
/// 7-10); `register_ui`/`register_workflows`/`register_memory_provider`/
/// `register_models` are natural, well-motivated additions once their
/// own extension points are further along.
pub trait Plugin: Send + Sync {
    fn metadata(&self) -> PluginMetadata;

    fn initialize(&mut self) -> AppResult<()>;
    fn shutdown(&mut self) -> AppResult<()>;

    /// A quick, cheap self-check the manager can call anytime after
    /// activation (Plugins.md SS8 `health_check`). Not a substitute for
    /// `initialize`'s own error handling -- this is for detecting a
    /// plugin that initialized fine but has since gotten into a bad
    /// state.
    fn health_check(&self) -> bool {
        true
    }

    /// Tools this plugin contributes (Plugins.md SS9 "Tool
    /// Registration"), using the exact same `tool_framework::Tool` trait
    /// built-in tools implement (Phase 7) -- a plugin's tools go through
    /// identical validation, permission checks (Phase 9), and timeout
    /// enforcement as any built-in one. No separate, weaker path.
    fn register_tools(&self) -> Vec<Arc<dyn Tool>> {
        Vec::new()
    }
}
