//! Plugin lifecycle (Plugins.md SS6: Initialization -> Capability
//! Registration -> Permission Validation -> Activation -> Runtime ->
//! Deactivation -> Unload). Collapsed to the states that are actually
//! externally observable and worth tracking; "Capability Registration"
//! and "Runtime" aren't separate states here since they're not decision
//! points -- they happen automatically as part of `Active`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    /// Manifest found (or registered directly), not yet permission-checked.
    Discovered,
    /// Declared permissions checked against policy; ready to activate.
    Validated,
    /// `initialize()` succeeded; contributing tools, running.
    Active,
    /// `shutdown()` was called; can be validated and activated again.
    Deactivated,
    /// Something failed (bad manifest, denied permissions, a panic
    /// during initialize/shutdown) -- isolated here rather than
    /// propagating (Plugins.md SS16: "Plugin failures must not crash the
    /// application").
    Failed,
}
