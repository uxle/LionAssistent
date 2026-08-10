# Changelog

All notable changes to this project are documented here. Format loosely
follows [Keep a Changelog](https://keepachangelog.com/); this project uses
Semantic Versioning once it reaches 0.1.0+ stability milestones.

## [Unreleased]

### Added (Phase 18)

- `cache` crate (new): `Cache<K, V>` (capacity- and optional-TTL-bounded, brute-force LRU eviction -- deliberately, at this workspace's actual scale, the same reasoning `vector_db::VectorIndex` already documents for its own brute-force search) and `SharedCache<K, V>` (`Arc<Mutex<_>>`, the same sharing idiom `permission-manager`/`search` already use). Zero dependencies. 24/24 tests.
- `vector_db::VectorIndex::search`: replaced a full `sort_by` with `select_nth_unstable_by` partial selection when `top_k < n`. A first draft only compared scores and could reorder tied entries differently than the old stable sort under some runs; fixed by ranking on `(score descending, original position ascending)` so the unstable algorithm produces byte-for-byte identical output to the old stable one for every input, ties included -- see `docs/ROADMAP.md`'s Phase 18 notes for the full story and the 128-entry equivalence test that would have caught it.
- `embeddings::CachingEmbedder<E>`: memoizes any `Embedder`'s `embed()` by exact text via a `SharedCache`. Verified byte-identical output to the unwrapped embedder. 12/12 tests (+7).
- `memory::MemoryStore`: a bounded `search_memory` result cache, cleared on every `remember`/`forget` so a cached hit can never be stale -- verified by dedicated invalidation tests, not just cache-hit tests. New `open_with_search_cache`/`in_memory_with_search_cache` constructors added alongside the existing `open`/`in_memory` (unchanged). 13/13 tests (+4); downstream `search` re-verified unchanged.
- `assistant_core::LazyResponseGenerator`: defers constructing the real backend until the first `generate()` call. Tested as a real drop-in `Box<dyn ResponseGenerator>` inside a genuine `AssistantCore`, not just in isolation. 9/9 tests (+4).
- `utilities::run_blocking`: a shared, tested `spawn_blocking` wrapper with panic-safe `AppError` mapping -- the same pattern `tool_framework::ToolRouter::execute` has used bespoke since Phase 7, now available to every other call site. `utilities`' first real dependencies (`tokio`, `common`) since the crate was scaffolded in Phase 1. 6/6 tests (+4).
- `metrics::MetricsRegistry::time`/`MetricsSnapshot::slowest_timers`: per-phase timing built on Phase 15's existing timer storage rather than a new structure. 16/16 tests (+5).
- `config::PerformanceConfig`: `embedding_cache_capacity`/`search_cache_capacity`/`cache_ttl_seconds`, added to `Config` the same way Phase 17 added `UiConfig`'s new fields -- `#[serde(default)]`, additive, backward-compatible with a config file that predates the section entirely (tested). 12/12 tests (+4).
- `app` (unverified, same pre-existing reason as ever -- see below): model loading deferred via `LazyResponseGenerator`; the background task's blocking work (`process_message`, `save_session`) now runs via `utilities::run_blocking` instead of directly inline, fixing a genuine blocking-in-async-task issue; `open_memory` now builds a `CachingEmbedder` and calls `open_with_search_cache`, both sized from `config.performance`; a `MetricsRegistry` times startup and logs the slowest phases; `ui::run` call site fixed to the three-argument signature Phase 17 already needed. Verified against real rustc via a throwaway, never-committed harness reproducing these exact patterns outside the workspace (sidestepping the unrelated, pre-existing Slint/Candle walls) -- it compiled and then actually ran a message through the real event-bus/spawn_blocking round trip. See `docs/ROADMAP.md`'s Phase 18 notes for the full harness writeup.
- No new tool this phase, deliberately -- `security`/`sandbox` (Phase 16) set the precedent for infrastructure-only phases; here, Phase 15's `get_diagnostics_report` already surfaces this phase's new `MetricsRegistry` timers for free, no diagnostics-crate changes needed.
- 437/437 tests passing across all 32 non-Slint, non-Candle crates (+55 over Phase 17).

### Added (Phase 17)

- `config::UiConfig`: `reduced_motion`, `high_contrast`, `text_scale` (clamped `0.8..=2.0` via `effective_text_scale()`) -- real, compiled, tested Rust (`config` has no Slint dependency). 8/8 tests, including backward compatibility with a config file written before these fields existed.
- `theme.slint`: the full typography scale, shadow levels, complete radius variants, and semantic status colors Phase 2's own comment already flagged as deferred here, plus `Palette.high-contrast`/`text-scale` and a new `Motion` global for reduced-motion-aware animation tokens.
- `app-window.slint`: a reusable `Activatable` component (real keyboard focus + activation + focus ring, not pointer-only); `accessible-role`/`accessible-label` throughout; a responsive sidebar collapse; a `ThinkingIndicator`; and a switch from `ScrollView`+`for` to Slint's own `ListView` for the message list (UI-UX SS18 "efficient rendering of long conversations").
- `ui::run` gained a `&config::UiConfig` parameter and a `waiting` property driving the new thinking indicator around the send/receive flow.
- **Explicitly not compile-verified**, by Lion's direct choice after a much deeper re-investigation of the Slint blocker than previous phases documented -- see `docs/ROADMAP.md`'s Phase 17 notes for the full nine-attempt investigation and why it's a materially bigger wall than the dependency-pin fixes that worked in Phases 14/16. 382/382 tests still passing across every crate that *can* be verified.

### Added (Phase 16)

- `security` crate: `MasterKey` (generate / derive from a passphrase via Argon2 / load-or-create a `0600` key file, zeroized on drop, `Debug`-redacted), AES-256-GCM authenticated encryption (`encrypt`/`decrypt`, tamper and wrong-key detection verified by test), `SecureStore` (SQLite-backed, stores only opaque encrypted blobs), and `SecretManager` (the plaintext-in/plaintext-out API on top). `key.rs`'s doc comment is explicit about exactly what the file-backed key scheme does and doesn't protect against -- no overclaiming. Deliberately exposes **no** assistant tool: a secret value becoming visible in a tool result is close to the opposite of storing it encrypted in the first place.
- `sandbox` crate: `ProcessLimits`/`apply_limits` (CPU time, memory, open files, process count, max file size via POSIX rlimit) -- empirically confirmed enforced in this environment (a child given a 1-second CPU limit and told to spin forever is actually killed, not left running; this is a real, automated test, not a claim). `FilesystemSandbox` (Landlock-based path restriction) is correct, real code that compiled and ran but reported `NotEnforced` in this specific sandbox -- see `docs/ROADMAP.md` for the full investigation. A standalone `examples/filesystem_sandbox_demo.rs` demonstrates it outside the automated suite, since the real call is process-wide and irreversible.
- Both crates ship with zero assistant tools, deliberately -- this phase is infrastructure ("Enterprise-grade security" as a foundation), not new LLM-facing surface area.
- 377/377 tests passing across all 31 non-Slint, non-Candle crates.

### Added (Phase 15)

- `metrics` crate: `MetricsRegistry` (thread-safe counters/gauges/timers, deterministic `BTreeMap`-ordered snapshots) and `PerformanceMonitor` (samples real CPU/memory/uptime/load via Phase 8's `linux_services::system::system_info`, records into the registry, keeps a bounded rolling history). A `run_periodic_sampling` helper exists but isn't started anywhere automatically -- PRD SS3 rules out hidden background behavior.
- `telemetry` crate: strictly opt-in, double-gated (`enabled` AND `endpoint` must both be explicit) usage telemetry. `TelemetryEventKind` is a closed enum, not a free-form string -- callers cannot construct an event outside the fixed vocabulary; the one optional label is length-capped. `record()` always no-ops when disabled; `flush()` is the only network touchpoint in the crate and is a true no-op (verified against an unreachable address, not just by inspection) unless both gates are open.
- `diagnostics` crate: `install_panic_hook` writes a structured local JSON crash report for any panic on any thread, then chains to whatever hook was previously installed -- normal panic output is unaffected. Crash-report construction/writing is tested independently of mutating the global hook, with one additional test exercising the full `catch_unwind` -> hook -> file path safely. `build_report`/`get_diagnostics_report` (real, `ReadOnly`, `ToolCategory::System` tool) ties metrics + crash count + telemetry status together -- never telemetry's buffered event content.
- 342/342 tests passing across all 29 non-Slint, non-Candle crates.

### Added (Phase 14)

- `network` crate: `HttpClient` (synchronous, matching every other stateful crate in this workspace -- see its doc comment), real `GET`/`POST`+JSON/streaming `download` via `ureq`, backed by the OS's own certificate trust store (`rustls-native-certs`) rather than a compiled-in root list -- see `docs/ROADMAP.md`'s Phase 14 notes for why that distinction mattered in practice, not just in theory, in this sandbox.
- `network::testing::MockServer`: a ~50-line, std-only, one-request HTTP/1.1 mock server, so `network`'s own tests and `updater`'s never touch the real internet.
- `updater` crate: `check_for_update`/`download_update`, comparing the running app's version (via `semver`, not string comparison) against a small original JSON manifest format at a pre-configured URL. Never auto-installs -- downloads to disk and stops there; see `docs/ROADMAP.md` for why self-replacing the running binary is out of scope on purpose.
- `check_for_updates` (`PermissionLevel::ReadOnly`) and `download_update` (`PermissionLevel::Moderate`, requires confirmation under the default `SecurityRules`): real `ToolCategory::PackageManagement` tools. Neither accepts a URL as input -- the manifest URL is fixed at construction time, never something a tool call can redirect.
- `crates/updater/tests/permission_integration.rs`: proves `check_for_updates` runs without confirmation and `download_update` is genuinely gated (denied under `AlwaysDeny`, proceeds under `AlwaysApproveForTestingOnly`) against a real `PermissionManager`.
- Deliberately not built: a generic "fetch any URL" tool (broader security surface than anything TCLIS's tool categories call for) and any auto-install/self-update flow (Phase 20's packaging formats are the right place for real updates to flow through).
- 312/312 tests passing across all 26 non-Slint, non-Candle crates.

### Added (Phase 13)

- `search` crate: `ranking::score_text` (deterministic, dependency-free relevance scoring), `FileSearchProvider` (wraps Phase 8's real `linux_services::files::search`), `MemorySearchProvider` (wraps Phase 5's real `memory::MemoryStore::search_memory`), `CommandSearchProvider` (indexes Phase 7's registered tools + Phase 10's saved, enabled workflows), `SearchEngine` (the "Global Search" module, merging all three into one ranked list).
- `search_files`, `search_memory`, `search_commands`, `global_search`: four new `PermissionLevel::ReadOnly`, `ToolCategory::Search` tools -- the first real use of `ToolCategory::Search`, declared in Phase 7 and unused since. Registered via `register_all`, same pattern as `linux_services::tools`.
- `crates/search/tests/permission_integration.rs`: proves all four search tools run cleanly through a real `PermissionManager` (Phase 9) configured to deny everything above ReadOnly, and that calls are genuinely audit-logged.
- `MemoryStore` is `Send` but not `Sync` (unshared `Connection`/`RefCell`-cached index); `MemorySearchProvider` wraps it in `Arc<Mutex<_>>` itself, the same fix `permission-manager` already applied to its own `Connection`, without changing Phase 5's code.
- Along the way: fixed a pre-existing `clippy::box_default` finding in `memory`'s own test helper (one-line, zero-risk); noticed but deliberately left alone a pre-existing `clippy::not_unsafe_ptr_arg_deref` finding in `example-plugin`'s FFI code (Phase 11, out of this phase's scope to safely touch) -- see `docs/ROADMAP.md` Phase 13 notes for both.
- 286/286 tests passing across all 24 non-Slint, non-Candle crates.

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
