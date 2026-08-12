//! `integration-tests` -- cross-crate integration, adversarial security,
//! and stress tests (Development_plan Phase 19 "Testing").
//!
//! Every other crate in this workspace already carries its own thorough
//! unit tests (437 of them as of Phase 18), and a few crates already
//! have their own crate-local `tests/permission_integration.rs`
//! (`linux-services`, `search`, `updater`) proving specific real
//! multi-crate pipelines. What doesn't exist anywhere yet is a place for
//! tests that are inherently *about* more than one or two crates working
//! together, or about behavior (load, adversarial input, panic
//! isolation) that a crate's own unit tests aren't the natural home for.
//! This crate is that place -- no library code of its own, just
//! `tests/`:
//!
//! - `tests/cross_crate_flows.rs` -- genuinely new combinations not
//!   covered elsewhere: `plugin-manager`'s tools reaching a real
//!   `tool_framework::ToolRouter` (including a panic during actual tool
//!   *execution*, not just plugin lifecycle -- `plugin-manager`'s own
//!   tests already cover lifecycle panics), and `assistant-core` +
//!   `tool-framework` + `permission-manager` + `memory` cooperating in
//!   one scenario.
//! - `tests/security_adversarial.rs` -- TDS SS10's "Permission bypass
//!   attempts / Invalid tool inputs / Plugin isolation / File access
//!   restrictions", investigated for real against this workspace's
//!   actual code rather than asserted in a document.
//! - `tests/stress.rs` -- `cache`, `vector-db`/`memory`, and `event-bus`
//!   at a scale two or three orders of magnitude past what any unit test
//!   uses, checking correctness holds (not just that nothing panics)
//!   under real load.
//!
//! Every dependency here is a `[dev-dependencies]` one: nothing in the
//! rest of the workspace depends on this crate, and it depends only on
//! already-independently-verified crates -- no Slint, no Candle, so
//! (unlike `app`) everything in it is expected to build, run, and prove
//! something for real.
