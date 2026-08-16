//! Code Generation, Project Analysis, and Debugging (Development_plan
//! Phase 24 "Coding Agent"; Ai_reasoning SS15 names a future "Coding
//! Agent" as one of several specialized reasoning modules that "can be
//! added without redesigning the Coordinator Agent").
//!
//! # Why one crate, not three
//!
//! Phase 23's `screenshot`/`ocr`/`vision` split into separate crates
//! because each wrapped a genuinely distinct external dependency
//! (ImageMagick, tesseract, the `image` crate) with its own install
//! story. None of this phase's three capabilities have that
//! shape -- [`analysis`] and [`diagnostics`] need nothing beyond
//! `std::fs`, `toml`, and `serde_json` (already workspace dependencies),
//! and [`generation`] depends only on `assistant-core`, also already
//! present. Three tightly-related modules with no distinct dependency
//! footprint is `linux-services`'/`workflow-engine`'s shape, not Phase
//! 23's -- so that's the one this crate follows.
//!
//! # What's real here, and what deliberately isn't
//!
//! - [`analysis::analyze_project`] and [`diagnostics::run_cargo_diagnostics`]
//!   are fully real: directory traversal, TOML/JSON manifest parsing, and
//!   a real `cargo check --message-format=json` invocation with its
//!   output actually parsed (verified during development against a
//!   deliberately-broken scratch crate -- see `diagnostics`' own doc
//!   comment for the exact real JSON shape that verification produced).
//! - [`generation::generate_from_template`] is real, deterministic
//!   scaffolding -- no model involved, so nothing here is blocked by the
//!   Candle/rustc wall.
//! - [`generation::generate_freeform`] is the one piece that's honestly
//!   *not* real yet: it exercises `assistant_core::ResponseGenerator`
//!   the same way `AssistantCore::process_message` does, but with no
//!   real backend wired into this crate's own tests or default
//!   registration, it produces `EchoResponseGenerator`'s same honest
//!   stub reply Phase 3 already established -- not fabricated code, an
//!   explicit "no real backend configured" signal instead
//!   (Ai_reasoning SS17: "must not fabricate completed actions").
//!   Swapping in a real backend (`llm-runtime`, once it compiles on a
//!   modern toolchain) needs no change here -- the same "replaceable AI
//!   backend" property Archd SS3 already established for chat applies
//!   identically to code generation, since both go through the same
//!   trait.

pub mod analysis;
pub mod diagnostics;
pub mod generation;
pub mod tools;
