//! Developer Platform (Development_plan Phase 29: "Plugin
//! Marketplace", "SDK Improvements", "Templates", "Code Generator").
//!
//! Builds on Phase 11's `plugin-manager` rather than duplicating it:
//! this crate is entirely about *authoring and discovering* plugins,
//! never about running them -- loading, lifecycle, and permission
//! validation all stay exactly where Phase 11 put them.
//!
//! - [`catalog`] -- "Plugin Marketplace": a local, searchable index
//!   over installed plugin manifests, honestly scoped (see that
//!   module's own docs for why "local" is deliberate, not a
//!   placeholder for an unbuilt remote one).
//! - [`templates`] / [`generator`] -- "Templates" and "Code
//!   Generator" together: three real template files rendered into a
//!   complete, three-file plugin crate implementing the actual
//!   `plugin_manager::Plugin` trait. Proven to genuinely compile and
//!   pass its own generated tests in
//!   `tests/generated_plugin_compiles.rs` -- a real `cargo build`/
//!   `cargo test` run against real generated output, not an
//!   inspection of the text produced.
//! - [`lint`] -- "SDK Improvements": a second, softer validation layer
//!   on top of `PluginManifest::from_toml`'s hard parse requirements,
//!   catching things that are technically valid but likely wrong
//!   (a non-semver version, a platform list that will always fail
//!   validation on this OS, ...) before a plugin author discovers them
//!   the hard way.
//!
//! ## Deliberately not built this phase
//!
//! - **A remote catalog source.** No real, hosted marketplace exists
//!   for this from-scratch project to honestly query yet -- see
//!   `catalog`'s own module docs.
//! - **UI/Theme/Workflow/Memory/Voice plugin templates.** Only the
//!   Tool Plugin category has a fully real, proven extension point
//!   (`tool_framework::Tool`, wired through `PluginManager` since
//!   Phase 11) to generate genuinely working code against; the other
//!   categories Plugins.md SS7 lists don't have one yet, and a
//!   template for a plugin type this workspace can't actually load
//!   would be exactly the kind of "looks right, does nothing" gap
//!   this project avoids.
//! - **A `cargo generate`-style CLI.** `generator::generate_plugin` is
//!   a plain library function this phase; `app` doesn't yet depend on
//!   `plugin-manager` or any other tool-providing crate either (mcp's
//!   and remote-control's own phases both name this same gap), so
//!   wiring a command-line entry point ahead of that would be reaching
//!   past where the rest of this workspace currently stands.

pub mod catalog;
pub mod generator;
pub mod lint;
pub mod templates;

pub use catalog::{CatalogEntry, CatalogSource, PluginCatalog};
pub use generator::{generate_plugin, PluginSpec};
pub use lint::{lint_manifest, LintWarning};
