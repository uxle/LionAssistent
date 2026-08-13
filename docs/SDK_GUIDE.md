# Plugin SDK Guide

How to write a plugin against `plugin-manager`'s real, tested API
(Development_plan Phase 11; `Plugins.md`, the project's Plugin SDK
specification). Every signature below is copied from the actual current
source, not the aspirational spec doc -- cross-check `crates/plugin-manager/src/`
if anything here looks stale.

## Why plugins are in-process Rust, not `.so` files

`Plugin` is a Rust trait; loading a plugin means constructing a
`Box<dyn Plugin>` and handing it to `PluginManager::register` -- compiled
into (or linked with) the host, not `dlopen`ed at runtime. This is a
deliberate choice, not a shortcut: Rust has no stable ABI across
compiler versions or dependency graphs, so passing a `Box<dyn Trait>`
across a real `dlopen` boundary isn't safely guaranteed to work unless
the plugin and host were built by the exact same toolchain against the
exact same type definitions. `plugin_manager::dynamic` separately proves
real `.so` loading *does* work in this sandbox, but over a plain C ABI
boundary, not Rust trait objects -- see that module if you need genuinely
out-of-process, language-agnostic plugins; this guide is for the
in-process Rust SDK.

## The `Plugin` trait

```rust
pub trait Plugin: Send + Sync {
    fn metadata(&self) -> PluginMetadata;

    fn initialize(&mut self) -> AppResult<()>;
    fn shutdown(&mut self) -> AppResult<()>;

    fn health_check(&self) -> bool { true }              // optional, defaults to "healthy"
    fn register_tools(&self) -> Vec<Arc<dyn Tool>> { Vec::new() }  // optional, defaults to none
}
```

Only `register_tools` is implemented today -- `tool_framework` is the one
extension point that exists end-to-end (Phases 7-10).
`register_ui`/`register_workflows`/`register_memory_provider`/
`register_models` (`Plugins.md` SS8) are natural additions once their
own extension points mature further; adding one means adding a method
with a default no-op implementation, the same shape `register_tools`
already has, so existing plugins don't break.

**A plugin's tools go through the identical pipeline a built-in tool
does** -- the same validation, the same permission checks, the same real
timeout enforcement via `spawn_blocking` (`ToolRouter::execute`, Phase
7/9). There is no separate, weaker path for plugin-contributed tools.
This isn't just asserted: `crates/integration-tests/tests/cross_crate_flows.rs`
proves it two ways -- a plugin tool executing successfully through a
real router, and a plugin tool's panic *during execution* (not just
during `initialize`/`shutdown`) being caught cleanly rather than
crashing the host.

## The manifest

`PluginManifest` is plain TOML (`Plugins.md` SS5), loaded via
`PluginManifest::from_toml(text)` or `PluginManifest::load(path)`:

```toml
id = "com.example.my-plugin"
name = "My Plugin"
version = "1.0.0"
author = "Jane Developer"
description = "Does something useful."
license = "MIT"
minimum_sdk_version = "0.1.0"
supported_platforms = ["linux"]
permissions = []               # optional, defaults to empty
# homepage = "..."             # optional
# repository = "..."           # optional
```

`id`, `name`, `version`, `author`, `description`, `license`,
`minimum_sdk_version`, and `supported_platforms` are required;
`homepage`, `repository`, and `permissions` default sensibly if omitted.

## Lifecycle

```rust
use plugin_manager::{PluginManager, PluginManifest, PluginPermissionPolicy};

let mut manager = PluginManager::new();

// 1. Register: manifest + in-process instance, starts in `Discovered`.
manager.register(manifest, Box::new(MyPlugin::default()))?;

// 2. Validate: checks the manifest's declared `permissions` against a
//    policy the host controls (PRD 6.9/Archd SS13: users can review,
//    grant, or revoke a plugin's permissions at any time).
let mut policy = PluginPermissionPolicy::new();
// policy.approve("com.example.my-plugin", PluginPermission::FileAccess); // grant what it actually needs
manager.validate("com.example.my-plugin", &policy)?;

// 3. Activate: calls the plugin's own `initialize()`. A panic inside
//    initialize()/shutdown() is caught (catch_unwind) and marks the
//    plugin `Failed` rather than taking down the host -- a *different*
//    panic-safety mechanism from the one that protects tool execution
//    (see the cross-crate test noted above).
manager.activate("com.example.my-plugin")?;

// 4. Use: gather every active plugin's tools into a real ToolRegistry.
for tool in manager.collect_tools() {
    registry.register(tool)?;
}

// 5. Deactivate when done (calls shutdown()).
manager.deactivate("com.example.my-plugin")?;
```

`manager.state(id)`, `manager.manifest(id)`, and `manager.list()` are
available for inspection at any point (e.g. a future Plugin Manager UI
panel per `UI-UX_` SS11).

## A complete minimal example

`crates/example-plugin/` is a real, working, tested reference
implementation -- read it alongside this guide, not instead of it. It
registers one tool (`tool_framework::stubs::EchoTool`-equivalent) and
exercises the full lifecycle in its own tests.

## Testing your plugin

Follow the pattern in `crates/plugin-manager/src/manager.rs`'s own
tests and `crates/integration-tests/tests/cross_crate_flows.rs`:

- Unit-test your `Plugin` impl directly (construct it, call
  `initialize()`, check `register_tools()` returns what you expect).
- If your plugin's tools do anything security-relevant, add adversarial
  cases the way `crates/integration-tests/tests/security_adversarial.rs`
  does for the built-in tools -- don't assume the shared pipeline's
  general-purpose tests cover your specific tool's edge cases.
- If you want to prove your plugin's tool survives a real panic cleanly
  (recommended for anything doing I/O or parsing untrusted input),
  mirror `a_plugin_tools_panic_during_execution_is_isolated_not_a_crash`
  in `cross_crate_flows.rs`.
