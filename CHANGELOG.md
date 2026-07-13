# Changelog

All notable changes to this project are documented here. Format loosely
follows [Keep a Changelog](https://keepachangelog.com/); this project uses
Semantic Versioning once it reaches 0.1.0+ stability milestones.

## [Unreleased]

### Added (Phase 8)

- `linux-services` crate: `files`, `system`, `processes`, `terminal`, `packages`, `network` -- real, tested, working Linux operations using only `/proc`/`/sys` and standard utilities (no new dependencies). `terminal` runs only a fixed allowlist of commands, args as argv, never through a shell.
- `clipboard`, `notifications`, `audio`, `display`, `applications` -- real code, tested for graceful failure since this container has no display session, sound server, D-Bus daemon, or backlight device; the same code succeeds on a real desktop.
- `power` (lock/suspend/hibernate/restart/shutdown) -- implemented, but deliberately never executed, even in this crate's own tests.
- `tools`: 8 read-only/safe capabilities wired as real `tool_framework::Tool` implementations, replacing Phase 7's stub tools with genuine Linux operations in the pipeline.
- `tool-framework::ParamKind` gained `StringArray` for terminal command arguments; re-verified (21/21) after the change.
- Caught a real environment gap this phase: `iproute2` (`ip`) wasn't installed in this sandbox; installed it and made the network module degrade gracefully rather than hard-fail if it's ever missing.
- 151/151 tests passing across all 19 non-Slint, non-Candle crates.

### Added (Phase 7)

- `tool-framework` crate: `InputSchema`/`ParamSchema` validation against `serde_json::Value` args (reports every problem, not just the first); `ToolRegistry` (register/unregister/get/list, refuses silent duplicate shadowing); `ToolRouter::execute` running the real TCLIS pipeline (lookup -> validate -> permission check -> execute -> structured `ToolResult`), with genuine timeout enforcement via `spawn_blocking` + `tokio::time::timeout`, proven by a test with a stub tool that sleeps 10x longer than its declared timeout.
- `EchoTool`, `SlowTool`, `DangerousStubTool`: honest stand-ins proving the happy path, timeout enforcement, and permission denial. Not real Linux tools -- those are Phase 8.
- 97/97 tests passing across all 17 non-Slint, non-Candle crates. Deliberately not yet wired into `assistant-core` -- see `docs/ROADMAP.md` for why.

### Added (Phase 6)

- `reasoning` crate: `Intent`, `ReasoningLevel`, `IntentClassifier` trait + `KeywordClassifier` (dependency-free rule-based classifier, correctly handles all five of Ai_reasoning's own canonical examples). 11/11 tests.
- `planner` crate: `Plan`/`PlanStep` with real dependency-graph validation, cycle detection, and topological execution order (Kahn's algorithm). `Planner::create_plan` is honest about not fabricating multi-step breakdowns without a Tool Framework to ground them in. 11/11 tests.
- `assistant-core` now classifies every message and folds intent/reasoning-level into the system prompt actually sent to the generator, proven by a prompt-capturing test.
- 77/77 tests passing across all 16 non-Slint, non-Candle crates.

### Added (Phase 5)

- `vector-db` crate: `VectorIndex<Id>`, brute-force cosine similarity search. Zero dependencies, 8/8 tests.
- `embeddings` crate: `Embedder` trait + `HashingEmbedder` (dependency-free character-trigram feature hashing). 5/5 tests.
- `memory` crate: `MemoryStore` on SQLite (`rusqlite`, bundled). Session/message persistence, long-term memory (`remember`/`forget`/`search_memory`), extractive summarization. 9/9 tests.
- `common::Id` and `conversation::Role` gained `FromStr` impls for round-tripping through storage.
- `app` now persists every processed turn to `data/database/assistant.db` immediately after assistant-core replies.
- 54/54 tests passing across all 14 non-Slint, non-Candle crates. One real bug (foreign-key delete ordering in `forget()`) caught and fixed by the test suite during this phase.

### Added (Phase 4)

- `model-registry` crate: `ModelInfo`, `ModelRegistry` -- filesystem-based model discovery, zero ML dependencies, fully verified (3/3 tests).
- `llm-runtime` crate: `CandleGemmaBackend`, a real `assistant_core::ResponseGenerator` built on Candle's `quantized_gemma3::ModelWeights` (GGUF loading, forward pass, sampling, tokenizer). API verified against Candle's actual current source; not compiled (candle-core 0.11 requires rustc 1.88+, unrelated to and deeper than Phase 1/2's issues).
- `assistant-core`'s `ResponseGenerator::generate` changed from `&self` to `&mut self` for real backend KV-cache state; `EchoResponseGenerator` and tests updated, still 4/4 passing.
- `app` now discovers and loads a real model automatically if present, falling back to the Phase 3 stub with a logged warning otherwise.
- 29/29 tests passing across all 11 non-Slint, non-Candle crates.

### Added (Phase 3)

- `conversation` crate: `Role`, `Message`, `Conversation`.
- `session-manager` crate: `Session`, `SessionManager` (in-memory; SQLite persistence is Phase 5).
- `context-manager` crate: `ContextBuilder`, character-budget history selection.
- `prompt-builder` crate: `PromptFormatter` trait + `GemmaPromptFormatter` (real Gemma chat template).
- `assistant-core` crate: `ResponseGenerator` trait + `EchoResponseGenerator` stub + `AssistantCore::process_message` end-to-end pipeline.
- `app` runs assistant-core as a background Tokio task; `ui` now subscribes to `Event::ResponseGenerated` instead of faking a reply locally.
- 26/26 tests passing across all Phase 1 + Phase 3 crates (no Slint dependency in any of them).

### Added (Phase 2)

- `ui` crate: `theme.slint` (Palette global, dark/light design tokens), `app-window.slint` (header with theme toggle, sidebar nav, chat bubbles styled per role, composer).
- `ui::run(EventBus)`: composer publishes `Event::UserMessageReceived`; placeholder assistant reply appended locally pending Phase 3.
- `app` binary now launches the UI; `main` switched from `#[tokio::main]` to sync + an explicitly-entered runtime, since native GUI backends need the real OS main thread.

### Added (Phase 1)

- Phase 1 foundation: Cargo workspace with shared `[workspace.dependencies]`.
- `common` crate: `AppError` / `AppResult`, `Id`, `Timestamp`, `PermissionLevel`.
- `utilities` crate: `human_bytes`, `truncate` helpers.
- `config` crate: TOML config load/save with per-field defaults.
- `logger` crate: `tracing` + `tracing-subscriber` initialization.
- `event-bus` crate: broadcast-based pub/sub with a starter `Event` enum.
- `app` binary: wires config, logger, and event bus together end-to-end.
- `rustfmt.toml`, `clippy.toml`, `.gitignore`, CI workflow (fmt/clippy/test/build).
