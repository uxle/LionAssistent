# Changelog

All notable changes to this project are documented here. Format loosely
follows [Keep a Changelog](https://keepachangelog.com/); this project uses
Semantic Versioning once it reaches 0.1.0+ stability milestones.

## [Unreleased]

### Added (Phase 12)

- `voice` crate: `wav` (dependency-free WAV read/write, chunk-scanning parser), `tts::EspeakTts` (real speech synthesis via `espeak-ng`, no model download needed), `stt::PocketsphinxStt` (real transcription via `pocketsphinx`, models bundled by `apt`), `vad::VoiceActivityDetector` (energy-based, honestly scoped short of true wake-word recognition).
- Unlike Phase 4's LLM, no model download was needed here at all -- both `espeak-ng` and `pocketsphinx-en-us` ship complete models via `apt`. Verified empirically before writing tests that STT accuracy on synthetic (TTS-generated) speech is poor, an expected property of the classical-acoustic-model-vs-robotic-voice combination, not a bug; tests check pipeline reliability (produces output for real speech, none for silence) rather than exact accuracy.
- 247/247 tests passing across all 27 non-Slint, non-Candle crates.
- Recovered mid-phase from a full sandbox filesystem reset (this environment's documented "resets between tasks" behavior) by restoring the last saved tarball (Phase 11) -- full git history intact, re-verified 227/227 before continuing.

### Added (Phase 11)

- `plugin-manager` crate: `PluginManifest` (TOML parsing, SS15 SDK version compatibility), `PluginState` lifecycle, `PluginPermissionPolicy`, `PluginManager` (discovery, validation, activation/deactivation with panic isolation, tool collection into `tool_framework::ToolRegistry`).
- `Plugin` trait: the primary, safe, in-process loading mechanism -- plugin tools use the exact same `tool_framework::Tool` interface as built-in ones.
- `dynamic`: real `dlopen`-based loading via `libloading` (pinned 0.8.9), across a plain C ABI boundary rather than Rust trait objects (Rust has no stable cross-compiler ABI). A real `example-plugin` crate compiles to an actual `.so` that the test suite loads and queries.
- 227/227 tests passing across all 25 non-Slint, non-Candle crates.

### Added (Phase 10)

- `workflow-engine` crate: `Workflow`/`WorkflowStep` (named, persisted, referencing real tools -- unlike Phase 6's ephemeral, unfabricated `Plan`), dependency-graph validation with conditional-branch ordering checks, `parallel_stages()` for identifying concurrency-safe steps.
- `RunControl`: genuine pause/resume/cancel, tested with a background task confirmed to actually block mid-pause, not just return immediately.
- `run_workflow`: executes through `tool-framework`'s real `ToolRouter`, so Phase 9's Permission Manager gates workflow steps like any other tool call.
- `Scheduler`: pure "what's due now" logic, no background timer.
- `WorkflowStore`: SQLite-backed persistence for workflows and run history.
- Caught a real bug this phase: a test assumed a recurring schedule fires immediately; the implementation's actual (and correct) behavior delays the first fire by one interval, matching cron/`setInterval` convention. Fixed the test and documented the semantic explicitly.
- 203/203 tests passing across all 23 non-Slint, non-Candle crates.

### Added (Phase 9)

- `permission-manager` crate: `SecurityRules`/`PermissionDecision` (policy), `ConfirmationHandler` trait + `AlwaysDeny`/`AlwaysApproveForTestingOnly`, `PermissionManager` (SQLite-backed remembered decisions with expiry, audit logging, `check_and_log` pipeline).
- `tool-framework::ToolRouter` gained a pluggable `PermissionChecker`, replacing the `max_auto_level` threshold with `permission-manager`'s real authorization -- fully backward compatible.
- `linux-services` registers `DeleteFileTool`, deferred in Phase 8 pending exactly this confirmation flow.
- Full-stack integration test (`crates/linux-services/tests/permission_integration.rs`) proving linux-services + tool-framework + permission-manager genuinely work together: confirmation denial/approval, remembered decisions, Administrator-level policy denial.
- Caught a real bug this phase: the audit log's `message` never included the tool name (it went into a separate `module` field); fixed.
- 175/175 tests passing across all 21 non-Slint, non-Candle crates.

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
