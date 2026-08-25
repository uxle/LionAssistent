# Roadmap & Phase Status

Full detail lives in the project's `Development_plan` document. This file
tracks build status per phase as the code lands, so it's always clear what
actually exists versus what's still planned.

| Phase | Name | Status |
|---|---|---|
| 1 | Foundation | 🟢 Done -- workspace, `common`, `config`, `logger`, `event-bus`, `app` stub. CI/build scripts done. Fully verified (build+test+run) in-sandbox. |
| 2 | UI Framework (Slint) | 🟡 Written, NOT compile-verified in this sandbox -- see note below. Main window, theme (dark/light), sidebar nav, chat bubbles, composer done. |
| 3 | Assistant Core | 🟢 Done and fully verified -- conversation, session-manager, context-manager, prompt-builder, assistant-core all build+test clean in this sandbox (no Slint dependency). Stub response generator only; real LLM is Phase 4. |
| 4 | LLM Runtime | 🟡 Written, NOT compile-verified in this sandbox -- Candle itself hard-requires rustc 1.88+. `model-registry` (metadata, no ML deps) is fully verified. See note below. |
| 5 | Memory | 🟢 Done and fully verified -- `vector-db`, `embeddings`, `memory` all build+test clean in this sandbox (no Slint/Candle dependency). SQLite via `rusqlite` (bundled), real cosine-similarity search, real (non-neural) embeddings. |
| 6 | Planner & Reasoning | 🟢 Done and fully verified -- `reasoning`, `planner` build+test clean, and are now wired into `assistant-core`. Rule-based classifier (no LLM needed), real dependency-graph planning with cycle detection. See note below on scope. |
| 7 | Tool Framework | 🟢 Done and fully verified -- `tool-framework` builds+tests clean. Real registry, router, JSON-arg validation, and genuine timeout enforcement (proven by a test that actually times one out). Stub tools only; real Linux tools are Phase 8. Not yet wired into `assistant-core` -- see note below. |
| 8 | Linux Integration | 🟢 Done and fully verified -- `linux-services` builds+tests clean (151/151 workspace tests). Files/system/processes/terminal/packages/network are real and tested for genuine success. Clipboard/notifications/audio/display/applications are real code tested for graceful failure (no display session in this container). Power is implemented but never executed, even in tests. 8 capabilities wired into Phase 7's tool pipeline. |
| 9 | Permission System | 🟢 Done and fully verified -- `permission-manager` builds+tests clean, now the *real* checker behind `tool-framework`'s router (replacing the `max_auto_level` stand-in), with SQLite-backed remembered decisions and audit logging. Full-stack integration test proves linux-services + tool-framework + permission-manager genuinely work together. 175/175 workspace tests. |
| 10 | Workflow Engine | 🟢 Done and fully verified -- `workflow-engine` builds+tests clean. Named, persisted, SQLite-backed workflows over real tool-framework tool calls (not Phase 6's inferred, unfabricated plans); dependency-graph validation with conditional branches; genuine pause/resume/cancel; a pure, testable scheduler. 203/203 workspace tests. |
| 11 | Plugin SDK | 🟢 Done and fully verified -- `plugin-manager` builds+tests clean, including *real* `.so` dynamic loading via `libloading` (not just in-process registration). 227/227 workspace tests. See note below on the two loading mechanisms and why. |
| 12 | Voice | 🟢 Done and fully verified -- `voice` builds+tests clean, including *real* speech synthesis (`espeak-ng`) and *real* transcription (`pocketsphinx`, models bundled via `apt` -- no download needed, unlike Phase 4). 247/247 workspace tests. See note below on mechanical reliability vs. accuracy. |
| 13 | Search | 🟢 Done and fully verified -- `search` builds+tests clean (286/286 workspace tests). File search wraps Phase 8's real filesystem search; memory search wraps Phase 5's real semantic search; command search indexes Phase 7's registered tools and Phase 10's saved workflows; Global Search merges all three via one dependency-free ranking function. All four exposed as real `ToolCategory::Search` tools (declared in Phase 7, unused until now), gated by Phase 9's real Permission Manager. See note below on scope (no settings search yet) and a pre-existing clippy finding noticed along the way. |
| 14 | Networking | 🟢 Done and fully verified -- `network` and `updater` build+test clean (312/312 workspace tests). Real HTTPS via `ureq` + the OS's own certificate trust store (not a compiled-in list -- see the Phase 14 notes below for why that distinction mattered in practice here, not just in theory), verified against a real external host during development and against a std-only local mock server in the test suite. `check_for_updates`/`download_update` are real `ToolCategory::PackageManagement` tools; neither accepts a URL from a caller, and neither auto-installs anything -- see notes below. |
| 15 | Diagnostics | 🟢 Done and fully verified -- `metrics`, `telemetry`, and `diagnostics` build+test clean (342/342 workspace tests). Real CPU/memory/uptime sampling (reuses Phase 8's `linux_services::system`, nothing reimplemented); telemetry is double-gated opt-in with a closed, non-identifying event vocabulary enforced by the type system, not just documentation; a global panic hook writes structured local crash reports without disturbing normal panic output. `get_diagnostics_report` is a real `ReadOnly` tool. See notes below. |
| 16 | Security | 🟢 Done and fully verified -- `security` and `sandbox` build+test clean (377/377 workspace tests). Real AES-256-GCM + Argon2 (RustCrypto, not hand-rolled), with tamper/wrong-key detection actually tested, not assumed. Real, empirically-verified `rlimit`-based process resource limits (a CPU-limited child is genuinely killed, tested). A Landlock filesystem sandbox is correct, real code whose enforcement could **not** be verified in this specific sandbox -- see notes below for the full investigation, which is different in kind from the Phase 2/4 toolchain gaps. No new tools this phase, deliberately. |
| 17 | UI Polish | 🟡 Written, mostly **not** compile-verified -- by explicit choice (Lion confirmed after a fresh, deeper investigation of the Slint blocker; see notes below). `config::UiConfig`'s new fields (`reduced_motion`/`high_contrast`/`text_scale`) are real, compiled, tested Rust (8/8 `config` tests, +5 over Phase 16) since `config` has no Slint dependency. Everything touching `theme.slint`/`app-window.slint`/`ui/src/lib.rs` joins Phase 2 in "written but unverified," same as it's been since Phase 2 -- 382/382 tests still passing across every crate that *can* be verified. |
| 18 | Performance | 🟢 Done and fully verified -- `cache` (new), plus real changes to `vector-db`, `embeddings`, `memory`, `assistant-core`, `utilities`, `metrics`, and `config`, all build+test clean (437/437 workspace tests, +55 over Phase 17). Capacity/TTL-bounded caching (embedder memoization, memory search-result cache, both invalidation-tested, not just hit-tested); a real algorithmic win in `vector_db::VectorIndex::search` (partial selection instead of a full sort, proven equivalent to the old behavior including tie-breaking against a 128-entry reference); deferred model loading (`LazyResponseGenerator`, tested as a real drop-in `assistant-core` backend); a shared `spawn_blocking` helper (`utilities::run_blocking`) fixing a genuine blocking-in-async-task issue found in `app`'s background loop; and per-phase startup timing (`metrics::MetricsRegistry::time`/`slowest_timers`). No new tool, deliberately -- see notes below. `app`'s own wiring remains unverified for the same pre-existing reason as ever (needs `ui`), but was checked against real rustc via a throwaway harness exercising the identical patterns -- see notes below.
| 19 | Testing | 🟢 Done and fully verified -- new `integration-tests` crate (447/447 workspace tests, +10 over Phase 18), plus dependency-free benchmarks for `vector-db` and `cache`. Ten new cross-crate integration/security/stress tests investigate real gaps found by reading the actual code first, not padding: a plugin tool's panic *during execution* (not just lifecycle) proven caught by the router; two honestly-documented security findings (direct `Tool::execute()` bypasses every router check; `read_file` reaches any OS-readable path unconfirmed, by design, pending Landlock actually being applied at startup); capacity/correctness held under real concurrent load. `criterion` hit the same workspace-wide Candle/rustc-version wall as Slint (confirmed empirically, not assumed) -- see notes below for why hand-rolled benchmarks turned out to be the more consistent choice anyway.
| 20 | Packaging | 🟢 Done -- DEB, RPM, AppImage, and a portable tarball all really built, installed (DEB/RPM where the sandbox's own package database allowed it), run, and (DEB) removed against a stand-in binary. Flatpak's manifest is confirmed fully valid by real `flatpak-builder` parsing (`--show-manifest` succeeds); Snap's is correct per spec but only generically YAML-validated -- both limits confirmed empirically (network/tooling checks), not assumed. Fixed two pre-existing gaps found along the way: no LICENSE file existed despite the workspace declaring one since Phase 1, and no `assets/` directory existed at all.
| 21 | Documentation | 🟢 Done -- `cargo doc` builds with zero warnings across all 33 verified crates (four real doc-link bugs found and fixed getting there). Four new runnable `examples/*.rs`, all run for real. `docs/API.md`, `docs/DEVELOPER_GUIDE.md`, `docs/SDK_GUIDE.md`, `docs/USER_GUIDE.md` added -- the User Guide explicit from its first line about what's actually been verified versus described from the code. Fixed a small pre-existing config drift found along the way (`config/default.toml` missing Phase 18's `[performance]` section). This phase's own clean-room tarball verification also caught a genuine build-order hazard in `plugin-manager`'s dynamic-loading tests (masked all session by one long-lived `target/` directory) -- confirmed the README's actual documented commands aren't affected, documented the hazard for anyone running a subset of tests directly.
| 22 | Stable Release (v1.0) | 🟢 Done -- **deliberately not tagged v1.0.0**; see `docs/RELEASE_NOTES.md` for why and an honest assessment against PRD SS18's Definition of Done. `docs/PERFORMANCE_REVIEW.md` checks every measurable target from TDS/DBDMS/Archd against real benchmark data. One real bug fix: Phase 16's Landlock sandbox is now actually wired into `app`'s startup, closing the gap Phase 19's security tests documented. Branch `lts/0.1.0-foundation` created at this commit.
| 23 | Vision Support | 🟢 Done and fully verified for `screenshot`/`ocr`/`vision` (481/481 workspace tests, +34 over Phase 22, 36 verified crates) -- `screenshot` genuinely captures against a real (if virtual, via Xvfb) X11 display, not graceful-failure-only like Phase 8's audio/notifications; `ocr` genuinely reads real generated text via `tesseract`; `vision` decodes real images for dimensions/format/color. `vision-runtime` (a real CLIP-based zero-shot classifier implementing `vision::VisionBackend`) joins `llm-runtime`/`ui`/`app` in the unverified-in-this-sandbox category -- same Candle/rustc 1.88+ wall, confirmed to reproduce exactly. See notes below.
| 24 | Coding Agent | 🟢 Done and fully verified (512/512 workspace tests, +31 over Phase 23, 37 verified crates) -- one crate, `coding-agent`, all three capabilities (analysis, diagnostics, generation) real with no unverified half this time, unlike Phase 23's `vision-runtime`. `run_diagnostics` (real `cargo check --message-format=json` integration) is this workspace's first `Destructive`-level tool; a real `build.rs`-executing scratch crate proved the permission gate actually stops it from running, not just returns a denial. See notes below.
| 25 | Multi-Agent System | 🟢 Done and fully verified (542/542 workspace tests, +30 over Phase 24, 38 verified crates) -- one crate, `agents`: a real `Coordinator`/`AgentMessage` dispatch layer (deliberately mirrors `ToolRouter`'s pipeline one layer up) plus five concrete agents, each wrapping an already-real crate rather than reimplementing it. `ResearchAgent` is explicitly local-only (PRD SS3 non-goal: no cloud dependency); a documented, corrected overclaim about `PlannerAgent`/`SecurityAgent` pairing readiness. See notes below.
| 26 | RAG System | 🟢 Done and fully verified (569/569 workspace tests, +27 over Phase 25, 39 verified crates) -- one crate, `rag`: real SQLite + `VectorIndex` persistence (mirrors `memory::MemoryStore`'s proven shape), real PDF text extraction (`pdf-extract`, pinned like `vision`'s `image`/`memory`'s `rusqlite`), real word-window chunking. Two genuine empirical findings along the way: a wrong hand-derived chunk count caught before shipping, and a real tesseract misread at an image's edges (fixed with a wider fixture, not a looser assertion). See notes below.
| 27 | Model Context Protocol (MCP) | 🟢 Done and fully verified (617/617 workspace tests, +48 over Phase 26, 40 verified crates) -- one crate, `mcp`: a real client (spawns a peer subprocess, real stdio JSON-RPC handshake), a real server (exposes this assistant's own `ToolRouter` over stdio, same permission pipeline as any other caller), and the "External Tool Integration" bridge itself (discovered remote tools become ordinary, permission-gated `tool_framework::Tool`s). Targets spec revision 2025-11-25, not the newer 2026-07-28 stateless revision -- a deliberate, empirically-checked choice (a real, currently-published npm reference server was spawned and probed directly), not an oversight. Two real subprocess-boundary test paths: this crate's own compiled `mcp-stub-server` binary (always run) and the real third-party reference server via `npx` (`#[ignore]`d by default, run explicitly and confirmed passing). See notes below.
| 28 | Mobile Companion | 🟢 Done and fully verified (665/665 workspace tests, +48 over Phase 27, 41 verified crates) -- one crate, `remote-control`: real pairing (6-digit code, 5-minute TTL, 5-attempt cap, LAN-only, SHA-256-hashed tokens never stored raw), a real `tiny_http`-based server routing every tool call through the actual `ToolRouter::execute` pipeline, and SSE-relayed live notifications from the real `event_bus`. "Android App" confirmed unbuildable in this sandbox before deciding anything (no installed target stdlib, no rustup, Google's servers blocked, a real cross-compile attempt failed as predicted) -- same category of wall as Phase 20's Flatpak/Snap -- so it's a real, tested, dependency-free PWA instead, not a spec description of an APK. Two real bugs found by the real client/server network test and fixed properly: query-string-aware route dispatch, and a missing flush that caused a genuine hang, fixed with correct `Transfer-Encoding: chunked` framing. See notes below.
| 29 | Developer Platform | 🟢 Done and fully verified (695/695 workspace tests, +30 over Phase 28, 42 verified crates) -- one crate, `plugin-sdk`: a local plugin catalog (reuses `PluginManager::discover_manifests` rather than duplicating it), a real code generator producing a complete, compiling `Plugin`-trait plugin crate from three checked-in templates, and a soft manifest linter layered on top of `PluginManifest`'s hard parse rules. Proven with a real `cargo build`/`cargo test` against real generated output, which caught a genuine `edition2024` dependency-resolution wall (the generated crate has its own independent `Cargo.lock`, so it doesn't inherit the main workspace's pins) and a real test-cost finding (sharing one `CARGO_TARGET_DIR` across generated plugins reuses Cargo's own build cache). See notes below.
| 30 | AI Operating System Integration | 🟢 Done and fully verified (731/731 workspace tests, +36 over Phase 29, 43 verified crates) -- one crate, `os-integration`: real window management (`wmctrl`/`xdotool`, tested against a real Xvfb+fluxbox+xterm stack), real systemd user-service management (verification honestly partial -- this sandbox has no running systemd to reach), and a real D-Bus native API routing through the same `ToolRouter::execute` pipeline as every other transport. "LionOS Integration" and a literal "Full OS Control Layer" are both named and declined with full reasoning rather than built or silently skipped -- see notes below and `os-integration/src/lib.rs`'s own crate docs. This is the last phase Development_plan originally named; see the closing note below the table for what that does and doesn't mean.

**On finishing Phase 30:** every phase on Development_plan's original 1-30 roadmap has now been attempted, and each was either delivered for real (verified against actual running code, actual binaries, actual test failures found and fixed) or explicitly declined with stated reasoning when the honest answer was "not yet possible" (LionOS, having no real target) or "not consistent with this project's own values" (a literal full-control layer, contradicting PRD SS3's non-goals). That is a real, meaningful form of completion -- but it is deliberately not being called "v1.0" here. DIG SS17 defines Alpha/Beta/Stable as their own separate milestone track ("Full documentation, Performance tuning, Security review, Long-term support plan" for Stable specifically), and nothing in this roadmap's own phase numbering was ever a stand-in for that track. Declaring a release milestone this document was never asked to declare would be exactly the kind of overclaim every phase before this one has tried hard to avoid. What's true and can be stated plainly: 43 verified crates, 731 passing tests, zero clippy warnings, zero doc warnings, a clean-room-verified tarball at every phase boundary, and a written, honest account of every wall this project hit along the way -- Rust 1.75, Flathub's network, the Android SDK, a sandbox with no systemd, and, this phase, a future operating system that does not exist yet. Whether and when to pursue a Stable release, or a genuinely new roadmap once LionOS is real, is Lion's call to make next, not this document's to presume.

## Phase 1 checklist

- [x] Cargo workspace (`resolver = "2"`, shared `[workspace.dependencies]`)
- [x] `common` crate: `AppError` / `AppResult`, `Id`, `Timestamp`, `PermissionLevel`
- [x] `config` crate: TOML load/save with per-field defaults
- [x] `logger` crate: `tracing` + `tracing-subscriber` init
- [x] `event-bus` crate: broadcast-based pub/sub, starter `Event` enum
- [x] `app` binary: wires the above together, runs end-to-end
- [x] `rustfmt.toml`, `clippy.toml`, `.gitignore`
- [x] CI workflow (fmt, clippy, test, build)
- [ ] `utilities` crate gets used by real callers (currently only self-tested)
- [ ] Root `README.md` kept in sync as new crates land

## Phase 2 checklist

- [x] `ui` crate: `theme.slint` (Palette global: dark/light tokens)
- [x] `app-window.slint`: header + theme toggle, sidebar nav (6 items), scrollable chat bubbles (user/assistant/system styling), composer
- [x] `ui::run(EventBus)`: composer publishes `Event::UserMessageReceived`; placeholder assistant reply appended locally (clearly labeled, since Phase 3 doesn't exist yet)
- [x] `app` binary launches the UI; `main` is sync (not `#[tokio::main]`) since native GUI backends need the real OS main thread -- a multi-threaded Tokio runtime is still built and entered for Phase 3+ to spawn background work from UI callbacks
- [ ] **Not done:** subscribing to `Event::ResponseGenerated` to render a real backend's replies -- deferred to Phase 3, where there's an actual producer to build and test against

### ⚠️ Toolchain note (read before building Phase 2)

This sandbox only has Rust 1.75 via `apt` (rustup's own servers aren't
reachable from its network allowlist). Slint's dependency tree has moved
well past what 1.75 can parse -- newest Slint needs rustc 1.77+, and its
transitive deps (even just to run `slint-build` in `build.rs`) hit crates
requiring the `edition2024` Cargo feature, which needs rustc 1.85+.

Phase 1's crates (`common`, `utilities`, `config`, `logger`, `event-bus`)
were verified for real: build, 9/9 tests, and the binary actually running.
Phase 2's code (`ui`, and `app` since it now depends on `ui`) is written
carefully and `cargo fmt --check` confirms it's syntactically valid Rust,
but it could **not** be compiled end-to-end in this sandbox, so the actual
Slint API calls (property/callback names, model downcasting, etc.) are
unverified. Build it on a machine with Rust 1.85+ (via
[rustup](https://rustup.rs)) and fix whatever cargo's error messages turn
up -- that's a normal, fast edit-compile loop once a real toolchain is
available, not a redesign.

`cargo build -p common -p utilities -p config -p logger -p event-bus`
still works fine in this sandbox and is unaffected by any of this.

## Phase 3 checklist

- [x] `conversation`: `Role`, `Message` (matches DBDMS Messages table, `token_count` reserved for Phase 4's tokenizer), `Conversation`
- [x] `session-manager`: `Session` (matches DBDMS Sessions table), `SessionManager` (create/get/append/end) -- in-memory only, SQLite persistence is Phase 5
- [x] `context-manager`: `ContextBuilder` -- keeps the newest messages within a character budget, always keeps at least the newest message
- [x] `prompt-builder`: `PromptFormatter` trait + `GemmaPromptFormatter` (real Gemma turn-template: `<start_of_turn>user/model...<end_of_turn>`)
- [x] `assistant-core`: `ResponseGenerator` trait (swappable backend) + `EchoResponseGenerator` stub (proves the pipeline ran; not a real reply) + `AssistantCore::process_message` wiring all of the above together
- [x] `app` runs assistant-core as a background Tokio task subscribed to `UserMessageReceived`, publishing `ResponseGenerated` back
- [x] `ui` now subscribes to `ResponseGenerated` (via `slint::invoke_from_event_loop`) instead of faking a reply locally -- the deferred Phase 2 item is now done
- [x] All 5 new crates: 26/26 tests passing across the whole non-UI workspace, verified in this sandbox
- [ ] **Not done:** planning, tool calls, memory retrieval -- Assistant Core's `process_message` is intentionally linear (append -> context -> prompt -> generate -> append) until Phases 5-7 exist

## Phase 4 checklist

- [x] `model-registry`: `ModelInfo`, `ModelRegistry` -- filesystem-based discovery (`{name}.gguf` + `{name}.tokenizer.json`). Zero ML dependencies; fully verified (3/3 tests).
- [x] `llm-runtime`: `CandleGemmaBackend` implementing `assistant_core::ResponseGenerator` via [Candle](https://github.com/huggingface/candle)'s real `quantized_gemma3::ModelWeights` (GGUF loading, forward pass, KV-cache position tracking, `LogitsProcessor` sampling, tokenizer encode/decode).
- [x] `assistant-core`'s `ResponseGenerator::generate` changed from `&self` to `&mut self` -- a real backend needs mutable KV-cache state; `EchoResponseGenerator` and tests updated to match, still 4/4 passing.
- [x] `app` tries `model-registry` discovery first; loads `CandleGemmaBackend` if both model files are found, falls back to `EchoResponseGenerator` with a logged warning otherwise -- the fallback path itself doesn't need real weights and is straightforward to reason about, though it's exercised through `app`, which can't compile here either (see below).
- [ ] **Not done:** streaming generation (`generate_stream`) -- current `generate` returns the full reply at once; streaming is a reasonable next increment once this is verified against a real model
- [ ] **Not done:** GPU/backend auto-selection (Mms SS13) -- hardcoded to `Device::Cpu`, appropriate for a 1B model as a first cut

### ⚠️ Toolchain note: llm-runtime could not be compiled here

Two separate, unrelated blockers stacked up for Phase 4:

1. **This sandbox can't reach Hugging Face or any model host** (only
   crates.io/npm/PyPI/GitHub/Ubuntu are reachable), so no Gemma 3 GGUF
   weights or tokenizer could be downloaded here regardless of code
   quality. True Gemma integration testing needs a real machine.
2. **Candle itself needs a newer Rust than this sandbox has.**
   `candle-core` 0.11's dependency on `zip = "^8.6.0"` requires rustc
   1.88+ -- and unlike Phase 1/2's issues, this isn't an open-ended range
   that resolves to something too new by accident; it's Candle's own
   direct, non-negotiable requirement. Older `candle-core` releases (up
   through 0.9.x) use `zip ^1.1.1` instead and would parse fine under
   1.75, but it's unconfirmed whether they include the `quantized_gemma3`
   module specifically, so downgrading risked silently losing Gemma 3
   support for the sake of an in-sandbox green checkmark -- not a good
   trade. Latest Candle was kept.

Given both blockers, `llm-runtime`'s code was written against Candle's
**actual current source** (fetched and read directly, not recalled from
memory -- see the doc comment at the top of `crates/llm-runtime/src/lib.rs`
for the exact API basis) rather than compiled. `cargo fmt --check` does
confirm it's syntactically valid Rust. Build it for real on a machine with
Rust 1.85+ and a Gemma 3 1B GGUF file + tokenizer.json; expect a normal
edit-compile loop against candle's actual type signatures, not a redesign.

`model-registry`, and everything from Phase 1 and Phase 3, remain fully
verified and unaffected:

```bash
cargo build -p common -p utilities -p config -p logger -p event-bus \
  -p conversation -p session-manager -p context-manager -p prompt-builder \
  -p assistant-core -p model-registry
```

## Phase 5 checklist

- [x] `vector-db`: `VectorIndex<Id>`, brute-force cosine similarity search -- appropriate at local single-user scale (hundreds to low-thousands of memories, not millions). Zero dependencies. 8/8 tests.
- [x] `embeddings`: `Embedder` trait + `HashingEmbedder` (character-trigram feature hashing -- real, deterministic, dependency-free; captures lexical overlap, not deep semantic meaning). A neural embedder can implement the same trait later, once Phase 4's toolchain constraint is resolved on a real machine. 5/5 tests.
- [x] `memory`: `MemoryStore` on SQLite (`rusqlite`, bundled -- no system libsqlite3 needed). Sessions/messages persistence (idempotent re-saves), long-term memories (`remember`/`forget`/`search_memory`/`list_memories`), extractive session summarization. 9/9 tests, including a real bug caught and fixed during this phase (see below).
- [x] `common::Id` and `conversation::Role` gained `FromStr` impls so `memory` can parse them back out of SQLite without every downstream crate needing its own `uuid`/string-matching logic.
- [x] `app` now opens `data/database/assistant.db` at startup and persists every processed turn immediately after `assistant-core` replies.
- [x] 54/54 tests passing across all 14 non-Slint, non-Candle crates.
- [ ] **Not done:** abstractive (model-generated) summarization -- current `summarize_session` is extractive (keeps head/tail messages verbatim); a real summary needs a working `llm-runtime`, Phase 4's own unresolved constraint
- [ ] **Not done:** UI-level "resume last session" -- data persists correctly, but `app` still always starts a fresh session; wiring the UI to offer resuming is a reasonable follow-up, not attempted here to avoid over-scoping this phase

### A bug this phase's tests actually caught

`forget()` originally deleted the `memories` row before the `embeddings`
row that references it via a `FOREIGN KEY`, which SQLite correctly
rejected. Wrong order, caught immediately by
`remembers_and_forgets_long_term_memories` failing, fixed by deleting the
child row first. This is exactly why these crates are built with tests
alongside them rather than after the fact -- worth calling out as a
concrete example, not just an abstract claim of "verified."

## Phase 6 checklist

- [x] `reasoning`: `Intent` (12 categories, Ai_reasoning SS6), `ReasoningLevel` (0-4, Ai_reasoning SS5), `IntentClassifier` trait + `KeywordClassifier` (real, dependency-free rule-based classifier -- correctly classifies all five of Ai_reasoning's own canonical examples, used verbatim as tests). 11/11 tests.
- [x] `planner`: `Plan`/`PlanStep` with a real dependency graph, cycle detection + topological execution order via Kahn's algorithm, `PlanError` distinguishing unknown-dependency from cycle. `Planner::create_plan` is honest about scope: Direct/MemoryLookup/SingleTool requests get one real step; MultiStep/LongRunning requests get a single step that says decomposition needs Phase 4 or Phase 7, rather than fabricating a breakdown (Ai_reasoning SS10 "Never invent tool results" applies to plan steps too). 11/11 tests.
- [x] `assistant-core` now classifies every message and folds the result into the system prompt actually sent to the generator -- proven by a test that captures the generated prompt and asserts the detected intent/level appear in it, not just logged and ignored.
- [x] 77/77 tests passing across all 16 non-Slint, non-Candle crates.
- [ ] **Not done, intentionally:** `process_message` still doesn't *branch* on the plan (e.g. actually calling a tool for `SingleTool` requests) -- there's no Tool Framework (Phase 7) yet for it to call. Classification reaching the prompt is real progress; acting on it is Phase 7's job.
- [ ] **Not done:** the "Context Optimizer" module (pulling relevant long-term memories into context, per Ai_reasoning SS8) -- `assistant-core` deliberately doesn't depend on `memory` (Phase 5), keeping session persistence as an `app`-level concern. Wiring memory search into context selection is a real, well-motivated follow-up, better done deliberately (with a concrete caller to validate it) than bolted on speculatively here.
- [ ] **Not done:** "Reflection" as its own subsystem (retrying failed tool calls, suggesting alternatives) -- there's nothing real yet to reflect *on* without Phase 7's tool execution. `Plan::validate()` is the honest Phase 6 subset: structural verification of the plan itself.

## Phase 7 checklist

- [x] `schema`: `InputSchema`/`ParamSchema`/`ParamKind`, validating `serde_json::Value` arguments -- missing-required, wrong-type, and unexpected-parameter all detected, and *all* problems reported at once rather than just the first (TCLIS SS7). 7/7 tests.
- [x] `registry`: `ToolRegistry` -- register/unregister/get/list, refusing to silently shadow a duplicate tool name. 5/5 tests.
- [x] `router`: `ToolRouter::execute` runs the real TCLIS SS3 pipeline -- lookup, validate, permission check (against `common::PermissionLevel`, built all the way back in Phase 1), execute, structured `ToolResult` (TCLIS SS8). Every outcome (unavailable/invalid/denied/timed-out/failed/ok) is data, not a Rust error, matching TCLIS SS10. 6/6 tests, including one that proves timeout enforcement is real: a stub tool sleeps 500ms against a 50ms declared timeout, and the test asserts it comes back `TimedOut` in well under 500ms -- not just that a timeout field exists.
- [x] `stubs`: `EchoTool`, `SlowTool`, `DangerousStubTool` -- prove the happy path, timeout enforcement, and permission denial respectively, the same honest-stand-in pattern as `EchoResponseGenerator` (Phase 3) and `KeywordClassifier` (Phase 6). Not real Linux tools.
- [x] 97/97 tests passing across all 17 non-Slint, non-Candle crates.
- [ ] **Not done, intentionally: wiring into `assistant-core`.** `ToolRouter::execute` is async (needs real timeout enforcement via `tokio::time::timeout`); `assistant-core::process_message` is sync. Bridging them well needs either a real way to extract concrete tool calls (name *and* arguments) from a user request -- which needs a working `llm-runtime` (Phase 4) -- or real tools worth calling (Phase 8). Wiring it up now against stub tools would mean guessing at a design better made once there's something real on the other end, rather than genuine Phase 7 progress.
- [ ] **Not done:** the real Permission Manager (Phase 9) -- `max_auto_level` is a single configured threshold, not the interactive confirmation-dialog flow PRD 6.6 and Archd SS7 describe. It's an honest, working stand-in, not the real thing.

## Phase 8 checklist

This phase splits honestly into three groups, based on what this
headless container actually has running -- not a compile-time
limitation like Phases 2/4, but a genuine runtime difference between a
container and a desktop session:

**Fully verified with real success paths** (no new dependencies -- `/proc`,
`/sys`, and shelling out to standard, always-present Linux utilities):
- [x] `files`: list/read/write/delete (confirm-gated)/rename/move/copy/metadata/search. 10/10 tests.
- [x] `system`: CPU/memory/uptime/kernel/load average via `/proc`, disk usage via `df`. 7/7 tests.
- [x] `processes`: list/info via `/proc/[pid]`, `kill` via the `kill` command -- tested only ever against a process the test itself spawned. 4/4 tests.
- [x] `terminal`: a **fixed allowlist** of commands (`date`, `uptime`, `whoami`, `uname`, `hostname`, `echo`, `sleep`), args passed as argv (never through a shell -- confirmed by a test with shell metacharacters in an argument), genuine timeout enforcement via process kill. A test explicitly confirms `rm -rf /` is rejected before ever spawning. 5/5 tests.
- [x] `packages`: list (`dpkg-query`) and search (`apt-cache`) tested for real; `install`/`remove` implemented but deliberately never invoked by the test suite (slow, network-dependent, and would leave the sandbox in a different state -- not something a test should do regardless of environment).
- [x] `network`: interface listing via `/sys/class/net` + `ip addr`. **Caught a real gap during this phase:** this container didn't have `iproute2` installed, so `ip` was missing; fixed by installing it *and* making the code degrade to interface-names-only (rather than failing outright) if `ip` is ever unavailable, since a real system shouldn't hard-fail on a missing optional dependency either. 3/3 tests.

**Real code, tested for graceful failure** (all need a desktop session this container doesn't have -- the same code succeeds on a real desktop):
- [x] `clipboard` (`xclip`), `notifications` (`notify-send`), `audio` (`pactl`) -- no display/D-Bus/sound-server here. 2 tests each.
- [x] `display` (backlight brightness via `/sys/class/backlight`) -- a kernel interface, not display-server-dependent, but this container has no physical display at all. 2/2 tests.
- [x] `applications` (`.desktop` file discovery, XDG spec) -- tested against a fixture file this crate writes itself, since the sandbox has no real desktop entries to rely on; `launch` strips XDG field codes and is tested with both a nonexistent program and one needing no display (`true`). 6/6 tests.

**Implemented, never executed even in its own tests:**
- [x] `power` (lock/suspend/hibernate/restart/shutdown via `systemctl`/`loginctl`). Tests check only that the right command gets *constructed* (`PowerAction::Shutdown.command() == ("systemctl", ["poweroff"])`) and that only `Lock` is below Administrator permission. Automatically shutting down whatever machine runs `cargo test` -- this sandbox included -- is never a defensible thing for a test to do. 3/3 tests.

**Tool Framework integration:**
- [x] `tools`: wires 8 read-only/safe capabilities (`system_info`, `disk_usage`, `list_directory`, `read_file`, `list_processes`, `run_terminal_command`, `list_packages`, `search_packages`) as real `tool_framework::Tool` implementations -- Phase 7's registry/router/validation pipeline now routes to genuine Linux operations instead of `EchoTool`/`SlowTool`/`DangerousStubTool`. A test confirms `run_terminal_command` rejects `rm -rf /` through the *full* pipeline (validate -> permission check -> execute), not just at the `terminal` module level directly. 6/6 tests.
- [x] `tool-framework::ParamKind` gained `StringArray` (needed for terminal command arguments) -- a small, well-justified extension of an already-verified Phase 7 crate, re-verified after the change (21/21).
- [ ] **Not wrapped as registered tools, intentionally:** `delete_file`, `kill_process`, `install`/`remove` packages, and all of `power` -- these are destructive/administrative (TCLIS SS6 Level 3-4) and deserve the real Permission Manager's confirmation flow (Phase 9) before being reachable through the automated pipeline, not just the router's single `max_auto_level` threshold.
- [ ] **Not done:** wiring any of this into `assistant-core`/`app`'s live message flow -- same reasoning as Phase 7's checklist: needs real tool-call extraction (Phase 4's `llm-runtime`) to decide *when* to call a tool from a natural-language request, which remains blocked on this sandbox's toolchain.

151/151 tests passing across all 19 non-Slint, non-Candle crates.

## Phase 9 checklist

- [x] `rules`: `SecurityRules`/`PermissionDecision` -- pure policy logic. Administrator actions denied by default (PRD SS6.6: "Root-level actions must be blocked by default"); explicitly enabling them still routes through confirmation, it doesn't skip it. 5/5 tests.
- [x] `confirmation`: `ConfirmationHandler` trait + `AlwaysDeny` (the safe default) / `AlwaysApproveForTestingOnly` (deliberately named so it can't be reached for by accident). A real interactive dialog needs Phase 2's UI, which has its own unresolved toolchain constraint -- same honest gap, not new.
- [x] `PermissionManager`: the real `check_and_log` pipeline (remembered decision -> security policy -> confirmation if needed -> always audited), SQLite-backed (same `rusqlite` 0.31.0 pin as Phase 5's `memory`, for the same reason). Remembered decisions support expiry (DBDMS SS3 "Permissions" table's `expires` field) and revocation. 16/16 tests, including one real bug this phase's tests caught: the audit log's `message` never actually included the tool name (it went into a separate `module` field instead) -- fixed so `module` consistently identifies the logging component and the tool name is in the message text, which is what you'd actually want to search an audit log for.
- [x] `tool-framework` gained `PermissionChecker`, a pluggable trait `ToolRouter` now checks first, falling back to the old `max_auto_level` threshold when none is configured -- fully backward compatible, all 21 prior tests plus 1 new one still pass.
- [x] `linux-services` registers `DeleteFileTool` (`Destructive` level) -- deferred in Phase 8 specifically pending a real confirmation flow, which now exists.
- [x] **Full-stack integration test** (`crates/linux-services/tests/permission_integration.rs`): real linux-services tools, running through tool-framework's real router, gated by a real `PermissionManager` -- not stubs standing in for each other. Confirms: read-only tools skip confirmation entirely; a denied confirmation leaves the file untouched; an approved confirmation actually deletes it; Administrator-level actions are denied by policy before confirmation is ever asked; a remembered approval persists across repeated calls without re-asking. 5/5 tests.
- [x] 175/175 tests passing across all 21 non-Slint, non-Candle crates.
- [ ] **Not done:** a real interactive confirmation UI -- needs Phase 2's Slint UI, which has its own unresolved sandbox toolchain constraint. `AlwaysDeny`/`AlwaysApproveForTestingOnly` are honest stand-ins, not a substitute.
- [ ] **Not done:** wiring any of this into `assistant-core`/`app`'s live message flow -- the same reasoning as Phases 7 and 8: deciding *when* to call a tool (and with what arguments) from a natural-language request needs real tool-call extraction, which needs a working `llm-runtime` (Phase 4), still blocked by this sandbox's toolchain. What Phase 9 closes is the confirmation/audit gap *within* the tool pipeline itself, which is now genuinely complete and tested end to end.

## Phase 10 checklist

- [x] `workflow`: `Workflow`/`WorkflowStep`, referencing real tool names + `tool_framework::ToolArgs` -- deliberately different from Phase 6's `planner::Plan`, which is ephemeral and honestly non-specific about tools; a `Workflow` is named, explicit, and meant to be run repeatedly. Dependency-graph validation (reused cycle-detection approach from Phase 6) plus a new check: a step's conditional branch must be backed by a real `depends_on` entry, or nothing guarantees the referenced step has actually run yet. `parallel_stages()` groups steps with no ordering relationship into the same stage (Ai_reasoning SS14 "parallel tasks where safe"). 7/7 tests.
- [x] `control`: `RunControl` -- genuine, tested pause/resume/cancel (Ai_reasoning SS14), checked between steps rather than fabricating mid-step interruption `tool_framework::Tool` was never designed to support. One test spawns execution in the background, confirms it's genuinely still blocked mid-pause (not just returning immediately), then resumes it and confirms it completes. 4/4 tests.
- [x] `executor`: `run_workflow` -- runs a `Workflow` through `tool-framework`'s real `ToolRouter` (so Phase 9's Permission Manager gates workflow steps exactly like any other tool call), respects `StepCondition`, and checks `RunControl` between every step. Stages run in order; steps *within* a stage run sequentially rather than concurrently -- an honest, documented simplification (see the module doc comment) rather than a missing feature, since nothing in this codebase yet needs true concurrent tool dispatch. 6/6 tests.
- [x] `scheduler`: `Scheduler`/`Schedule` -- a pure function of "what's due now", not a background timer; easy to test, and it's up to whatever embeds this to poll `due()`. **Caught a real bug this phase:** a recurring schedule's first run happens after one interval elapses (the cron/`setInterval` convention), not immediately -- the test originally assumed the opposite and failed; fixed the test (the implementation's delayed-first-fire behavior was correct) and documented the semantic explicitly on `schedule()` itself. 5/5 tests.
- [x] `store`: `WorkflowStore`, SQLite-backed (same `rusqlite` 0.31.0 pin as Phases 5 and 9), persisting workflow definitions (steps serialized as JSON, matching DBDMS's single `steps` field) and run history. 5/5 tests.
- [x] 203/203 tests passing across all 23 non-Slint, non-Candle crates.
- [ ] **Not done:** true concurrent execution of same-stage steps -- `parallel_stages()` correctly identifies *which* steps could run concurrently; the executor doesn't yet dispatch them that way, since doing so needs either a new dependency or `Arc`-wrapping `ToolRouter` for a capability nothing here currently needs (every registered tool completes well under a second).
- [ ] **Not done:** wiring the scheduler into a real always-running loop, or workflows into `assistant-core`/`app`'s live message flow -- same reasoning as Phases 7-9: there's no natural trigger point yet without either a working `llm-runtime` (Phase 4) to decide when a request should kick off a workflow, or a long-running `app` process to host the scheduler's poll loop (which the current `app` only runs while its window is open, itself blocked on Phase 2's UI toolchain constraint).

## Phase 11 checklist

This phase raised a real architectural question up front: Plugins.md
SS4 describes a `plugin.so` file, implying genuine dynamic loading --
but Rust has no stable ABI across compiler versions (or even different
dependency graphs of the same version), which makes passing Rust trait
objects across a `dlopen`ed boundary a real, well-known safety hazard,
not a solved problem. Rather than either skip dynamic loading entirely
or paper over that risk, this phase built **two honestly-scoped
mechanisms**:

- [x] `plugin`: the `Plugin` trait (Plugins.md SS8: `initialize`/`shutdown`/`metadata`/`health_check`/`register_tools`) -- the **primary** mechanism. A plugin is in-process Rust code handed to `PluginManager::register` directly. Its tools go through `tool_framework::Tool` -- the exact same interface, and later the exact same registry/router/permission pipeline, as built-in tools. No separate, weaker path for plugin-contributed capabilities.
- [x] `dynamic`: **real** `dlopen`-based loading via `libloading` (pinned to 0.8.9; 0.9+ needs rustc 1.88), across a plain C ABI (`extern "C"` functions, C strings, freed by the plugin's own allocator) rather than Rust types. A real `example-plugin` crate (`crate-type = ["cdylib"]`) is compiled to an actual `.so` and loaded by the test suite -- this is genuinely testable in this sandbox, unlike Phase 2/4's Slint/Candle gaps, since dynamic linking is a Linux capability, not a toolchain or model-hosting limitation. 4/4 tests passing, including loading a real compiled plugin twice independently and confirming a plain `libc.so.6` (present on any Linux system) fails cleanly on the missing-symbol lookup rather than crashing. Deliberately narrow (name/version only) -- extending it to carry richer data needs a deliberately-designed `#[repr(C)]` vtable, real future work rather than something to improvise here.
- [x] `manifest`: `PluginManifest` (TOML, Plugins.md SS5), permission string parsing (unrecognized permissions are dropped, not fatal -- forward compatible), platform support checking, and `major.minor.patch` SDK version compatibility (SS15). 6/6 tests.
- [x] `lifecycle`: `PluginState` (Discovered -> Validated -> Active -> Deactivated, or Failed at any point) -- collapsed from Plugins.md SS6's full diagram to the states that are actual decision points.
- [x] `policy`: `PluginPermissionPolicy` -- a simpler, capability-based cousin of Phase 9's tool-level `PermissionManager`, gating whether a plugin may activate at all (Plugins.md SS13: users can review/grant/revoke/disable at any time). 5/5 tests.
- [x] `manager`: `PluginManager` ties it together -- discovery (scanning for `manifest.toml`, mirroring Phase 8's `.desktop`-file discovery pattern), validation, activation/deactivation with `catch_unwind`-isolated failures (Plugins.md SS16: "Plugin failures must not crash the application" -- tested with a plugin that deliberately fails `initialize()`), and `collect_tools()` for pulling active plugins' tools into the host's real `ToolRegistry`. 9/9 tests.
- [x] 227/227 tests passing across all 25 non-Slint, non-Candle crates.
- [ ] **Not done:** a `#[repr(C)]` vtable letting a genuinely dynamically-loaded plugin contribute real tools (not just report its name/version) -- real, valuable future work, deliberately not improvised inline given how easy an ABI mistake is to get subtly wrong.
- [ ] **Not done:** SQLite persistence for installed-plugin state (DBDMS SS3 "Plugins" table) -- `PluginManager` is fully in-memory this phase. A reasonable enhancement, not included here so the unsafe FFI work got full attention rather than being diluted alongside another SQLite store in the same phase.
- [ ] **Not done:** wiring plugin-contributed tools into `assistant-core`/`app`'s live pipeline -- same reasoning as Phases 7-10.

## Phase 12 checklist

Unlike Phase 4's LLM, nothing in this phase needed a model download from
a host this sandbox can't reach: both `espeak-ng` (TTS) and
`pocketsphinx-en-us` (STT) are classical, non-neural tools that ship
complete, usable models via plain `apt install`. That made *mechanical*
verification straightforward; the honest caveat is *accuracy*, checked
empirically before writing a single test (see below), not assumed.

- [x] `wav`: pure-Rust, dependency-free WAV read/write, chunk-scanning (not assuming a fixed chunk order, since real files -- including `sox`/`espeak-ng` output -- can have extra chunks between `fmt ` and `data`). 5/5 tests.
- [x] `tts`: `EspeakTts::synthesize_to_wav` -- real, reliable speech synthesis, verified by reading back real audio data and confirming longer text produces longer audio. `speak()`/`play_wav` additionally try live playback via `aplay`, which fails gracefully here (no sound server, same honest gap as `linux_services::audio` from Phase 8) and would succeed on a real desktop. 4/4 tests.
- [x] `stt`: `PocketsphinxStt::transcribe` -- a genuinely real pipeline (real acoustic model, real audio samples, real output), but manually verified *before* writing tests that its accuracy on this sandbox's only available test audio (`espeak-ng`-synthesized speech) is poor -- "check the weather" transcribed as "you cannot wear lots". That's an expected property of feeding robotic, formant-synthesized speech into a classical acoustic model trained on natural human speech, not a bug in the integration; real microphone input would perform meaningfully better against the identical model. Tests check what's actually reliable instead: the pipeline produces *some* output for real speech and *no* output for silence, not exact transcription accuracy that wouldn't hold up even on a correctly-working system. 3/3 tests.
- [x] `vad`: `VoiceActivityDetector` -- energy-based voice activity detection, honestly scoped short of true wake-word phrase recognition (Ai_reasoning-style "did they say *this specific phrase*" needs a trained keyword-spotting model, the same model-hosting constraint as Phase 4). Real, reliable, fully tested: silence vs. tone discrimination, frame-level onset detection, configurable sensitivity. 8/8 tests.
- [x] 247/247 tests passing across all 27 non-Slint, non-Candle crates.
- [ ] **Not done:** true wake-word phrase recognition, live microphone capture, or live continuous listening -- all need either a trained model (blocked the same way as Phase 4) or real audio hardware (blocked the same way as Phase 8's audio/clipboard/notifications/display). `Apis SS14`'s `start_listening`/`stop_listening`/`interrupt` describe a live streaming interaction model this sandbox cannot exercise; the file-based `synthesize_to_wav`/`transcribe` equivalents are what's real and tested here.

### A sandbox note worth being transparent about

Partway through this phase, the sandbox's underlying filesystem reset --
losing everything in `/home/claude`, including the entire project and
even the apt-installed Rust toolchain from Phase 1, down to just the
`voice` crate files already in progress at that moment. This matches
this environment's own documented behavior ("file system resets between
tasks"); it's not something Phase 12's code did. Recovery was
straightforward specifically *because* every phase's final state was
packaged into a tarball and shared via `present_files` along the way,
which lives somewhere more persistent than the sandbox's live
filesystem: extracting the last saved archive (Phase 11) restored the
complete project, full git history included, and a full re-verify
(`cargo test`, every crate) came back 227/227 -- an exact match for the
state before the reset. The Rust toolchain and the voice-related `apt`
packages were reinstalled the same way as the first time, and produced
identical versions. Worth keeping in mind for anyone continuing this
project in a similarly ephemeral environment: save checkpoints often.

## Phase 13 checklist

Four focused providers behind one aggregator, all wired up as real tools
-- the first tenant of `ToolCategory::Search`, declared in Phase 7 and
unused ever since.

- [x] `ranking::score_text` -- a small, deterministic, dependency-free relevance scorer (exact match > prefix > substring > partial multi-word > in-order subsequence fallback > no match) shared by file and command search, so `SearchEngine` has a consistent scale to merge otherwise hard-to-compare results without pulling in a fuzzy-matching crate (DIG SS11). 9/9 tests, including hand-traced edge cases for the subsequence fallback.
- [x] `file_search::FileSearchProvider` -- thin wrapper over Phase 8's real `linux_services::files::search`, adding multi-root aggregation and ranking; that function already does the real recursive, depth-capped filesystem work. 6/6 tests.
- [x] `memory_search::MemorySearchProvider` -- wraps Phase 5's real `memory::MemoryStore::search_memory` (genuine semantic search, not reimplemented). `MemoryStore` is `Send` but not `Sync` (same reason `permission-manager`'s `Connection` needed one), so this crate wraps it in `Arc<Mutex<_>>` itself rather than changing Phase 5's code. Scores are honestly rank-derived, not `memory`'s real similarity numbers -- those aren't part of its public API. 4/4 tests.
- [x] `command_search::CommandSearchProvider` -- indexes Phase 7's registered `Tool`s and Phase 10's saved, *enabled* `Workflow`s into a snapshot rebuilt via `reindex()`, rather than querying either store live (neither is thread-safe for arbitrary sharing, and both change far less often than memories do). 7/7 tests, including reindex picking up a newly-registered tool.
- [x] `engine::SearchEngine` -- the "Global Search" module: queries all three providers and merges into one ranked list, deterministic tie-breaks. Deliberately does *not* add a fourth "search settings" provider (TCLIS SS4) -- there's no real settings UI yet to index, just a spec (PRD SS6.7); a small, additive change once Phase 2's UI (and this sandbox's toolchain gap) has one. 4/4 tests.
- [x] `tools` -- `search_files`, `search_memory`, `search_commands`, `global_search`, all `PermissionLevel::ReadOnly`, all `ToolCategory::Search`, registered via `register_all` following `linux_services::tools`'s exact pattern. 6/6 unit tests + a `permission_integration.rs` proving all four run cleanly through a real `PermissionManager` configured to deny everything above ReadOnly (they're never asked), and that calls are genuinely audit-logged.
- [x] 286/286 tests passing across all 24 non-Slint, non-Candle crates.
- [ ] **Not done:** wiring into `assistant-core`/`app`'s live message flow, and command search re-indexing automatically on tool/plugin/workflow changes (would need Archd SS9's event bus wired up as the trigger) -- both deferred for the same reasons Phases 7-12 already documented: `app` still can't be compile-verified in this sandbox (Phase 2's UI), and there's no `llm-runtime` yet to turn a user's request into a concrete tool call in the first place (Phase 4). Settings search is also not done, for the reason above.

### Two things worth being transparent about

This container started completely fresh (no filesystem reset mid-phase
this time, just a new sandbox for a new session) -- `rustc`/`cargo`
weren't installed at all, and `rustfmt`/clippy, `iproute2` (Phase 8's
`network` tests), and `espeak-ng`/`pocketsphinx`/`sox` (Phase 12's
`voice` tests) were all missing too. Reinstalling exactly what
`docs/ROADMAP.md`'s Phase 8 and 12 notes already say this environment
needs brought every one of the 247 previously-documented tests back to
green with zero source changes -- good evidence the restored code itself
was never the problem, just a clean container needing the same setup
every previous phase already needed.

Running `cargo clippy --workspace --all-targets -- -D warnings` (the
exact command `.github/workflows/ci.yml` runs) turned up two pre-existing
findings unrelated to this phase's own code, both left exactly as found
except one trivial fix:

- `memory`'s own test helper used `Box::new(HashingEmbedder::default())`, which `clippy::box_default` flags in favor of `Box::<HashingEmbedder>::default()`. Fixed there (and used the shorter form in every new test in this phase) -- a one-line, zero-risk, purely stylistic change, re-verified 9/9 `memory` tests still pass.
- `example-plugin`'s `free_string` (the `dynamic`/FFI loading path from Phase 11) trips `clippy::not_unsafe_ptr_arg_deref` -- a public function dereferences a raw pointer (`CString::from_raw`) without being marked `unsafe`. This is a deny-by-default lint touching a C-ABI boundary this phase has no context to safely change; **left as-is** and flagged here for whoever picks up Phase 11-adjacent work next, rather than risking a rushed fix to raw-pointer/FFI code.

## Phase 14 checklist

Two crates: `network` (the HTTP client and the building blocks
underneath it) and `updater` (the one concrete, narrow thing built on
top of it this phase).

- [x] `network::HttpClient` -- synchronous by design (see the crate's doc comment for why that actually matches the rest of this workspace better than an async client would), real `GET`/`POST`+JSON/streaming `download`, backed by `ureq`. 8/8 tests, including a genuine transport-failure case (connection refused) and a response-size-limit case that confirms a partially-written file gets cleaned up, not left behind.
- [x] `network::testing::MockServer` -- a ~50-line, std-only, one-request HTTP/1.1 mock server, so every test above (and every `updater` test) runs against real loopback sockets, never the real internet. Exposed as a normal public module, not `#[cfg(test)]`-gated, specifically so `updater`'s tests can use it too.
- [x] The OS's own certificate trust store, not `rustls`'s bundled list -- via `rustls-native-certs`. This turned out to matter for real, not just as a theoretical "more correct for a desktop app" choice: `ureq`'s default TLS setup failed real HTTPS requests in this exact sandbox with `InvalidCertificate(UnknownIssuer)`, while `curl` against the same host worked immediately (`curl` reads the system store; this environment sets `SSL_CERT_FILE` for exactly that reason). See `client::system_tls_config`'s doc comment for the full story.
- [x] `updater::check_for_update` / `updater::download_update` -- version comparison via `semver` (not string comparison -- one of the new tests exists specifically to prove `"1.9.0"` correctly loses to `"1.10.0"`, which a naive string compare gets backwards), against a small original JSON manifest format (nothing in the existing docs specified one). A missing manifest (404) is "nothing to report," not an error; a malformed one, or a genuinely unreachable server, is. 9/9 tests.
- [x] `check_for_updates` (`ReadOnly`) / `download_update` (`Moderate`) -- real `ToolCategory::PackageManagement` tools (the closest existing category; TCLIS SS4 lists "Update packages" under it, and neither `ToolCategory::Network` -- already Phase 8's *local network status* category -- nor `ToolCategory::System` was as good a fit). **Neither tool accepts a URL as input** -- the manifest URL is fixed at construction time by whatever composes the registry, never something a tool call can redirect; see `tools`' module doc comment. 6/6 unit tests + a `permission_integration.rs` proving `check_for_updates` needs no confirmation and `download_update` genuinely does (denied under `AlwaysDeny`, proceeds under `AlwaysApproveForTestingOnly`) against a real `PermissionManager`, not just the router's simple threshold fallback.
- [x] 312/312 tests passing across all 26 non-Slint, non-Candle crates.
- [ ] **Not done, deliberately:**
  - **No generic "fetch any URL" tool.** TCLIS SS4's categories are all about local system control; an open-ended web-fetch capability reachable from natural language is a materially broader security surface than anything else in this workspace, and nothing in the specs actually asks for it. `network::HttpClient` is there as a library for the next concrete, well-scoped feature that needs it -- not exposed wholesale.
  - **No auto-install / self-replacing-binary update flow.** `download_update` stops at "the file is on disk." Replacing a running binary safely, atomically, and consistently across deb/rpm/AppImage/Flatpak/Snap (Phase 20) is a different, much riskier problem that a Networking phase shouldn't decide unilaterally -- see `download`'s module doc comment.
  - **No concrete "optional cloud feature."** This phase built the enabling infrastructure (PRD SS3 rules out a *required* cloud dependency, not an opt-in one); a specific feature on top is future work, matching MMS SS19's own framing of cloud model providers as future, user-configurable, and optional.
  - Wiring into `assistant-core`/`app`'s live message flow -- same reason as every phase since 7: `app` still can't be compile-verified here (Phase 2's UI), and there's no `llm-runtime` yet to turn a request into a concrete tool call in the first place (Phase 4).

### Finding a working HTTP stack on this sandbox's rustc 1.75 took real trial and error

Worth documenting in full, since it's a different *kind* of gap than
Phases 2 and 4's: those need a genuinely newer compiler (Slint 1.77+,
Candle's `zip` dependency 1.88+) and nothing short of that helps. This
one turned out to be fully solvable *without* a newer rustc -- but not
on the first, second, or third try:

1. `reqwest` (both the 0.12 and 0.11 lines, both TLS backends) kept failing to even download its dependency tree: the *latest* releases of several unrelated transitive dependencies (`cpufeatures`, `zeroize`, `indexmap`, `idna_adapter`) have each independently adopted Cargo's `edition2024` resolver, which needs a much newer Cargo than 1.75 ships -- not a version *this* workspace controls directly.
2. Switched to `ureq` (a much smaller dependency tree) -- hit the exact same wall via `zeroize`, then `idna_adapter`.
3. Pinning those two specific crates to slightly older versions (`zeroize =1.7.0`, `idna_adapter =1.1.0`, both otherwise-unremarkable point releases) was enough: the whole tree resolved and **compiled**.
4. Compiling isn't the same as working: a real HTTPS request against crates.io failed TLS certificate validation (`UnknownIssuer`), even though `curl` against the same host succeeded immediately. That gap -- not the compiler -- was the real second problem, and pointed at `rustls`'s bundled root list rather than the sandbox's networking itself.
5. `native-tls` (OpenSSL) was the obvious next thing to try, since `curl` uses it successfully here -- but `openssl-sys` itself declares a hard `rustc 1.80+` requirement, a real language-level floor no version pin can work around (the same category of wall as Slint/Candle, just discovered one layer deeper).
6. `rustls-native-certs`, feeding the *system's* trust store into the already-working `rustls` setup from step 3, was the actual fix -- no OpenSSL needed, no newer rustc needed. A real `GET` against `crates.io`'s API, parsed as JSON, worked end to end.

Four pins total (`zeroize`, `idna_adapter`, both in `network`'s
`Cargo.toml` with comments explaining why) is the entire cost of this
gap. Unlike Phases 2 and 4, there is nothing left blocking this one on a
newer rustc -- it's already fully resolved.

## Phase 15 checklist

Three crates, one per concern, tied together by one report.

- [x] `metrics::MetricsRegistry` -- thread-safe counters/gauges/timers behind `RwLock`s, `BTreeMap`-ordered snapshots for deterministic output. 11/11 tests, including a genuine 8-thread concurrent-increment stress test (all 800 increments land, none lost to a race).
- [x] `metrics::PerformanceMonitor` -- samples real system state via Phase 8's already-verified `linux_services::system::system_info` (no `/proc` parsing duplicated here), records gauges, keeps a bounded rolling history. `run_periodic_sampling` exists as a ready-to-use async helper but isn't spawned anywhere in this crate -- PRD SS3's "no unsafe hidden background behavior" means nothing here starts a background task on its own; that's `app`'s call to make, later.
- [x] `telemetry::TelemetryEventKind` / `TelemetryClient` -- see the crate's own doc comment before touching it. Two independent, explicit gates (`enabled`, `endpoint`) both must be true before any network call happens; `record()` always no-ops when disabled rather than requiring call sites to check first; the event vocabulary is a closed enum so a caller cannot construct an arbitrary/identifying event name even by mistake. 11/11 tests, including two that point `flush()` at an address nothing is listening on and confirm it returns `Ok(0)` instead of erroring -- proof the gates are checked *before* any connection is attempted, not just documented as if they were.
- [x] `diagnostics::install_panic_hook` -- a global panic hook writing a small local JSON crash report (message, source location, a force-captured backtrace regardless of `RUST_BACKTRACE`) for any panic on any thread, then still calling whatever hook ran before it, so normal panic output is unaffected. Crash-report construction (`from_panic_info`) and file writing are tested as plain functions, independent of the global hook, specifically to avoid the test-isolation hazard of mutating process-wide panic state from parallel tests; one additional test does exercise the real `catch_unwind` -> hook -> file path, with a documented note on why it's the only test in the crate allowed to do that. 8/8 tests.
- [x] `diagnostics::build_report` / `get_diagnostics_report` -- the phase's actual deliverable ("Debug tools"): one real, `ReadOnly`, `ToolCategory::System` tool reporting a metrics snapshot, crash report count, and telemetry's enabled-status (never its buffered *content* -- `report.rs` only ever calls `telemetry.is_enabled()`/`buffered_count()`, nothing that could leak what's actually queued).
- [x] 342/342 tests passing across all 29 non-Slint, non-Candle crates.
- [ ] **Not done, deliberately:**
  - **No concrete telemetry endpoint or default-on anywhere.** This phase built the (thoroughly gated) machinery; turning it on for real is a product decision for later, made explicitly, not something this phase should default toward.
  - **`install_panic_hook` is not called anywhere yet.** Like every tool `register_all`, wiring it into `app`'s actual startup is deferred for the usual reason (`app` still can't be compile-verified here, Phase 2's UI).
  - Wiring into `assistant-core`/`app`'s live message flow generally -- same reason as every phase since 7.

### A note on testing a global panic hook

`std::panic::set_hook` is process-wide state, and Rust's default test
runner executes tests from the same binary in parallel threads. Only one
test in this workspace (`diagnostics::crash::tests::from_panic_info_extracts_a_string_literal_message`)
actually swaps the hook, and it restores the previous one immediately
after; as long as no *other* test in the same binary panics
concurrently, this is safe. If a future phase adds another
deliberately-panicking test to this crate, it's worth knowing this
constraint exists rather than rediscovering it as a flaky test --
serializing such tests (`--test-threads=1`, or a small serialization
crate) would be the fix, not a dependency this phase pulled in for a
single test.

## Phase 16 checklist

Two crates: `security` (Encryption + Secure Storage + Secret Manager --
closely related, all about protecting data at rest) and `sandbox`
(Sandbox -- a genuinely distinct concern, isolating what a process can
do rather than protecting stored data).

- [x] `security::MasterKey` -- generate / derive from a passphrase (Argon2) / load-or-create a `0600` file, `ZeroizeOnDrop`, `Debug` output hard-redacted to `"MasterKey(REDACTED)"` so a stray `{:?}` in a log statement can't leak it. 7/7 tests.
- [x] `security::cipher` -- AES-256-GCM via RustCrypto's `aes-gcm`, not hand-rolled. Tests don't just check the happy path: wrong key, tampered ciphertext, and tampered nonce are all confirmed to fail decryption (AEAD's authentication actually doing its job), and two encryptions of identical plaintext are confirmed to produce different ciphertexts (nonces genuinely not reused). 7/7 tests.
- [x] `security::SecureStore` -- SQLite-backed (same `rusqlite =0.31.0` pattern as `memory`/`permission-manager`/`workflow-engine`), stores only opaque `EncryptedBlob`s, `list_names` never touches ciphertext. 6/6 tests, including real on-disk persistence across reopening.
- [x] `security::SecretManager` -- the plaintext-in/plaintext-out API on top, with a regression test that would fail immediately if it were ever "simplified" into storing values unencrypted. 6/6 tests. **No assistant tool** -- see the crate's own doc comment for why even `list_names` alone wasn't wired up as one.
- [x] `sandbox::ProcessLimits`/`apply_limits` -- CPU time, memory, open files, process count, max file size via POSIX `rlimit`. Not a claim: a real automated test spawns a child told to busy-loop forever with a 1-second CPU limit and confirms it's actually killed within a few real seconds, not left running.
- [x] `sandbox::FilesystemSandbox` -- Landlock-based path restriction, `EnforcementStatus` forcing every caller to check whether the kernel actually enforced the rules rather than assuming success means protection. Builder logic is unit-tested; the live `restrict_current_process()` call is deliberately *not* exercised in the automated suite (process-wide, irreversible -- see below) and instead has a standalone `examples/filesystem_sandbox_demo.rs`, run manually once during this phase and confirmed to behave exactly as documented.
- [x] 377/377 tests passing across all 31 non-Slint, non-Candle crates.
- [ ] **Not done, deliberately:**
  - **No OS keyring integration** (`libsecret`/Secret Service) for `MasterKey`. This sandbox has no session bus to verify it against, the same category of "can't confirm it actually works here" as Landlock's enforcement -- but unlike Landlock, integrating it at all was skipped rather than written-but-unverified, since a broken keyring integration masquerading as working would be worse than the honestly-limited file-backed scheme this phase actually shipped.
  - **No tools in either crate.** Deliberate, not an oversight -- see both crates' doc comments.
  - **Not wired into `linux_services::terminal`.** `apply_limits` is a ready-to-use standalone function specifically so a *future* revision of that call site can adopt it without this phase reaching backward into Phase 8's already-complete code.
  - **No namespace or seccomp-based sandboxing.** Landlock was the one unprivileged option realistic for this workspace's actual target (a normal user's desktop, not a privileged container runtime); namespaces/seccomp generally need cooperation this phase has no way to assume.

### Landlock: real code, honestly unverifiable enforcement in this sandbox

This is a different *kind* of gap than any other in this project. Phases
2 and 4 are blocked by the *compiler* (Slint needs rustc 1.77+, Candle's
dependency tree needs 1.88+) -- nothing about the *code* is in question,
only whether this specific `rustc` can build it. Phase 14's dependency
saga was resolved entirely with version pins, no compiler change needed.
Landlock is different again: it compiles and runs cleanly on this
sandbox's rustc 1.75, and the kernel here (6.18) is far newer than
Landlock's 5.13 floor -- but calling `restrict_self()` on a real ruleset
and then testing it against a genuinely-not-allowed directory shows the
kernel not actually enforcing anything:

```
ruleset status: NotEnforced
read allowed dir: Ok("hi")
read blocked dir: Ok("shh")   <- should have failed, didn't
```

The most likely explanation is that this sandbox runs inside another
layer of process isolation (Claude's own) that itself blocks the
underlying `landlock_create_ruleset`/`landlock_restrict_self` syscalls --
plausible given how deeply nested this environment already is, and not
something `sandbox` has any way to detect or work around from inside a
Rust process. On a normal, unrestricted Linux desktop -- this
application's actual target -- real enforcement is expected; Landlock
has shipped in mainline distro kernels since well before 5.13 became
common.

This is exactly why `EnforcementStatus` exists as a forced check rather
than `restrict_self()` just returning `AppResult<()>`: a security
control that silently no-ops is worse than one that's honestly absent,
and this module was built to make that distinction impossible to miss.

## Phase 17 checklist

Every one of this phase's four modules (Animations, Performance,
Accessibility, Responsive Improvements) lives entirely inside the Slint
UI layer, with no Slint-independent subset to carve out the way earlier
phases sometimes had. Before writing anything, this was flagged to Lion
directly rather than assumed either way; Lion chose to write it anyway,
unverified, matching Phase 2's original precedent. What follows is that
choice, executed as carefully as it's possible to execute code that
can't be compiled here.

- [x] **Design tokens** (`theme.slint`) -- the exact expansion Phase 2's own comment already promised: full typography scale, shadow levels, complete radius variants, semantic status colors (success/warning/error/info, UI-UX SS3), plus new `Palette.high-contrast`/`text-scale` and a `Motion` global (`reduced`, plus shared duration/easing tokens) for SS13/SS15.
- [x] **Animations** -- `animate` blocks on nav-item background/text color, the sidebar's width (responsive collapse), the header theme toggle, the composer send button, and a focus-ring indicator, all reading `Motion.duration-*` rather than hardcoded values. A `ThinkingIndicator` (SS15 "loading shimmer") pulses via a `Timer` toggling a property, deliberately *not* using the `animate` block's own repeat/direction sub-properties -- see the inline comment where that's decided; this pass wasn't confident enough about their exact form to rely on blind.
- [x] **Accessibility** -- a reusable `Activatable` component (`FocusScope` + `TouchArea`) gives every interactive element real Tab-reachable keyboard focus, Enter/Space activation, and a visible focus ring, instead of each control reimplementing it; `accessible-role`/`accessible-label`/`accessible-checked` added throughout (nav tabs, the composer's input and send button, the theme toggle, each message bubble reading its role and content aloud).
- [x] **Responsive Improvements** -- `AppWindow.narrow` (a width breakpoint) drives the sidebar's collapse; UI-UX SS14's "minimum usable layout maintained" is taken literally -- labels stay, the sidebar just narrows, rather than an icon-only rail this phase has no real icon set to back.
- [x] **Performance** -- the message list now uses Slint's own `ListView` (list virtualization) instead of Phase 2's `ScrollView`+`VerticalLayout`+`for`, directly addressing UI-UX SS18 "efficient rendering of long conversations... lazy loading for large histories." `MessageBubble` was restructured to own its own spacing so it drops into a `ListView` cleanly.
- [x] `config::UiConfig` gained `reduced_motion: bool`, `high_contrast: bool`, `text_scale: f32` (clamped `0.8..=2.0` via `effective_text_scale()`, mirroring `text` MIN/MAX constants) -- **this part is real, compiled, tested Rust**, since `config` has no Slint dependency. 8/8 `config` tests (+5 over Phase 16), including one that loads a config file written *before* this phase existed (missing the new fields entirely) and confirms it still loads with sane defaults.
- [x] `ui::run`'s signature grew a third parameter (`&config::UiConfig`) to seed the new globals at startup, plus a `waiting` property toggled around the send/receive flow to drive the thinking indicator. `ui`'s own dev-runner (`src/main.rs`) was updated to match.
- [x] 382/382 tests passing across every crate that can actually be verified in this sandbox (everything except `ui`/`llm-runtime`/`app`, unchanged in kind since Phase 2).
- [ ] **Known, deliberately-unaddressed follow-on:** `app/src/main.rs` (untouched since Phase 5, for the same "can't compile-verify it" reason as always) still calls `ui::run(bus, session_id)` with the *old* two-argument signature. It won't build against the new one -- but it already couldn't build at all, for the same underlying reason, so this doesn't newly break anything that worked. Worth fixing in the same pass that finally revisits `app.rs` once Slint is unblocked, not before.

### The Slint investigation, done properly this time

Every previous phase's notes repeated the same short line: Slint needs
rustc 1.77+, this sandbox has 1.75. True, but incomplete -- that
framing made it sound like the same *kind* of gap as Phase 14's
dependency pins, just bigger. It isn't. Before writing a single line of
`.slint` for this phase, that assumption got tested properly, the same
way Phase 14's HTTP stack did:

1. `slint = "1"` fails immediately -- `idna_adapter` needs Cargo's
   `edition2024` resolver, the exact class of issue four earlier pins
   (`network`, `security`) already solved elsewhere in this workspace.
2. Applying those same known-good pins doesn't finish the job the way it
   did for `ureq`/`aes-gcm` -- it uncovers another edition2024 wall
   (`indexmap`), then another (`vtable-macro`, one of *Slint's own*
   internal crates, not a generic leaf dependency -- a materially
   different, riskier thing to pin arbitrarily, since its version is
   tightly coupled to whichever Slint release is in use).
3. Trying an older Slint release (`1.3.2`, predating some of this churn)
   gets further -- real progress through `cpp`/`cpp_build` (Slint's C++
   interop layer), `accesskit`/`atspi` (real accessibility plumbing --
   confirming Slint's accessibility story is genuine, not vaporware),
   `glutin`/`gbm-sys` (OpenGL) -- but keeps needing new pins at every
   step, eventually reaching `skia-bindings` (Google's Skia, a large C++
   graphics library with its own build requirements entirely separate
   from any Rust edition concern).

Nine real attempts, each pin uncovering the next requirement, spanning
graphics rendering, platform windowing, accessibility, and C++ interop --
not the two-to-four isolated pins that resolved `network`/`security`.
And critically, even a fully successful resolution wouldn't answer the
original question: whether rustc 1.75 itself accepts Slint's own source,
which is where the original "1.77+" finding came from in the first
place. Dependency resolution was never actually the blocker being
described; it's a second, independent wall in front of the first one.

This is why Lion was asked directly rather than this phase just being
written silently: stacking new, equally-unverified UI code on a
foundation that has never once compiled, on the back of a blocker that
turned out to be substantially deeper than previously documented, is a
meaningfully different bet than continuing the existing "written but
unverified" precedent by default. Lion's answer was to proceed anyway --
so this phase does, with the specific, extra care documented in the
checklist above (an `Activatable` component redesigned mid-writing to
fix a real encapsulation bug caught on review, an animation approach
deliberately chosen over a fancier one because it's easier to get right
blind, string-manipulation avoided entirely in favor of a slightly less
polished but syntactically safer alternative) -- the honest ceiling of
what careful work looks like when the compiler that would normally catch
mistakes isn't available at all.

## Phase 18 checklist

Five modules per Development_plan: Memory Optimization, Startup
Optimization, Async Improvements, Caching, Lazy Loading. One new crate
(`cache`) plus real, tested changes to seven existing ones -- unlike
Phase 17, none of this phase's *substance* touches Slint, so almost all
of it landed as fully compiled, fully tested code rather than
written-but-unverified. Only wiring it into `app` keeps the usual
caveat.

- [x] `cache` (new) -- capacity- and TTL-bounded `Cache<K, V>`, brute-force LRU eviction deliberately (documented against `vector_db::VectorIndex`'s own identical reasoning for its scale), plus `SharedCache<K, V>` (`Arc<Mutex<_>>`, the same sharing idiom `permission-manager`/`search` already established rather than a new one). Zero dependencies, matching `vector-db`/`embeddings`. 24/24 tests: eviction correctness, recency refresh, TTL expiry (real timing, generous margins), stats, and genuine multi-thread concurrency via `std::thread` for `SharedCache`.
- [x] `vector_db::VectorIndex::search` -- replaced a full `sort_by` with `select_nth_unstable_by` partial selection when `top_k < n`, so ranking a handful of results out of a much larger memory store no longer pays for sorting entries it's about to discard. See the stability note below for a real subtlety this surfaced and how it was closed. 11/11 tests, including a 128-entry property-style equivalence check against a naive full-sort reference across many `top_k` values, ties included.
- [x] `embeddings::CachingEmbedder<E>` -- wraps any `Embedder` in a `SharedCache`, memoizing `embed()` by exact text. Verified to produce byte-identical output to the unwrapped embedder, not just "probably the same." 12/12 tests.
- [x] `memory::MemoryStore` -- a bounded `search_memory` result cache, keyed on `(query, top_k)`, cleared on every `remember`/`forget`. The two tests that actually matter here aren't the cache-hit ones -- they're `remembering_a_new_memory_invalidates_stale_cached_search_results` and `forgetting_a_memory_invalidates_stale_cached_search_results`, which fail loudly if invalidation is ever removed or narrowed. New `open_with_search_cache`/`in_memory_with_search_cache` constructors added *alongside* the existing `open`/`in_memory` (unchanged signatures, DIG's "backward-compatible when possible") rather than replacing them. 13/13 tests; downstream `search` crate re-verified unchanged (39/39).
- [x] `assistant_core::LazyResponseGenerator` -- defers constructing the real backend until the first `generate()` call, then keeps it. Tested for zero eager construction, exactly-once factory execution across repeated calls, and -- the test that actually proves it's useful, not just correct in isolation -- as a real drop-in `Box<dyn ResponseGenerator>` inside a genuine `AssistantCore`. 9/9 tests.
- [x] `utilities::run_blocking` -- `tokio::task::spawn_blocking` wrapped with panic-safe `AppError` mapping; the shared version of the bespoke pattern `tool_framework::ToolRouter::execute` has used since Phase 7. Tested including a real concurrency proof (a sibling timer task is confirmed to fire *while* the blocking work sleeps, not after) -- not just that the function returns the right value. 6/6 tests (`utilities` first real dependency addition since Phase 1: `tokio` + `common`).
- [x] `metrics::MetricsRegistry::time`/`MetricsSnapshot::slowest_timers` -- reuses Phase 15's existing timer storage rather than a new startup-specific structure, so a future report gets these for free. 16/16 tests (+5).
- [x] `config::PerformanceConfig` -- `embedding_cache_capacity`/`search_cache_capacity`/`cache_ttl_seconds`, wired into `Config` the identical way Phase 17 added `UiConfig`'s new fields: `#[serde(default)]`, additive, and a backward-compatibility test proving a config file that predates `[performance]` entirely still loads with sane defaults. 12/12 tests (+4).
- [x] `app` (unverified, same reason as ever) -- `load_backend` deferred via `LazyResponseGenerator` instead of blocking startup; the background task's per-turn work (`process_message`, `save_session`) now moves through `utilities::run_blocking` instead of running directly inline in the async task, fixing a genuine blocking-in-async-task issue that predated this phase; `open_memory` now builds a `CachingEmbedder` and calls `open_with_search_cache`, both sized from `config.performance`; a `MetricsRegistry` times the startup phases and logs the slowest ones; `ui::run(bus, session_id)` fixed to the three-argument `ui::run(bus, session_id, &config.ui)` Phase 17 already needed and explicitly deferred fixing until this file was touched again. See the harness note below for how this got real compiler verification anyway.
- [x] 437/437 tests passing across all 32 non-Slint, non-Candle crates (+55 over Phase 17).
- [ ] **Not done, deliberately:**
  - **No new tool this phase.** `security`/`sandbox` (Phase 16) set this precedent for infrastructure-only phases, and the same reasoning applies here for a different honest reason: a `get_performance_report` tool would have meant either inventing something with little new information (Phase 15's `get_diagnostics_report` already surfaces whatever `MetricsRegistry` records -- including this phase's new `startup_*`/`model_load_deferred` timers, automatically, no diagnostics-crate changes needed) or padding the tool surface for its own sake. Cache hit rates and deferred-construction state are implementation details a tool call doesn't obviously need either.
  - **`vector-db`'s optimization is the only algorithmic change.** `memory`'s and `embeddings`' caches reduce *repeated* work; they don't change any single search's own complexity. A real approximate-nearest-neighbor index remains future work if this project's memory counts ever grow past the "hundreds to low-thousands" scale `vector-db`'s own doc comment already targets.
  - **No cache wired into `search`'s command/file providers.** `memory`'s cache was the clearest, most directly-motivated win (`search_memory` re-embeds and re-scans on every call); extending the same treatment to the other two providers is a natural, bounded follow-on, not done here to keep this phase's diff reviewable.

### A stability subtlety in the `vector-db` optimization

`Vec::select_nth_unstable_by` is, as the name says, *unstable*: it's
free to reorder equal elements however it likes while partitioning. The
`sort_by` it replaced is stable. A first draft of this optimization
compared scores alone and passed a 128-entry equivalence test against a
naive full-sort reference right up until the test data included
intentional ties (every 10th vector a deliberate duplicate of an earlier
one) -- at which point tied entries straddling the `top_k` boundary
could come out in a different order than the old, stable-sorted
behavior would have produced, not on every run, but on some.

The fix wasn't to abandon the optimization, it was to make tie-breaking
explicit instead of relying on whichever algorithm happened to be
running: each entry's original position gets tracked alongside its
score, and the comparison function ranks by `(score descending,
original position ascending)`. A stable sort is *equivalent* to sorting
by `(key, original index)` with the index breaking ties -- so ranking on
that same compound key makes an unstable partial-selection algorithm
produce byte-for-byte identical output to the old stable full sort,
for any input, ties included, not just usually the same. The 128-entry
test with intentional duplicates stayed in specifically because it's
the one that would have caught this if it hadn't already been caught
during writing.

### Verifying unverifiable code: a scratch harness for `app`

Every phase since 2 has left `app`/`ui` in "written but not
compile-verified" -- correct as far as careful reading and `cargo fmt
--check`'s syntax validation can confirm, but never actually type-checked
by rustc. This phase's changes to `app/src/main.rs` were involved enough
(a `'static` closure boundary for the lazy backend, a value round-trip
through `spawn_blocking`, new struct-literal construction of
`cache::CacheConfig` from `config::PerformanceConfig` fields) that
"reads correctly" felt like a meaningfully weaker guarantee than usual
for exactly the parts of this phase that mattered most.

`app` itself can't be checked directly -- `ui` needs Slint (rustc 1.77+)
and `load_backend` touches `llm_runtime`, which needs Candle (rustc
1.88+), both pre-existing, both unrelated to what this phase actually
changed. So a throwaway crate outside the workspace (never committed,
deleted immediately after use) depended on the *same* local path crates
this phase touched -- `assistant-core`, `utilities`, `cache`,
`embeddings`, `memory`, `metrics`, `config`, `event-bus`, `common` -- and
reproduced `app/src/main.rs`'s new patterns line-for-line, with
`load_backend` stubbed to sidestep the unrelated Candle wall. It
compiled clean on the first real attempt, and then -- going one step
further than a type-check -- actually ran: publishing a real
`UserMessageReceived` event, driving it through the real background-task
shape (lazy backend construction inside `MetricsRegistry::time`, the
`run_blocking` round trip moving owned `assistant`/`memory_store`
through Tokio's blocking pool and back), and receiving a real
`ResponseGenerated` reply back off the bus within a timeout.

This doesn't make `app` itself verified -- the two Slint/Candle gaps are
exactly as open as they were after Phase 17, and the harness's stub
`load_backend` is not proof that the *real* one (which this phase only
changed the signature of, not the body) still type-checks against
`llm_runtime`'s actual types. But it's real evidence, from a real
compiler, for the specific new patterns this phase introduced, which is
a meaningfully stronger position than the "written carefully, never
checked" ceiling every prior phase's `app` changes had to accept.

## Phase 19 checklist

Six modules per Development_plan: Unit Tests, Integration Tests, UI
Tests, Stress Tests, Security Tests, Benchmarks. By this phase, unit
testing is already thoroughly covered -- 437 tests landed across Phases
1-18, plus three crates (`linux-services`, `search`, `updater`) already
carry their own crate-local `tests/permission_integration.rs`. So this
phase's actual job was finding what *isn't* covered yet: combinations of
three or more crates nothing exercises together, adversarial input and
bypass attempts nobody's tried, and real load rather than a unit test's
handful of entries -- investigated by reading the relevant crates first
(`tool-framework`, `permission-manager`, `plugin-manager`,
`linux-services::files`, `sandbox`, `workflow-engine`), the same
empirical-first approach as every other phase, so what got written is
answering a real gap rather than padding a checklist.

- [x] `integration-tests` (new crate) -- no library code, `[dev-dependencies]` only, three `tests/*.rs` files:
  - `cross_crate_flows.rs` (3 tests): a `plugin-manager`-contributed tool reaching a real `tool_framework::ToolRouter` and executing successfully; that same tool panicking *during `execute()`* (not just plugin lifecycle -- `plugin-manager`'s own tests already cover `initialize()`/`shutdown()` panics via `catch_unwind`) caught cleanly by the router's `spawn_blocking`, not a crash; and `assistant-core` + a real permission-gated tool call + `memory` persistence all cooperating in one scenario, explicitly scoped to *not* claim the still-Candle-blocked automatic LLM-driven tool-call extraction is done.
  - `security_adversarial.rs` (4 tests, TDS SS10's four categories -- plugin isolation lives in the file above since it's as much a cross-crate story as a security one): two honest findings written up as checked assertions, not prose -- calling `Tool::execute()` directly (bypassing `ToolRouter`) skips every safety check there is, and `read_file` reaches any path the OS-level user can read, unconfirmed, because path-scoping is deliberately deferred to `sandbox::FilesystemSandbox` (Phase 16), which nothing in this workspace applies at process startup yet. Plus adversarial robustness: a 4MB string and a 200k-element array handled promptly (via `spawn_blocking` + a real timeout -- see the note below on a mistake this caught in its own first draft), and 500 consecutive denials that never drift into an accidental approval.
  - `stress.rs` (3 tests): `memory::MemoryStore::search_memory` correct and fast at 5,000 entries (an order of magnitude past Phase 18's own 128-entry equivalence test, exercised end-to-end through embedding + index + cache rather than `VectorIndex::search` alone); `cache::SharedCache`'s capacity invariant holding under 32 threads doing 20,000 mixed put/get/remove operations against 500 overlapping keys; `event_bus::EventBus` delivering all 2,000 events to all 50 concurrent subscribers under load.
  - All 10 pass; clippy clean; fmt clean.
- [x] `vector-db`/benches/search.rs and `cache`/benches/cache_ops.rs -- dependency-free, hand-rolled timing harnesses (`cargo bench -p vector-db` / `-p cache`), not `criterion`. See the note below for why. Both produced real, meaningful numbers that empirically confirm each crate's own documented complexity claims (roughly-linear O(n) scoring in `vector-db`'s search regardless of `top_k`, shrinking benefit from partial selection as `top_k` approaches `n`; `cache`'s brute-force eviction scan costing ~15.5µs even at 8,192 entries; `SharedCache`'s `Arc<Mutex<_>>` measurably but modestly slower than bare `Cache` for a single-threaded hit -- the exact multiple is noisy at sub-microsecond scale run to run, not a number worth quoting precisely).
- [x] 447/447 tests passing across all 33 non-Slint, non-Candle crates (+10 over Phase 18).
- [ ] **UI Tests**: not attempted. Slint has never once compiled in this sandbox (Phase 2 on); testing a UI that's never been built, using a testing approach that itself has never been tried against this project's actual `.slint` files, would be speculation on top of speculation. Consistent with every other UI-adjacent gap, this is left honestly open rather than filled with untested guesswork.
- [ ] **Not done, deliberately:** `assistant-core` + tool-framework + permission-manager + memory cooperating (see `cross_crate_flows.rs` above) is explicitly *not* the automatic LLM-driven tool-call wiring every phase since 7 has flagged as blocked on Phase 4/Candle -- it proves the pieces have no interface friction, not that the missing glue exists.

### A self-caught mistake: `tokio::time::timeout` around synchronous code doesn't bound it

`security_adversarial.rs`'s oversized-input test originally wrapped a
call to `InputSchema::validate` -- a plain synchronous function with no
`.await` inside it -- in `tokio::time::timeout(Duration::from_secs(5),
async { schema.validate(&args) })`. That looks like it should work, and
it would have kept passing (`validate` doesn't actually hang on 200,000
elements), which is exactly what makes this worth writing down: a
`timeout` can only interrupt a future at a point where it yields control
back to the runtime, and a synchronous call inside an `async` block with
no `.await` never yields at all -- it runs to completion, however long
that takes, before the timeout gets a chance to fire. If `validate` ever
did hang, this "safety net" would have hung right along with it.

The fix is the same real-timeout pattern `tool_framework::ToolRouter::execute`
already uses for synchronous tool code: run the call on a separate
blocking thread via `tokio::task::spawn_blocking`, and time out waiting
for *that* task to finish, not the call itself. That's a genuine
external bound, since the runtime can walk away from an unfinished
`spawn_blocking` task's `JoinHandle` regardless of what the thread
running it is doing.

### Why `vector-db`/`cache`'s benchmarks don't use `criterion`

`criterion` was the first choice -- it's the standard, and both crates
already had a natural `[[bench]] harness = false` home for it. Adding it
as a dev-dependency and running `cargo check -p vector-db --benches`
failed immediately: `rayon-core ^1.13.0` required, unavailable on this
sandbox's rustc 1.75. Pinning an older `rayon-core` failed too --
`candle-core v0.11.0` (an existing `llm-runtime` dependency, needed
because `app` depends on `llm-runtime`) already locks `rayon` to a
version requiring that same `rayon-core ^1.13.0`, and this workspace's
single shared `Cargo.lock` unifies dependency versions across every
member. Disabling `criterion`'s default features (dropping the `rayon`-based
plotting backend, keeping only `cargo_bench_support`) got past that, but
surfaced the same wall one dependency later: `half`, criterion's float16
serialization dependency for benchmark result caching, is *also* a
`candle-core` dependency, pinned by candle to `^2.5.0` -- and every
version of `half` in the `2.5.x`-`2.7.x` range that satisfies that
constraint turns out to need rustc 1.81+. Three separate pin attempts
(`rayon-core@1.12.1`, `half@2.7.1`→`2.5.0`) confirmed this is the same
class of structural, workspace-wide toolchain wall Phase 2/4 already
documented for Slint/Candle directly, not a fixable version conflict.

Rather than treat that as a blocker to route around, it's worth noticing
it's actually consistent with how this project already builds things:
`vector-db` describes itself as zero-dependency in its own `Cargo.toml`,
`cache` has no dependencies at all, and DIG SS11 says outright to avoid
introducing unrelated dependencies. A benchmark harness that pulls in
criterion's full analysis/plotting/serialization stack for two small,
already-fast operations was always a slightly awkward fit for either
crate's stated design philosophy. The hand-rolled replacement -- warmup
iterations, then timed samples, min/median/mean, printed plainly -- is
maybe forty lines of genuinely well-understood code, needs nothing from
crates.io, and produces numbers exactly as real and useful for tracking
regressions across future phases as criterion's would have been.

## Phase 20 checklist

Five modules per Development_plan: DEB, RPM, AppImage, Flatpak, Snap,
plus a sixth format ("portable") the project's own directory layout
already anticipated (`packaging/portable/`). This phase is a different
kind of work from every phase before it -- there's no Rust to compile or
`cargo test` to run. Every one of these six formats packages a
*compiled binary*, and `ai-assistant` has never once compiled in this
sandbox (Slint since Phase 2, Candle since Phase 4). So the approach
here mirrors Phase 18's throwaway harness: build the real packaging
mechanism -- scripts, manifests, control files -- and verify it for real
wherever the sandbox's own tooling and network allowlist permit, against
a trivial stand-in binary standing in for the still-unbuildable real one,
rather than either skip verification entirely or pretend a stand-in
proves more than it does.

Two genuine, pre-existing gaps surfaced immediately on starting this
phase, both fixed as part of it since packaging is exactly what exposed
them as blocking: no `LICENSE` file existed anywhere in the repository
despite the workspace declaring `license = "MIT OR Apache-2.0"` since
Phase 1 (added `LICENSE-MIT`/`LICENSE-APACHE`, the standard Rust
dual-license convention), and no `assets/` directory existed at all
despite the project's own directory layout anticipating
`assets/icons/` (added a placeholder SVG + rendered PNGs).

- [x] **Shared assets**: `assets/icons/ai-assistant.svg` (placeholder -- explicitly documented as such, pending real design work) rendered to PNGs at 16/32/48/64/128/256/512px via `rsvg-convert`. Two real bugs caught and fixed while building this, neither found by inspection: an XML double-hyphen-in-comment syntax error in the SVG (rejected by `rsvg-convert`, not by ImageMagick's own delegate mechanism, which failed with a much less specific error first), and `render-icons.sh`'s `cd` only climbing one directory level when the script lives two levels deep.
- [x] **`.desktop` entry** (`packaging/shared/ai-assistant.desktop`): real `desktop-file-validate` (installed via apt) caught a genuine issue on the first pass -- `Categories=Utility;Office;` has two main categories, which the spec allows but warns could duplicate the app across menu sections. Fixed to `Categories=Utility;`; validates clean.
- [x] **AppStream metadata** (`packaging/shared/com.lionos.AiAssistant.metainfo.xml`): real `appstreamcli validate` (installed via apt) caught a deprecated `<developer_name>` tag (fixed to the current `<developer>`/`<name>` form) and a hard error -- AppStream requires the metainfo filename to match its own component ID, and `ai-assistant.appdata.xml` didn't match `com.lionos.AiAssistant`. Renamed to `com.lionos.AiAssistant.metainfo.xml`; validates clean except one warning that's really a placeholder-content issue (the project's own GitHub URL isn't real yet), not a spec violation.
- [x] **DEB** (`packaging/deb/`, `scripts/package/build-deb.sh`): real `dpkg-deb --build --root-owner-group` produced a well-formed package (correct FHS layout, correct permissions, `Installed-Size` computed from the actual staged tree, `postinst`/`postrm` present and marked executable). Then actually installed with `dpkg -i` -- `ai-assistant` landed on `PATH` and ran, the desktop file and all seven icon sizes installed under `/usr/share/`, `dpkg -l` showed it correctly registered -- and cleanly removed with `dpkg -r`, confirmed nothing left behind.
- [x] **RPM** (`packaging/rpm/ai-assistant.spec`, `scripts/package/build-rpm.sh`): real `rpmbuild -bb` (via `rpm-build`'s actual binary, `/usr/bin/rpmbuild`, installed via the plain `rpm` apt package) produced a well-formed package -- `rpm -qip`/`-qlp`/`-qRp` show correct metadata, file layout, and an auto-detected `Requires:` list (glibc/libgcc SONAME versions) that rpmbuild itself derived from the binary's actual dynamic linking, not hand-written. Installing it with `rpm -i` was correctly rejected: this Debian-based sandbox has no native RPM dependency database, so `glibc`/`libc.so.6(...)` etc. aren't satisfiable even though the real libraries are present (installed via `dpkg`, invisible to `rpm`'s own provides database) -- confirmed to be an environment limitation by reading `rpm`'s own warning ("RPM should not be used directly ... use Alien instead"), not a defect in the spec file.
- [x] **AppImage** (`packaging/appimage/`, `scripts/package/build-appimage.sh`): downloaded real `appimagetool` (continuous release, from `github.com`/`release-assets.githubusercontent.com`, both already in this sandbox's network allowlist) and built a real, complete AppImage -- runtime embedded, AppStream metadata validated inline by `appimagetool`'s own call into `appstreamcli` (which is how the metainfo filename bug above actually got caught). Ran the resulting `.AppImage` directly; it executed the bundled binary correctly.
- [x] **Portable tarball** (`packaging/portable/` build output, `scripts/package/build-portable.sh`): the sixth format, not in Development_plan's original five but already implied by the project's own directory layout. No external packaging tool at all -- just `tar` -- so it's the most completely verified of the six: built, extracted to a clean directory, and run via its own `run.sh`.
- [x] **`scripts/package/build-all.sh`**: runs DEB + RPM + AppImage + portable in one command; run end-to-end for real, produced all four artifacts correctly in `target/package/`.
- [x] Every packaging script (`render-icons.sh`, five `build-*.sh` files, `packaging/appimage/AppRun`, `packaging/deb/postinst`/`postrm`) passes `shellcheck` with zero warnings.
- [ ] **Flatpak** (`packaging/flatpak/com.lionos.AiAssistant.yml`): written to the real spec -- `org.freedesktop.Platform`/`Sdk` 23.08 runtime, a `simple` buildsystem module packaging the pre-built binary (see the note below on why, not a source build), and a `finish-args` list reasoned through explicitly against the tension between this app's own system-control mission and what Flatpak sandboxing is *for* (documented at length in the manifest's own comments, not just asserted here). `flatpak-builder --show-manifest` confirms it's fully valid JSON-parseable YAML with every field understood; a real `flatpak-builder --force-clean` build gets exactly one step further before failing, at "SDK not installed" -- confirmed empirically that this is a network wall, not a mistake, by `curl`ing `dl.flathub.org` directly and getting back this sandbox's own egress proxy's `x-deny-reason: host_not_allowed`, not a DNS or TLS failure.
- [ ] **Snap** (`packaging/snap/snapcraft.yaml`): written to the real spec from memory (base, confinement, apps, parts, `extensions: [gnome]`, `plugin: dump` for the same pre-built-binary reason as Flatpak) -- explicitly declares `confinement: devmode` rather than `strict`, with a comment explaining why that's not a shipping configuration, matching this project's standing "don't overclaim a property that hasn't been checked" principle. No `snapcraft` binary exists via apt on this Debian-based sandbox at all (Canonical distributes it primarily as a snap, a circular problem on a system with no snapd), and `snapcraft.io` itself returned `host_not_allowed`. Only generically validated as well-formed YAML (Python's `yaml.safe_load`), not against snapcraft's actual schema.
- [ ] **Not done, deliberately**: neither the Flatpak nor the Snap module builds Rust from source inside its own build sandbox -- both would need the *entire* workspace's `Cargo.lock` dependency tree vendored (via `flatpak-cargo-generator` for Flatpak; something equivalent for Snap), a substantial, separate undertaking orthogonal to what's actually blocking a real `ai-assistant` binary from existing yet. All six formats take the same pragmatic pre-built-binary approach for consistency, not just the two that are network-blocked anyway.

### Verifying six packaging formats without a real binary to package

Every format here fundamentally packages a compiled `ai-assistant`
binary, and one has never existed in this sandbox. Rather than either
skip real verification entirely (leaving six formats in the weakest,
"written but never run" category this project uses for Slint/Candle
work) or overstate what got checked, the same technique Phase 18's
throwaway harness used for `app`'s wiring got applied here: compile a
trivial stand-in binary (`rustc -O` on a five-line `fn main()`, named
`ai-assistant`, placed at the normal `target/release/ai-assistant`
location) and run every packaging script against *that*, for real,
checking the actual packaging mechanism -- directory layout, control
file syntax, permission bits, maintainer scripts, install/remove
behavior -- independent of whatever ends up inside the binary itself.

This is a meaningfully different, stronger claim than "the script looks
right": `dpkg-deb --build` either produces a well-formed archive or it
doesn't, `rpmbuild` either resolves `%{buildroot}`/`%files` correctly or
it doesn't, `appimagetool` either assembles a runnable AppImage or it
doesn't -- none of that depends on what the packaged binary actually
does once it's running, only on whether it's a valid ELF executable at
the path the script expects. A stand-in that prints one line and exits
is exactly as valid an ELF executable, for packaging-mechanism purposes,
as the real `ai-assistant` will be. What this *doesn't* prove: that the
real binary's actual runtime library dependencies match the
best-effort `Depends:`/`Requires:` lists in the .deb control file and
.rpm spec (both explicitly say so in their own comments) -- that needs
`ldd` against a real binary, which needs the Slint/Candle wall gone
first.

### The Flatpak/Snap network wall, and why it's not the same shape as Slint/Candle's

Every other unverifiable piece of this project so far -- Slint since
Phase 2, Candle since Phase 4, `criterion` in Phase 19 -- has been
blocked by this sandbox's *rustc version*: real tools, reachable
network, just a compiler too old for what they need. Flatpak and Snap
are a different kind of wall entirely: the tools either aren't
reachable (Flathub, `snapcraft.io`) or don't exist here at all
(`snapcraft`), regardless of toolchain version. That's worth being
precise about rather than lumping every "written but unverified" item
into one undifferentiated bucket -- a future session with broader
network access could plausibly finish verifying Flatpak and Snap today,
with the exact toolchain this sandbox already has; it could *not*
finish verifying Slint or Candle without a newer rustc first. Different
problems, different fixes, so it seemed worth not blurring them
together just because both end up in the same "not fully verified"
checklist category.

## Phase 21 checklist

Five modules per Development_plan: API Docs, User Guide, Developer
Guide, SDK Guide, Examples. Unlike Phase 20, this phase is back to
familiar territory -- real Rust, real `cargo` commands -- but the actual
work is different in kind from every code-writing phase before it: the
job here was making existing, extensive doc-comment coverage (every
phase since 1 has written `//!`/`///` comments explaining design
rationale, not just what a function does) actually *build* cleanly and
be genuinely navigable, plus writing the three guides no amount of doc
comments substitutes for -- how to work on this codebase, how to extend
it, and (honestly scoped) how to use it.

- [x] **API Docs**: `cargo doc --no-deps` across all 33 verified crates, zero warnings. Wasn't already zero -- `rustdoc::broken_intra_doc_links` and `rustdoc::redundant_explicit_links` (both on by default) caught four real mistakes: doc comments in `assistant-core::LazyResponseGenerator` and `utilities::run_blocking` (both written in Phase 18) linking to `#[cfg(test)]`-only test functions, which don't exist in a normal (non-test-cfg) doc build regardless of how the link is spelled; a redundant explicit link target in `workflow-engine::executor`; and a doc link to a private constant (`MAX_LABEL_LEN`) in `telemetry::event`. All four fixed by rewording rather than exposing anything -- the tests/constants themselves don't need to be public, the comments just needed to stop claiming rustdoc could resolve a link to something it structurally can't see.
- [x] **`docs/API.md`**: a categorized index (core pipeline / tools+permissions / memory+search / extensibility / automation / voice / infrastructure / testing / unverified) pointing to the real generated rustdoc rather than duplicating it, plus the four new examples below.
- [x] **Examples** (Development_plan's fifth module, and the one with the most actual engineering risk this phase): `cache/examples/basic_usage.rs`, `tool-framework/examples/custom_tool.rs`, `memory/examples/remember_and_search.rs`, `assistant-core/examples/minimal_chat_loop.rs`. Every one was checked against real source before being run, not written from memory and assumed correct -- `tool-framework`'s example needed two real corrections (`ToolOutcome` turned out to be a plain struct, not the `::ok()`/`.with_data()` builder API guessed at first; `ResultStatus` has `ToolUnavailable`, not the guessed `ToolNotFound`) caught by actually trying to compile it, not by careful reading. All four run correctly, `clippy --examples -D warnings` clean (one `clippy::box_default` catch, the same lint Phase 19 already hit once), `cargo fmt` clean.
- [x] **`docs/DEVELOPER_GUIDE.md`**: the "which crates are actually verified" triage from `docs/ROADMAP.md`'s own table, generalized into a standing rule (check before you start); both of this project's real techniques for verifying code that can't compile, named and compared (throwaway harness vs. stand-in artifact -- Phase 18 and Phase 20 respectively); cross-crate conventions (error-mapping helpers, `register_all`, why `Arc<Mutex<_>>` sharing lives at the call site and not baked into e.g. `MemoryStore` itself); concrete "add a tool" / "add a crate" walkthroughs; a short testing-philosophy section (real timing over mocked clocks, equivalence-against-a-reference over hand-picked cases, invalidation tests mattering more than hit tests for anything cached).
- [x] **`docs/SDK_GUIDE.md`**: written against the actual current `Plugin` trait and `PluginManifest` TOML shape (re-read from `crates/plugin-manager/src/plugin.rs`/`manifest.rs` while writing this, not recalled from having built it earlier) -- the full lifecycle (register/validate/activate/collect_tools/deactivate) with real method signatures, and pointers to Phase 19's actual cross-crate panic-isolation tests as the thing to mirror when testing a new plugin, not just prose reassurance that panics are handled.
- [x] **`docs/USER_GUIDE.md`**: opens with the same honest scoping this project applies everywhere else -- what's actually been run (every individual piece, standalone) versus what hasn't (the assembled `app` binary, still blocked on Slint/Candle). Covers installation via all of Phase 20's package formats, the real `config/default.toml` shape (including a drift this phase found and fixed -- see below), memory/privacy, and points at Phase 19's security findings by name rather than re-describing them and risking the two copies drifting apart.
- [x] Found and fixed one small pre-existing drift while writing the User Guide: `config/default.toml` -- the actual shipped config file, not just `config::PerformanceConfig`'s Rust-side defaults -- never got a `[performance]` section added when Phase 18 introduced it, even though Phase 17's `[ui]` fields were added there at the time. Added the section with the same inline-comment style the `[ui]` block already used, then verified it round-trips through a real `Config::load` (a throwaway harness, deleted after use) rather than just eyeballing the TOML syntax.
- [x] **Found a genuine build-order hazard via this phase's own clean-room tarball verification** (not a code bug, and not present in the README's actual documented commands, but real enough to be worth writing down): `plugin_manager::dynamic`'s tests dynamically load `example-plugin`'s compiled `.so` by filesystem path rather than through normal Rust `use`, so `example-plugin` being a `[dev-dependencies]` entry doesn't reliably cause Cargo to rebuild its cdylib artifact on every invocation -- only an explicit build of `example-plugin`'s own default lib target does. This session's single long-lived `target/` directory had that artifact sitting there from early in Phase 18 and never noticed; a genuinely fresh `target/` (extracting this phase's own output tarball and running tests directly, skipping the README's separate preliminary `cargo build` step) surfaced it immediately as a real, reproducible failure. Confirmed precisely, not just patched over: deleting `target/` entirely and running the README's actual two-command sequence (`cargo build ...` *then* `cargo test ...`, both with the full package list, exactly as documented) passes cleanly every time; skipping straight to `cargo test` -- on the full list or a subset -- can fail non-deterministically depending on what's already on disk. Documented as a `docs/DEVELOPER_GUIDE.md` gotcha rather than quietly working around it, since it's a real trap for anyone who (very reasonably) runs `cargo test -p plugin-manager` alone while iterating.
- [x] 447/447 tests passing, unchanged from Phase 20 -- this phase touched doc comments, new example files, and one config file, no crate's actual logic.

## Phase 22 checklist

Development_plan's final original-scope phase: Production Release,
Release Notes, Long-Term Support Branch, Bug Fixes, Performance Review.
Phase 23 onward are explicitly "Future Phases" (Vision Support, Coding
Agent, Multi-Agent, RAG, MCP, Mobile, Developer Platform, AI OS
Integration) -- a different, later scope, not this phase's job. The
single most consequential decision this phase makes isn't a line of
code: it's declining to call this v1.0.0, and being precise about why.

- [x] **Production Release, assessed honestly rather than declared**: `version.workspace = true` applies to every crate including `app`, which has never compiled in this environment across all 22 phases. A version bump toward 1.0.0 -- even a conservative one meant only to signal "the library layer is mature" -- would land on `app` too, the one crate where a reader would reasonably take a high version number as a claim the software actually runs. The version stays at 0.1.0. `docs/RELEASE_NOTES.md` does the honest work a version number can't: a plain, evidence-based assessment against **PRD SS18's actual Definition of Done**, not this project's own aspirational sense of how far it's come. Result: 2 of 6 criteria fully met (modularity, permissions/logs), 2 met at the library level but not end-to-end (safe Linux actions, local chat -- both real and tested in isolation, never connected through a running `app`), 2 not met at all (working UI, end-to-end-tested MVP). Writing "2 of 6" in a release-notes document for a 22-phase project is an uncomfortable sentence to write; it's also the accurate one, and this project's entire documentation style (every phase, every ROADMAP entry) has been built on writing the accurate sentence instead of the comfortable one.
- [x] **Release Notes** (`docs/RELEASE_NOTES.md`): the Definition-of-Done table above, a "what genuinely is done, and solid" section that gives real credit to the substantial, tested foundation (32 verified crates, a real tool-calling pipeline, tested cache invalidation, six packaging formats, warning-free API docs) without letting that credit blur into overclaiming the assembled product, a **complete** known-limitations list (not a curated subset -- includes the router-bypass and Landlock-enforcement findings from Phases 16/19 again, by name, rather than assuming a reader already found them in `docs/ROADMAP.md`), and a concrete, dependency-ordered path to an honest future v1.0.0 (toolchain first, then `app` compiling for the first time, then the real tool-call wiring every phase since 7 has correctly deferred, then re-running this phase's own Performance Review against a real running app, *then* a version number that means it).
- [x] **Long-Term Support Branch**: `lts/0.1.0-foundation`, created at this phase's final commit -- a real git branch, not just a mention in a document. Exists so future work with access to a modern toolchain (unblocking Slint/Candle) has a clean, fully-tested, fully-documented starting point without needing to replay 22 phases of history to find one.
- [x] **Bug Fixes**: one substantive fix, chosen for being a genuine, previously-*named* gap rather than a search for something to fix. Phase 16 wrote and tested `sandbox::FilesystemSandbox::restrict_current_process()` (real Landlock-based process restriction); Phase 19's security tests then documented, honestly, that nothing in the workspace actually *called* it at process startup, so file-reading tools' lack of their own path-scoping had no real mitigation in practice. This phase closes that: `app::setup_filesystem_sandbox` runs before anything else touches disk (even before `Config::load`, using the fixed set of directories the app always needs), restricts the process to `config/`, `models/`, `assets/`, `data/`, `logs/`, and -- once `tracing` is available a few lines later, since logging needs config which needs this to have already run -- logs the real `EnforcementStatus` rather than assuming success either way. `app` itself still can't be compiled to verify this end-to-end, so the same throwaway-harness technique Phase 18 established got reused: a small crate outside the workspace, depending on `sandbox`/`metrics`/`common` via absolute paths, reproducing `setup_filesystem_sandbox` and `main`'s handling of its result exactly. It compiled and ran correctly on the first real attempt, confirmed the expected `NotEnforced` status in this sandbox (matching Phase 16's own finding, not a new problem), and confirmed the allowed paths (`data/`) remain genuinely readable and writable after the restriction call -- not just that the call itself doesn't error.
- [x] **Performance Review** (`docs/PERFORMANCE_REVIEW.md`): every measurable target from TDS SS9/DBDMS SS13/Archd SS19 checked against real numbers from this development environment on this date, reproducible by re-running `cargo bench -p vector-db`/`-p cache`. Memory retrieval, database startup, and indexed search all clear their stated targets by roughly 50-500x. An honest linear-extrapolation section, clearly labeled as a projection rather than a measurement: `vector_db`'s O(n) scoring pass scales almost exactly linearly with entry count in the measured 100-20,000 range (a 200x increase in `n` produced a ~200x increase in search time), and extrapolating that same line out to 1 million entries lands around 90ms -- past DBDMS's 50ms "common queries" target. Not a problem today (`config::PerformanceConfig`'s defaults are nowhere near that scale), and `vector_db`'s own doc comment already names the fix (an approximate-nearest-neighbor index) as future work rather than a surprise finding.
- [x] 447/447 tests passing, unchanged -- this phase's only logic change was the sandbox wiring, additive to `app` (unverified directly, harness-verified indirectly) and not something any existing crate's test suite covers or could regress.

### Why this project never tags v1.0.0 lightly

Every phase in this project has followed the same rule when reporting
status: state what was actually verified, name what wasn't, and don't
let the two blur together. A version number is a much blunter
instrument than a ROADMAP entry -- it's one number, seen by someone who
may never read `docs/ROADMAP.md` at all, and "1.0.0" carries an implicit
promise ("this works, use it") that a careful paragraph can qualify but
a version string can't. Given that, treating the version number with
the same honesty standard as everything else in this project's
documentation -- rather than as a marketing decision separate from
it -- was the only consistent choice available once `app` turned out to
still not compile at the point this phase started. A future phase that
*does* get `app` running for the first time should feel free to bump
this immediately; the number was never precious, just accurate.

## Phase 23 checklist

Development_plan's first "Future Phase": OCR, Image Understanding,
Screenshot Analysis. Different in kind from Phase 22's assessment work --
back to writing new crates -- but the same honesty discipline applies:
this phase turned out to be an unusually *good* one for it, since two of
its three deliverables could be verified more thoroughly than most of
this project's existing Linux integration work, not less.

- [x] **Fresh-sandbox finding, before any code**: this session started with no Rust toolchain installed at all -- not even the documented pinned 1.75 -- and `iproute2` (already documented, in `linux_services::network`'s own source comment, as a dependency its tests need) was missing too. Confirms container state doesn't persist between sessions the way this project's own file/git state does; reinstalled `rustc`/`cargo`/`rustfmt`/`clippy` 1.75 and `iproute2` before doing anything else, then reproduced the documented 447/447 baseline exactly, unchanged, before writing a single line of Phase 23 code. Empirical-first applies to the environment itself, not just the code running in it.
- [x] **`screenshot`** (Screenshot Analysis): `capture_full_screen` wraps `import -window root -silent` (X11) or `grim` (Wayland) via `std::process::Command`, zero new dependencies -- the same one-external-tool-per-module shape `linux_services::audio`/`notifications` already established, not a new pattern. Session detection (`WAYLAND_DISPLAY` before `DISPLAY`) is `detect_session_from`, a pure function over explicit `Option<&str>` arguments rather than reading `std::env` directly inside the function under test -- deliberately, so its tests stay deterministic under `cargo test`'s default parallelism instead of racing on shared mutable process env state the way a `set_var`/`remove_var`-based test would. Dimensions are read back via a hand-rolled `IHDR`-chunk parser rather than pulling in an image-decoding crate for two integers -- the PNG spec mandates IHDR as the very first chunk at a fixed byte offset, so this is exact for any encoder, not a shortcut specific to ImageMagick's or grim's output.
- [x] **`screenshot`'s success path is fully verified for real**, a first for this project's desktop-session-dependent Linux integration: Phase 8's audio/display/notification/clipboard modules could only prove graceful failure, because that sandbox had no sound server, backlight device, notification daemon, or clipboard selection to succeed against. This one does have something to succeed against: a real (if headless) X11 server via `Xvfb :99 -screen 0 1024x768x24`. Confirmed by hand before being written down as a test -- real `import` invocation against it, real PNG written, real dimensions read back matching the configured 1024x768 screen exactly. The Wayland path's *success* case has no equivalent (no wlroots compositor to stand up the way Xvfb stands in for a real X server); only its failure mode (missing `grim` binary, which this sandbox genuinely doesn't have) is exercised, and that limitation -- GNOME/KDE Wayland sessions block direct capture and need the XDG Desktop Portal instead, which isn't implemented -- is documented in the crate's doc comment rather than left implicit.
- [x] **`capture_screenshot`'s permission level is `Moderate`, not `ReadOnly`** -- a deliberate, explicitly-reasoned departure from `read_file`'s precedent, not an oversight. `read_file` requires the caller to already know and name a specific path; a screenshot exposes whatever's currently on screen, across every window, without the caller naming anything at all. TCLIS SS6's levels don't have a category that fits either precedent perfectly, so this is this project's own judgment call, written down as one (in the tool's own doc comment) rather than presented as spec-mandated.
- [x] **`ocr`** (OCR): wraps the `tesseract` CLI the same way, zero new dependencies. `extract_text` validates its `language` parameter's shape (lowercase letters, optionally `+`-joined) before it ever reaches `Command` -- not a shell-injection concern (`std::process::Command` never invokes a shell), but TCLIS SS13's general "tool inputs are sanitized and validated" principle: fail fast with a specific message rather than letting `tesseract` reject it on its own terms. Fully, unconditionally verified for real (no display server needed, just an installed binary, unlike `screenshot`): a synthetic PNG containing the real, known text "Phase 23 Vision" is generated inline in the test via ImageMagick's `convert -annotate`, then handed to a real `tesseract` binary, which reads it back correctly. `extract_text_from_image` is `ReadOnly`, matching `read_file`'s exact precedent: a caller-named existing file, the same risk shape, so holding it to a stricter standard than `read_file` itself would be inconsistency, not extra caution.
- [x] **`vision`** (Image Understanding, real half): `analyze_image` decodes real pixel data via the `image` crate -- dimensions, decoder-reported format, per-channel mean color -- pinned `=0.24.9` in that crate's `Cargo.toml`. Confirmed empirically before committing to the pin: unpinned `image = "0.25"` fails to even resolve dependencies in this sandbox (pulls in `moxcms`, which requires the `edition2024` Cargo feature), the identical class of wall `workspace.dependencies`' existing `uuid = "=1.12.1"` pin already documents; `=0.24.9` compiles and was confirmed decoding a real captured screenshot's real dimensions correctly before being written into any crate. Also defines the `VisionBackend` trait -- deliberately mirrors `assistant_core::ResponseGenerator`'s role and shape (`&self, ... -> String`, infallible from the caller's side), the same relationship one phase later -- and ships `MetadataOnlyVisionDescriber`, an honest stub matching `EchoResponseGenerator`'s own reasoning exactly: incorporates real computed facts (real dimensions, real color) into its output without ever implying it interpreted the image's actual content. `describe_image` is `ReadOnly`, same reasoning as `extract_text_from_image`.
- [x] **`vision-runtime`** (Image Understanding, Candle half): a CLIP zero-shot image classifier implementing `vision::VisionBackend`, joining `llm-runtime` in the unverified-in-this-sandbox category -- Candle needs rustc 1.88+, confirmed to reproduce exactly (including the same `getrandom`/`edition2024` transitive-resolution failure `uuid`'s own comment already names, not a new or different wall). Its doc comment draws an explicit, honest line that `llm-runtime`'s own doesn't need to: `ClipConfig`'s real field shape (`text_config`/`vision_config`/`logit_scale_init_value`/`image_size`) was confirmed against real, fetched candle-transformers source (quoted, with commit hash, from a real GitHub issue), but the constructor and forward-pass method names built on top of that were reasoned from Candle's own consistent `VarBuilder`+`Config`-loading convention -- confirmed elsewhere in *this* exact workspace, in `bert.rs` and `llm-runtime`'s own integration -- rather than independently re-fetched for `clip/mod.rs` specifically this session. Said plainly rather than implying the same bar `llm-runtime` cleared. Candidate labels are supplied by the caller at construction (zero-shot classification against a given label set, not open-ended captioning) -- Phase 23's honestly-scoped answer to "Image Understanding," not a stand-in for a future captioning-capable model this project doesn't have.
- [x] Gemma 3 1B (this project's configured default model, PRD SS9) is text-only -- the smallest Gemma 3 size with a vision encoder is 4B. Noted explicitly in `vision`'s doc comment: even once `vision-runtime` compiles somewhere with a modern toolchain, "Image Understanding" needs its own separate model, not a capability the configured default happens to already have.
- [x] New `ToolCategory::Vision` (`tool-framework::lib`) -- not in TCLIS SS4's original category list, the same kind of later, Development_plan-anticipated addition `Search` and `Automation` already were (TCLIS SS15 "Future Expansion" names "OCR" and "local vision models" explicitly). Checked for exhaustive `match` statements on `ToolCategory` across the workspace before adding the variant (none exist -- every reference is `{:?}` `Debug` formatting), so this was a safe, non-breaking addition, confirmed rather than assumed.
- [x] `config::VisionConfig` (`ocr_language`, `screenshot_dir`) added to `Config`, `config/default.toml` gets a new `[vision]` section with the same inline-comment style `[performance]` already uses. Backward-compatibility-tested the same way Phase 18's `[performance]` addition was: a config file written before Phase 23 (missing `[vision]` entirely) still loads with sane defaults. Documented honestly as a settings *surface* (PRD 6.7) rather than something the tool crates read directly -- `Tool::execute` only ever receives `&ToolArgs` (TCLIS SS5), never a `Config`, the same not-yet-self-enforcing relationship `enabled_tools`/`enabled_plugins` already have.
- [x] **`integration-tests/tests/security_adversarial.rs`**, extended rather than duplicated: `ocr_and_vision_tools_share_read_files_unscoped_path_access` confirms, concretely, that `extract_text_from_image` and `describe_image` inherit `read_file`'s existing documented no-path-scoping finding (same `ReadOnly` level, same underlying reasoning) rather than just assuming it follows from being given the same permission level. `unlike_read_file_and_ocr_and_vision_screenshot_capture_actually_asks_first` is the paired negative case, confirming `Moderate` actually gates at the router -- `AlwaysDeny` is genuinely asked this time, and genuinely gets to say no.
- [x] **`integration-tests/tests/cross_crate_flows.rs`**: `a_real_captured_screenshot_flows_into_ocr_through_the_same_router` -- a real screenshot, captured through Xvfb, handed as-is to `ocr`'s tool through the same `ToolRouter` in one scenario. First proof that these two new crates' output and input genuinely fit together in practice (a captured PNG is something `tesseract` can actually open), not just that their types line up on paper -- exactly the kind of "genuinely new combination" this test file exists for.
- [x] **Found and fixed a pre-existing issue, unrelated to this phase's own changes**: `example-plugin::plugin_free_string` took a raw pointer and reconstructed a value from it (`CString::from_raw`) without the function itself being marked `unsafe` -- a real `clippy::not_unsafe_ptr_arg_deref` finding, not a false positive, surfaced by this session's freshly-(re)installed clippy rather than something Phase 11 got structurally wrong (the unsafe *operation* inside the function was already correctly scoped in its own `unsafe` block with a real safety comment; only the function signature's own annotation was the gap). Fixed by marking it `pub unsafe extern "C" fn` plus adding the `# Safety` doc section clippy then also required. Confirmed zero practical impact before considering it done, not just compiling clean: `plugin_manager::dynamic` already declared this exact symbol's expected type as `Symbol<unsafe extern "C" fn(*mut c_char)>` (calling anything through `libloading::Symbol` requires `unsafe` at the call site regardless of the callee's own signature), so the fix brings the real export in line with what its only actual caller already assumed. Rebuilt `example-plugin`'s `.so` and reran `plugin-manager`'s real dynamic-loading tests afterward (24/24 passing, including `loads_a_real_compiled_plugin_and_reads_its_metadata`) to confirm the calling convention genuinely didn't change, rather than trusting that reasoning alone.
- [x] 481/481 tests passing (+34 over Phase 22), 36 verified crates (+3 over Phase 22's 33) -- `screenshot` (13: 10 unit + 3 permission-integration), `ocr` (8: 7 unit + 1 permission-integration), `vision` (7: 6 unit + 1 permission-integration), `config` (+4), `integration-tests` (+3: 2 security findings + 1 cross-crate flow). `cargo fmt --all -- --check` clean workspace-wide, including `vision-runtime` alongside `ui`/`app`/`llm-runtime` -- `rustfmt` parses syntax, it doesn't need dependencies to resolve, so even permanently-uncompiled code stays held to the same formatting standard as everything else. `cargo clippy --all-targets -- -D warnings` clean across all 36 verified crates.

## Phase 24 checklist

Development_plan's second "Future Phase": Code Generation, Project
Analysis, Debugging. Structured differently from Phase 23 on purpose --
see the crate-level doc comment for why one crate with three modules,
not three crates.

- [x] **`analysis`** (Project Analysis): real recursive directory traversal (`std::fs`, no new dependency), skipping `target`/`node_modules`/`.git`/`.venv`/`__pycache__`/`dist`/`build`/`.cargo` -- generated or vendored content, not the project itself. A configurable file-walk ceiling (`DEFAULT_MAX_FILES_WALKED = 20_000`, overridable per call) rather than an unbounded walk -- TCLIS SS7 "resource limits" applies to how much work a tool can be made to do, not only to its inputs' shape. Manifest parsing is real, not heuristic, for Cargo.toml (`toml`, already a workspace dependency) and package.json (`serde_json`, likewise) -- both have one canonical dependency-table shape. pyproject.toml is explicitly handled as a best-effort case instead: PEP 621's `[project.dependencies]` (an array) and Poetry's `[tool.poetry.dependencies]` (a table) are both real, both common, and structurally different, so both are checked and whichever is found is reported, documented as a heuristic rather than asserted with the same confidence as the Cargo/npm paths.
- [x] Verified against a real target, not only synthetic fixtures: `analyzes_this_very_workspaces_cache_crate_for_real` runs `analyze_project` against this workspace's own already-committed `cache` crate (Phase 18) and asserts on its real, known file count, language breakdown, and parsed package name.
- [x] **`diagnostics`** (Debugging): real `cargo check --message-format=json` integration, its actual JSON shape confirmed by hand (a scratch crate with a deliberate `mismatched types` error, checked, and the real output inspected) before any parsing code was written -- this project's standing empirical-first rule applied to a compiler's own output format, not just to this project's own code. `cargo check`, not `cargo build`: both surface identical diagnostics (same frontend passes), `check` skips codegen and is strictly faster for a tool that only needs to report what's wrong.
- [x] **A real nuance found by testing against an actual broken crate, not assumed**: one compile error produces *two* `"level":"error"` JSON messages -- the located diagnostic itself, and a second, unlocated `"aborting due to previous error"` summary line. The first version of this module's own test asserted `error_count() == 1` and failed against the real output, which was the *test's* assumption being wrong, not the parser -- `error_count`/`warning_count` were changed to count only diagnostics carrying a real source location (matching what a user would recognize as "how many distinct problems"), while `DiagnosticReport::diagnostics` still keeps every raw message cargo actually emitted, unfiltered.
- [x] **`generation`** (Code Generation): `generate_from_template` is real, deterministic Rust scaffolding (`function`/`struct`/`test`) -- no model, nothing blocked by any toolchain wall. Its own test doesn't just eyeball the output string: `every_template_kind_produces_code_that_actually_compiles` writes all three template kinds into a real scratch crate and runs a real `cargo check` against the result. `generate_freeform` routes a description through `assistant_core::ResponseGenerator` -- the identical trait `AssistantCore::process_message` already uses for chat, reused directly rather than given its own new trait the way `vision::VisionBackend` was, since code generation's shape (`&str` prompt in, `String` out) is structurally identical to chat's, unlike image understanding's need for real pixel data. With no real backend wired in (this crate's own tests and default registration use `EchoResponseGenerator`), freeform generation honestly produces the same stub reply Phase 3 already established -- proven, not just claimed, by `freeform_generation_with_the_default_stub_is_honest_about_being_a_stub`.
- [x] New `ToolCategory::Development` (`tool-framework::lib`) -- not in TCLIS SS4's original list, the same kind of Development_plan-anticipated addition `Vision` (Phase 23), `Search`, and `Automation` already were.
- [x] **`run_diagnostics`'s permission level is `Destructive`, not `ReadOnly`** -- the most consequential judgment call this phase made, and the most heavily documented (see `diagnostics::run_cargo_diagnostics`'s and `tools::RunDiagnosticsTool`'s own doc comments). `cargo check` compiles real code: it runs `build.rs` scripts and executes proc-macros at compile time, both with this process's full privileges and neither sandboxed by Cargo itself. TCLIS SS6 doesn't have a category for "looks read-only but can execute arbitrary code depending on what's already in the target directory" -- anchored at `Destructive` rather than `Moderate` because TCLIS SS6's own Level 3 examples already include "remove packages," a comparable "runs code from a project's own declared dependencies" risk via install scripts. `sandbox::FilesystemSandbox` isn't wired into the spawned `cargo` process this phase -- named as a real, open gap (a natural next hardening step), not silently left out.
- [x] **The concrete proof behind that reasoning**, in `integration-tests/tests/security_adversarial.rs`, not just a level check: a real crate whose `build.rs` writes a canary file the instant it runs. `run_diagnostics_permission_gate_prevents_build_rs_from_ever_executing` confirms a denied `run_diagnostics` call means the canary is never created -- the permission gate stops the *build from starting*, not merely reports a denial after the fact, mirroring the same rigor the Phase 22 `build.rs` canary proof already established for `Destructive`-gated `cargo check` calls generally. The paired `run_diagnostics_actually_runs_build_rs_when_genuinely_approved` proves the mechanism is real (the canary genuinely *can* be created by this pipeline) rather than the denied case being silently vacuous -- this workspace's first test that confirms a permission gate prevents a real side effect from happening at all, not only that it returns the right status code.
- [x] `config::CodingAgentConfig` (`max_files_walked`) added to `Config`, backward-compatibility-tested the same way `[vision]`/`[performance]` were. Genuinely wired through, not only decorative: `AnalyzeProjectTool` accepts an optional `max_files` argument (`analyze_project_with_limit`), matching `DEFAULT_MAX_FILES_WALKED` when absent -- the same "settings surface a caller wires through explicitly" relationship `VisionConfig` established, made concrete this time by giving the tool itself the override path.
- [x] 512/512 tests passing (+31 over Phase 23), 37 verified crates (+1) -- `coding-agent` (26: 23 unit + 3 permission-integration), `config` (+3), `integration-tests` (+2: the `build.rs` canary pair). `cargo fmt --all -- --check` clean workspace-wide; `clippy --all-targets -D warnings` clean across all 37 verified crates; `cargo doc --no-deps` zero warnings, rechecked including `coding-agent`.

## Phase 25 checklist

Development_plan's third "Future Phase": Planner Agent, Research Agent,
Coding Agent, Memory Agent, Security Agent (Ai_reasoning SS4 "Agent
Architecture", SS13 "Agent Communication", SS15 "Extensibility"). One
new crate, `agents` -- genuinely new architecture in `message`/
`coordinator`, five wrappers around already-real crates everywhere else.

- [x] **`message::AgentMessage`**: `sender`, `receiver`, `task_id`, `payload`, `timestamp`, `priority` -- every field traces directly to Ai_reasoning SS13's own sentence describing agent communication, not invented independently. `priority` reuses `planner::Priority` rather than a second, near-identical enum. Not `Serialize`/`Deserialize` itself -- nothing in this phase actually serializes a whole message (dispatch is in-process), and adding the derive unused (`planner::Priority` doesn't have it either) would be exactly the kind of ahead-of-a-real-need surface this project avoids adding.
- [x] **`coordinator::{Agent, Coordinator}`**: deliberately mirrors `tool_framework::{Tool, ToolRegistry, ToolRouter}` one layer up rather than inventing a second pattern for the same underlying problem -- `Coordinator::dispatch` reuses `ToolRouter::execute`'s exact `tokio::time::timeout(..., spawn_blocking(...))` three-way match (agent ran and failed / agent panicked / agent didn't finish in time), which already correctly distinguishes those three outcomes. `Agent::handle` is synchronous, matching `Tool::execute`'s own reasoning exactly (no agent here does true async I/O -- `MemoryStore`, `SearchEngine`, and `coding_agent`'s functions are all blocking calls under the hood). One deliberate, considered difference from `ToolRegistry::register` worth calling out rather than leaving implicit: `Coordinator::register` replaces a same-named agent silently instead of erroring, since agents are few, named by role rather than generated, and a caller intentionally swapping one out (a test double, say) is a legitimate use this shouldn't block.
- [x] **What `Coordinator` deliberately doesn't claim**: Ai_reasoning SS4's Coordinator Agent responsibilities also include "Build final response" -- full request-to-response orchestration, which stays `assistant_core::AssistantCore::process_message`'s job. This phase doesn't merge the two; said explicitly in `coordinator`'s own doc comment rather than left to be discovered by absence.
- [x] **`planner_agent::PlannerAgent`** wraps `planner::Planner` (Phase 6) unchanged -- Ai_reasoning SS4's Planner Agent responsibilities ("break large tasks into steps, select tools, estimate complexity") are exactly what `create_plan` already does. Reply is a direct, field-for-field JSON rendering of the real `Plan` returned.
- [x] **`memory_agent::MemoryAgent`** wraps `memory::MemoryStore` (Phase 5) behind `Arc<Mutex<_>>`, matching `search::MemorySearchProvider`'s own established sharing pattern for this exact type rather than inventing a different one. `remember`/`recall`/`forget` actions map directly onto real `MemoryStore` methods; `forgetting_an_unknown_id_is_a_real_false_not_an_error` confirms the wrapper preserves `MemoryStore::forget`'s own honest `false`-not-error behavior for an unknown id rather than flattening it into a generic error.
- [x] **`research_agent::ResearchAgent`** wraps `search::SearchEngine` (Phase 13) -- **local search only**, said explicitly in this module's own doc comment and `agents`' crate-level one: PRD SS3's "no cloud dependency for the core assistant" non-goal means "research" here means this assistant's own files, remembered long-term memories, and registered tools/workflows, never the web. No web-search capability exists anywhere in this workspace to wrap even if a broader scope were wanted.
- [x] **`code_agent::CodeAgent`** wraps `coding_agent`'s real analysis/diagnostics/template-generation functions (Phase 24) -- named `code_agent`, not `coding_agent`, specifically so the module doesn't shadow the `coding-agent` crate dependency's own name in scope. `run_diagnostics_...` behavior means this agent's own `timeout()` is 120 seconds (matching `coding_agent::tools::RunDiagnosticsTool` exactly), not this trait's 30-second default. Freeform code generation (needing a live `ResponseGenerator`) is deliberately *not* duplicated here -- `coding_agent::tools::GenerateCodeTool::new(generator)` already has a clear place for managing that instance; said explicitly as a scope decision, not silently missing.
- [x] **`security_agent::SecurityAgent`** wraps `tool_framework::ToolRegistry` lookups against real, already-assigned `PermissionLevel`s -- reports what a set of named tools is actually permissioned at (the same levels `ToolRouter`/`PermissionManager` genuinely enforce), not a heuristic or invented risk score (Ai_reasoning SS17's "must not fabricate completed actions" applied to risk assessments, not just tool results). `requires_confirmation_count` counts tools strictly above `Safe`, matching `permission_manager::SecurityRules`'s own default `auto_approve_at_or_below: Safe` exactly rather than a threshold invented for this phase. An unknown tool name is reported in `unknown_tools`, not silently dropped from the response.
- [x] **A real overclaim, found and corrected before it shipped**: this module's first doc comment draft described a "natural pairing" with `PlannerAgent` -- collect a plan's tool names, check their risk. Checking `planner::Planner::create_plan`'s actual source (not assumed from its intent) showed every `PlanStep::tool` is hardcoded `None`, even for `SingleTool` requests -- picking a *specific* tool out of a broad `Intent` category genuinely needs language understanding a keyword classifier can't honestly provide, the same reason `planner`'s own doc comment already defers multi-step decomposition to a working `llm-runtime`. The doc comment was rewritten before this phase shipped to say the pairing is real "in principle," not yet in practice -- this project's standing rule (verify, don't assume) caught its own draft documentation making exactly the kind of claim that rule exists to prevent.
- [x] **`integration-tests/tests/cross_crate_flows.rs`**: `research_memory_and_security_agents_cooperate_through_one_coordinator` -- three real agents (not two, to make the orchestration point clearly rather than minimally) cooperating through one shared `Coordinator` in a single find-a-file-then-remember-a-note-then-check-tool-risk scenario. This workspace's first proof the multi-agent architecture actually orchestrates multiple agents together in one flow, not merely that each agent works correctly in isolation the way `agents`' own crate-local tests already show.
- [x] 542/542 tests passing (+30 over Phase 24), 38 verified crates (+1) -- `agents` (29 unit tests; no `tests/permission_integration.rs` this phase, since no new `Tool`/`PermissionLevel` was introduced -- `SecurityAgent` *reports* existing levels, it doesn't add new gated tools of its own), `integration-tests` (+1: the three-agent cooperation flow). `cargo fmt --all -- --check` clean workspace-wide; `clippy --all-targets -- -D warnings` clean across all 38 verified crates; `cargo doc --no-deps` zero warnings, rechecked including `agents`.

## Phase 26 checklist

Development_plan's fourth "Future Phase": Document Indexing, PDF
Support, Knowledge Base, Local Search (Archd SS20 names "Retrieval-
Augmented Generation (RAG)" explicitly as a future architecture
extension). One new crate, `rag` -- a genuinely separate store from
`memory::MemoryStore`, built on the same proven persistence shape.

- [x] **Why `rag` is its own store, not an extension of `memory`**: DBDMS's own data model and Ai_reasoning SS9's "Memory Strategy" already distinguish "Long-Term Memory" (short, user-approved personal facts) from "Semantic Memory" ("embedded documents and notes") as different concerns, even though both end up persisted and searched by embedding similarity. `rag::store::KnowledgeBase` gets its own SQLite database and its own `vector_db::VectorIndex`, not a bolt-on to `MemoryStore` -- said explicitly in this crate's own doc comment as a considered decision, not left to be inferred from the fact that a second, similar-looking store exists.
- [x] **`chunking::chunk_text`**: real, deterministic word-count sliding-window chunking with configurable overlap. Deliberately not sentence/paragraph-aware -- that needs real language understanding to do honestly (abbreviations, decimal numbers, and quoted speech all break a naive `.`-based splitter), the same reasoning `planner`'s own doc comment already gives for deferring a different kind of language-dependent decision to a working `llm-runtime` rather than faking it with a heuristic.
- [x] **A wrong hand-derived count, caught before it shipped**: `long_text_splits_into_multiple_overlapping_chunks`'s first draft asserted 250 words at `max_words=100`/`overlap=20` produces 4 chunks. A quick standalone Python trace of the same loop showed 3 (the third window's end clamps to the real text length -- 160+100=260, but the text is only 250 words long -- one step earlier than a mental count suggests). The test's expected value was corrected to match the real, traced behavior before being trusted, not adjusted to make a wrong assumption pass.
- [x] **`pdf::extract_pdf_text`** wraps `pdf-extract`, pinned `=0.6.5` after empirically checking several versions against this sandbox's rustc 1.75 (0.7.0 through 0.7.4 all fail to resolve -- a `time` crate version needing `edition2024`, the same wall class `vision`'s `image` pin and `memory`'s `rusqlite` pin already document; 0.6.5 compiles and was then confirmed extracting real text correctly).
- [x] **A real gap found while building this module's own test fixture, not assumed away**: generating a test PDF via ImageMagick's `convert` (already a trusted dependency for `screenshot`/`ocr`) produces a PDF with *no extractable text layer at all* -- `convert` rasterizes the requested text into a bitmap and embeds that as an image, the same as scanning a printed page. Both `pdf-extract` and the system's own `pdftotext` (poppler-utils) correctly report empty output against it, confirmed independently before concluding the PDF itself was the problem, not either extraction tool. The real fixture instead is a minimal, hand-built, spec-correct PDF with a genuine text-showing content stream (`BT ... Tj ET`) and byte-exact `xref` offsets computed as it's assembled -- verified with `qpdf --check` and `pdftotext` independently before being trusted as this crate's own test input, the same "verify the fixture is real before testing against it" discipline `ocr`'s and `screenshot`'s fixtures already follow.
- [x] **`store::KnowledgeBase`**: real SQLite schema (`documents`/`chunks`/`chunk_embeddings`, mirroring `memory::MemoryStore`'s memories/embeddings split) plus the identical `RefCell<VectorIndex<Id>>`-mirror-rebuilt-on-open-and-kept-in-sync-on-writes pattern `MemoryStore` already proved out -- reused deliberately rather than re-derived, since it's already a validated approach in this exact workspace. `index_document` detects PDF vs. plain text by file extension; a zero-chunk result (a PDF with no text layer, confirmed to genuinely happen -- see above) indexes successfully rather than failing, matching `pdf::extract_pdf_text`'s own "empty is a real outcome" reasoning all the way up the call stack, not just at the extraction layer. `forget_document` deletes children before parents (`chunk_embeddings` -> `chunks` -> `documents`, matching `MemoryStore::forget`'s own ordering) and returns `false`, not an error, for an unknown id. `persists_across_reopening_the_same_file` closes and reopens a real on-disk SQLite file and confirms a real search still finds a previously indexed chunk -- persistence proven, not assumed from the schema alone.
- [x] Four new tools (new `ToolCategory::Development` members, alongside Phase 24's `coding-agent` tools): `search_knowledge_base`/`list_indexed_documents` are `ReadOnly` (no side effects, matching `search_memory`'s -- Phase 25's `memory_agent` -- and `extract_text_from_image`'s precedent). `index_document` is `Moderate`: reads a caller-named path like `read_file`, but also *creates new persistent data* (rows in the knowledge base, real storage that outlives the call) -- the same "user data modification" reasoning TCLIS SS6 already puts `write_file`/`create_file` at `Moderate` for. `forget_document` is `Destructive`, matching `delete_file`'s precedent for irreversible data loss -- a meaningfully higher-stakes action than indexing, deliberately not given the same level.
- [x] **`integration-tests/tests/cross_crate_flows.rs`**: `ocr_extracted_text_flows_into_the_knowledge_base_and_is_found_by_search` -- a genuine cross-*phase* flow (not just cross-crate within one phase, the way Phase 25's own three-agent test was): `ocr` (Phase 23) extracts real text from a real generated image, that text is written to a file, and `rag` (Phase 26) indexes and finds it through a real `search_knowledge_base` call. Proves a Phase 23 capability's output is genuinely consumable as Phase 26 input, not merely type-compatible on paper.
- [x] **A second real empirical finding, surfaced building that same test**: the first-attempt fixture (a tight 500x100 canvas) had real tesseract misread the leading "Pho" as "9" and clip the trailing "rgy" off "energy" -- genuine OCR imprecision at an image's edges, not a bug anywhere in this pipeline, and not something `ocr`'s own Phase 23 tests happened to hit (a shorter string, more margin relative to its length). Confirmed by hand with a quick side-by-side `tesseract` comparison, then fixed with a wider, better-margined canvas (700x150) that reads perfectly -- fixing the actual cause rather than loosening the test's assertion to tolerate a misread it didn't need to accept.
- [x] 569/569 tests passing (+27 over Phase 25), 39 verified crates (+1) -- `rag` (26: 21 unit + 5 permission-integration, this workspace's second crate after `coding-agent` to span three permission levels in one crate), `integration-tests` (+1: the OCR-to-RAG flow). `cargo fmt --all -- --check` clean workspace-wide; `clippy --all-targets -- -D warnings` clean across all 39 verified crates; `cargo doc --no-deps` zero warnings, rechecked including `rag`.

## Phase 27 checklist

Development_plan's fifth "Future Phase": MCP Client, MCP Server,
External Tool Integration (Apis SS19 and Archd SS20 both name MCP
explicitly as a future extensibility point). One new crate, `mcp`.

- [x] **Spec version decided empirically, not assumed**: two MCP revisions exist as of this phase (August 2026). 2026-07-28 is the actual current/latest, finalized three weeks before this phase was written, and replaces the stateful `initialize`/`initialized` handshake and session-ID model entirely with self-contained, stateless requests. Real, but by its own release notes barely adopted yet (official SDK support still beta at release), and explicitly designed so new clients fall back to the 2025-11-25 handshake for most of the deployed ecosystem for some time. Confirmed directly rather than trusted from documentation alone: `@modelcontextprotocol/server-filesystem` `2026.7.10`, a real, currently-published npm package, was installed and spawned for real; a genuine `initialize` call requesting `protocolVersion: "2025-11-25"` got back exactly that version, with real `tools/list`/`tools/call` round trips against a real file. That's the version the actually-installable ecosystem speaks today, so that's what this crate targets -- the same "confirm, don't assume, then document the reasoning" discipline already applied to the Rust 1.75 toolchain wall and the `pdf-extract`/`image`/`uuid` pins.
- [x] **`protocol.rs`**: JSON-RPC 2.0 envelope and MCP's lifecycle/tools types, shapes taken directly from the real fetched schema (`modelcontextprotocol/modelcontextprotocol` on GitHub, `schema/2025-11-25/schema.json`, raw content fetched and read, not recalled from training data). Two behaviors the schema's text alone didn't make obvious, both confirmed by manually probing the real reference server before being relied on: a *tool-level* failure comes back as an ordinary JSON-RPC result with `isError: true`, not a protocol-level `error` object -- only a genuinely unrecognized method produces one -- and a real server attaches extra fields (`execution`, `outputSchema`) this crate doesn't act on, preserved rather than rejected (`ServerCapabilities::other` via `#[serde(flatten)]`) so an unrecognized-but-legal field doesn't fail parsing.
- [x] **`transport.rs`**: line-delimited JSON-RPC framing (MCP's stdio transport: one `\n`-terminated message per line), generic over any `BufRead`/`Write` pair so the identical `LineTransport` serves three real cases -- a spawned child's pipes (`client`), this process's own stdin/stdout (`server::serve_stdio`), and an in-memory buffer in tests -- without a separate implementation for any of them. A bounded 16 MiB per-line cap exists specifically so a broken or hostile peer streaming data with no newline can't grow this process's memory without bound (TCLIS SS7 "resource limits").
- [x] **`client::McpClient`**: spawns a peer as a real subprocess and speaks the real lifecycle (`initialize` then `notifications/initialized`), then `list_tools`/`call_tool`. Synchronous by design, matching `tool_framework::Tool::execute`'s own non-async signature exactly -- the identical reasoning `network::HttpClient` (Phase 14) already used for the same constraint. State lives behind one `Mutex`, which doubles as the mechanism enforcing this phase's one deliberate concurrency limit: at most one request in flight per client at a time, matching how every real server tested against here is actually used in practice (a single local session, not pipelined concurrent calls).
- [x] **`server::serve`/`serve_stdio`**: exposes this assistant's own `ToolRouter` as a real MCP server. `tools/call` is routed through `ToolRouter::execute` -- the *exact same* validate/permission-check/timeout pipeline any locally-issued call already goes through (TCLIS SS1: the LLM never touches the OS directly except through this pipeline) -- so an external MCP client asking this assistant to run a tool is just another caller of that one pipeline, never a separate, less-guarded path. Bridges into the router's `async fn execute` from a synchronous read loop via a `tokio::runtime::Handle` passed in explicitly rather than fetched via `Handle::current()`, which also makes the whole loop directly unit-testable: a plain `#[test]` builds its own throwaway `Runtime`, hands `serve` the handle, and drives a full scripted multi-request exchange over an in-memory buffer with no subprocess involved at all.
- [x] **`bridge.rs` -- the "External Tool Integration" itself**: `to_json_schema`/`to_input_schema` convert between MCP's arbitrary JSON Schema and this workspace's closed `tool_framework::ParamKind`, honestly in both directions rather than silently. `ParamKind` gains an `Any` variant (matches any JSON value) for shapes the existing five variants can't represent -- nested objects, `enum`/`oneOf`/`anyOf`, typed arrays, a missing or multi-valued `"type"` -- directly precedented by Phase 8's own `StringArray` addition for the identical kind of gap; confirmed before adding it that every existing `ParamKind::` reference across the workspace is a constructor call, never an exhaustive match, so the blast radius was genuinely zero. Every declared remote property becomes a `ParamSchema` entry (as `Any` when nothing more specific fits) specifically so `InputSchema::validate` doesn't reject a legitimate remote-tool argument as `UnexpectedParam` -- a schema that instead relies on `additionalProperties` to accept properties it never declares is a named, undefended limitation, not a hidden one.
- [x] **Permission level, reasoned and proven, not just asserted**: every bridged tool registers at `PermissionLevel::Moderate` and `ToolCategory::Mcp` (new variant) regardless of what the remote tool's own `annotations` claim -- the real fetched schema's own field-level comment says a client should never make a tool-use decision from an untrusted server's hints, confirmed directly from the schema text rather than inferred, so this crate doesn't read them for trust purposes at all. Proven concretely in `tests/permission_integration.rs`, mirroring `rag`'s and `linux-services`'s own such files: a stub tool that's `Administrator` when called locally is only `Moderate` once bridged, and a `PermissionManager` configured to deny still blocks a bridged call end to end through the real router.
- [x] **Deliberately not built this phase, each with its own documented reasoning** (`lib.rs`/`bridge.rs`): a model-callable "connect to a new MCP server" tool -- `Tool::execute` takes only `&self` with no path back to the live `ToolRegistry` it's dispatched from, the same class of structural gap Phase 7 named explicitly rather than guessing around; Resources/Prompts/Sampling/Roots/Elicitation -- no existing concept in this workspace's architecture to bridge any of them into yet; Tasks -- marked experimental in 2025-11-25 itself and substantially reworked into an optional extension for 2026-07-28, the least stable part of either revision; Streamable HTTP transport -- stdio covers every real use case and every reference server actually tested against, and `LineTransport`'s generic reader/writer means adding HTTP later doesn't require touching the framing layer at all.
- [x] **Real subprocess-boundary testing, twice over, not simulated either time**: `src/bin/mcp-stub-server.rs` is a genuine second binary (Cargo builds it automatically alongside the library, zero non-Rust dependency) serving `tool-framework`'s existing stub tools over real stdio; `tests/client_server_roundtrip.rs` spawns it as an actual child process and drives a real `McpClient` against it over real OS pipes end to end -- handshake, listing, calling, a genuinely-enforced timeout carried all the way through a real MCP round trip, and the full discover-then-bridge-into-`ToolRegistry` path. Separately, `tests/real_reference_server_interop.rs` re-runs a version of the actual manual development-time probe against the real `@modelcontextprotocol/server-filesystem` package via `npx --yes` (a pinned version, fetched fresh rather than depending on the ad hoc install performed outside the repo during development), `#[ignore]`d so the default `cargo test` sweep stays free of any npm/node dependency the other 39 crates don't need -- run explicitly during this phase and confirmed genuinely passing, including confirming the real server's real response does carry `annotations`, so "this crate chooses not to trust them" is a real choice about real data actually observed, not a moot point about a field that never shows up.
- [x] 617/617 tests passing (+48 over Phase 26), 40 verified crates (+1) -- `mcp` (47: 36 unit + 7 real-subprocess round-trip + 4 permission-integration, plus 1 `#[ignore]`d real-npm-server test run and confirmed passing separately), `tool-framework` (+1: `ParamKind::Any` coverage). `cargo fmt --all -- --check` clean workspace-wide; `clippy --all-targets -- -D warnings` clean across all 40 verified crates; `cargo doc --no-deps` zero warnings, rechecked including `mcp`.

## Phase 28 checklist

Development_plan's sixth "Future Phase": Android App, Remote Control,
Notifications. One new crate, `remote-control`.

- [x] **"Android App" confirmed unbuildable in this sandbox, not assumed**: checked directly before deciding anything. `rustc --print target-list` lists `aarch64-linux-android` and friends, but `rustc --print sysroot` shows only `x86_64-unknown-linux-gnu` actually installed; `rustup` -- the only way to fetch another target's stdlib -- isn't available; Google's own distribution servers return a real `403` through this sandbox's egress proxy, not a timeout; and an actual `cargo build --target aarch64-linux-android` attempt failed exactly as that predicts (`can't find crate for std`). The same category of wall Phase 20 already hit and handled the same way for Flatpak (network wall) and Snap (no tooling): name it precisely, don't work around it, build what's genuinely real instead. That's a small, dependency-free Progressive Web App (`assets/index.html`, `app.js`, `style.css`, `manifest.json` -- no bundler, no npm, no framework, embedded into the binary via `include_bytes!`), installable to a phone's home screen on both Android and iOS through `manifest.json`'s `display: standalone`, and genuinely tested (`assets.rs`'s own tests plus the network round-trip test's static-asset checks) rather than a spec description of an APK this sandbox cannot produce.
- [x] **`pairing.rs` -- the actual security boundary of this whole crate**: a 6-digit pairing code defended by three things together, not one -- [`PAIRING_CODE_TTL`] (5 minutes), [`MAX_PAIRING_ATTEMPTS`] (5 wrong guesses invalidates the code outright, so the real guessable space per code is 5, not 1,000,000), and `server.rs`'s LAN-only restriction on the one route that can complete it (`is_local_network_address`, checked against real RFC 1918/loopback/link-local boundaries -- including the boundary *just outside* each range, e.g. `172.32.0.0` one `/12` block past `172.16.0.0/12` -- not a prefix-string check). Pairing only ever *starts* as a plain function call (`PairingManager::begin_pairing`), never an HTTP endpoint, so a remote caller who has never touched this machine has nothing to submit. Device tokens are 256 bits of OS-CSPRNG randomness (the same `getrandom`-backed source, pinned to the identical `=0.2.17` version, that `security::MasterKey::generate` already uses elsewhere in this workspace); only their SHA-256 hash is ever persisted (`DeviceStore`, SQLite, mirrors `memory::MemoryStore`'s open/in_memory shape) -- a deliberately simpler, one-way choice than `security::SecretManager`'s reversible AES-256-GCM encryption, since nothing here ever needs the raw token back, only to check whether a presented one matches it.
- [x] **`server.rs`**: a `tiny_http`-based HTTP/SSE server, verified against the pinned Rust 1.75 toolchain before use, the same empirical check every dependency in this workspace gets. `/tools/execute` is routed through `ToolRouter::execute` -- the *exact same* validate/permission-check/timeout pipeline any local caller already goes through -- so a paired remote device is granted no more trust than a local one, only a different transport to reach the identical gate. Deliberately synchronous at the transport layer (`tiny_http`, not an async server), bridging into the router's `async fn execute` via an explicitly-passed `tokio::runtime::Handle::block_on` -- the same idiom `mcp::server::serve` already established for the identical constraint in Phase 27, now used a second time.
- [x] **Permission integration, proven concretely, not just asserted**: `tests/permission_integration.rs` mirrors `rag`'s and `mcp`'s own such files -- a real `PermissionManager` configured to deny still blocks a bridged remote call end to end. The more interesting finding along the way: `SecurityRules::default()`'s "Administrator actions blocked unless explicitly enabled" (PRD SS6.6, "root-level actions must be blocked by default") applies exactly as much to the remote path as to a local one, discovered when a test that assumed an approving confirmation handler alone would be enough was itself wrong -- the default security policy still blocked it, correctly, until the test was fixed to explicitly opt in the same way `permission-manager`'s own tests do.
- [x] **Two real bugs found by the real client/server test, not caught by reasoning alone**: the route dispatcher initially matched against the full request URL *including its query string*, so `/events?token=...` 404'd on every attempt -- found because the network round-trip test actually exercises `/events`, not just the simpler routes. Separately, the SSE handler's raw HTTP preamble was written but never explicitly flushed, so it could sit in an internal buffer indefinitely while the client blocked waiting for headers that hadn't actually left the process -- found as a genuine hang in that same test (diagnosed with temporary, targeted instrumentation rather than guessed at), and fixed properly with `Transfer-Encoding: chunked` framing -- the standards-correct way to signal an unbounded HTTP/1.1 body -- not a workaround.
- [x] **`notify.rs` -- "Notifications", scoped honestly**: SSE-relayed live activity from the real `event_bus::EventBus`, delivered while a device is actively connected -- deliberately *not* OS-level push-while-the-app-is-closed, which in virtually every real implementation needs a cloud relay (FCM/APNs) that PRD SS3's own non-goal #3 already rules out for this assistant ("no cloud dependency for the core assistant"). Named here as a deliberate, local-first reading of "Notifications", not a silently lesser stand-in for the cloud version.
- [x] **`config::RemoteControlConfig`**: `enabled: false` and `bind_address: "127.0.0.1"` by default -- a listening network port is a real, standing surface change from every prior phase's tools (all reachable only from a process already running as the local user), so it must be an explicit opt-in, never on just because the crate exists. Backward-compatibility proven the same way every prior config addition has been (Phase 18's, Phase 23's, Phase 26's own such tests): a config file written before this phase still loads, with `remote_control.enabled` specifically confirmed to come back `false`, not merely "loads without error."
- [x] **Deliberately not built this phase, each with its own documented reasoning** (`lib.rs`): a model-callable "pair a new device" tool -- unlike `mcp`'s "no connect tool" gap (a structural limitation of `Tool::execute`'s signature), this one is a deliberate design stance, not a plumbing limitation: pairing is a security boundary a *human* crosses deliberately, never a decision an LLM should be able to make on its own initiative, even were the plumbing trivial; TLS -- this phase's server targets loopback-by-default or an explicitly-opted-into trusted LAN, a materially different threat model from a server exposed to the open internet, which would need it; wiring into `app` -- which doesn't yet depend on `tool-framework` itself, or on any other tool-providing crate (`search`, `rag`, `linux-services`, `mcp`, ...) either, so this phase doesn't reach ahead of that.
- [x] 665/665 tests passing (+48 over Phase 27), 41 verified crates (+1) -- `remote-control` (46: 30 unit + 12 real network round-trip over an actual TCP socket with a real HTTP client + 4 permission-integration), `config` (+2: `RemoteControlConfig` round-trip and backward-compatibility). `cargo fmt --all -- --check` clean workspace-wide; `clippy --all-targets -- -D warnings` clean across all 41 verified crates; `cargo doc --no-deps` zero warnings, rechecked including `remote-control`.

## Phase 29 checklist

Development_plan's seventh "Future Phase": Plugin Marketplace, SDK
Improvements, Templates, Code Generator. One new crate, `plugin-sdk`.

- [x] **Real architecture read before designing anything**: `plugin-manager` turns out to have two deliberately separate loading mechanisms -- the in-process `Plugin` trait (the one that actually produces usable tools) and a narrower `dlopen`-based `.so` loader (metadata-only by design, per that crate's own module docs). `plugin-sdk`'s generator targets the real, tool-providing mechanism rather than the narrower one, confirmed by reading `plugin.rs`/`dynamic.rs`/`manager.rs` directly rather than assumed from the crate's name alone.
- [x] **`catalog.rs` -- "Plugin Marketplace", honestly local**: no real, hosted marketplace exists for this from-scratch project to connect to yet, and PRD SS3's own non-goal ("no cloud dependency for the core assistant") already makes local-first the right default regardless of whether one existed. Reuses `PluginManager::discover_manifests` for the actual directory scan -- that function already exists and is already tested, so this crate adds a search/browse layer on top of it rather than a second, competing scanner. Adds one concrete, curated entry (`with_bundled_examples`): this workspace's own real, compiled `example-plugin` from Phase 11, proving the catalog represents something real end to end, not a fabricated example. A `CatalogSource::Remote` is named in this module's own docs as the natural next step once a real marketplace exists to query -- not built speculatively ahead of that, the same "name it, don't build it" reasoning `mcp`'s "no connect tool" and `remote-control`'s "no TLS" used for their own deferrals.
- [x] **`templates.rs`/`generator.rs` -- "Templates" and "Code Generator" together**: three real, checked-in template files under `templates/` (`Cargo.toml.template`, `manifest.toml.template`, `lib.rs.template`), rendered via plain `{{key}}` string substitution -- deliberately not a templating-engine dependency, since three files and a dozen placeholders between them don't need conditionals or partials. `generate_plugin` produces a complete, three-file plugin crate implementing the real `plugin_manager::Plugin` trait with a working example `tool_framework::Tool`, refusing to silently overwrite an existing target directory rather than clobbering whatever's already there.
- [x] **Proven with a real `cargo build`/`cargo test`, not just inspected as text** (`tests/generated_plugin_compiles.rs`): the same standard `coding-agent`'s own scratch-crate tests already set for "a tool that produces Cargo output must be proven against a real `cargo` invocation." Generates directly under this workspace's real `crates/` (not `/tmp`) specifically so the generated `Cargo.toml`'s relative path dependencies (`../common`, `../tool-framework`, `../plugin-manager`) resolve exactly the way a real plugin author's would; the generated crate's own `[workspace]` table keeps it fully independent of the main workspace's `Cargo.lock` and `target/` despite living inside its directory tree.
- [x] **A real bug found empirically, not anticipated**: the first attempt at this test failed with the exact same `edition2024` wall this workspace has pinned around repeatedly elsewhere (rusqlite, image, uuid, getrandom, zeroize, idna_adapter) -- `indexmap` this time, resolved fresh because the generated crate's own independent `[workspace]` means it doesn't inherit the main workspace's careful pins. Pinned in `templates/Cargo.toml.template` to the exact version the main workspace's own `Cargo.lock` already proves works, documented with the same "confirm, don't assume" reasoning as every other pin in this workspace.
- [x] **A real test-cost finding, not just a correctness one**: two generated plugins each doing a from-scratch dependency build (serde, tokio, toml, chrono, ...) was slow enough to matter -- sharing one `CARGO_TARGET_DIR` across both real `cargo` invocations (Cargo's own build cache keys on crate/version/features/profile, not which top-level workspace asked for it) cut this test file's total real-`cargo`-invocation time substantially without changing what's actually being proven: two separate, real `cargo build` runs against two separately generated plugins. Documented as a deliberately persistent cache (the same reasoning `~/.cargo/registry` is never wiped automatically), with a `.gitignore` entry (`crates/_generated_*/`) as a safety net alongside the per-test `Drop`-guard cleanup that handles the normal case.
- [x] **`lint.rs` -- "SDK Improvements"**: a second, deliberately *soft* validation layer on top of `PluginManifest::from_toml`'s hard parse requirements -- flags a non-reverse-DNS `id` (Plugins.md SS5's own convention, not something the manifest parser enforces), a non-semver `version` (the same format `is_compatible_with_sdk` already expects internally), an empty `description`, and a `supported_platforms` list that will always fail `PluginManager::validate` on this OS. Warns rather than rejects throughout -- none of these are things `PluginManager` itself currently checks or acts on, so a strict linter claiming otherwise would overstate its own authority.
- [x] **Deliberately not built this phase, each with its own documented reasoning** (`lib.rs`): a remote catalog source (no real marketplace exists to query yet); templates for the UI/Theme/Workflow/Memory/Voice plugin categories Plugins.md SS7 lists (only the Tool Plugin category has a fully real, proven extension point -- `tool_framework::Tool`, wired through `PluginManager` since Phase 11 -- to generate genuinely working code against; a template for a plugin type this workspace can't actually load would be exactly the "looks right, does nothing" gap this project avoids elsewhere); a `cargo generate`-style CLI entry point (`app` doesn't yet depend on `plugin-manager` or any other tool-providing crate either, the same gap `mcp`'s and `remote-control`'s own phases already named rather than reached past).
- [x] 695/695 tests passing (+30 over Phase 28), 42 verified crates (+1) -- `plugin-sdk` (30: 28 unit + 2 real generate-then-build/test integration, covering both the default template and a custom `PermissionLevel`). `cargo fmt --all -- --check` clean workspace-wide; `clippy --all-targets -- -D warnings` clean across all 42 verified crates; `cargo doc --no-deps` zero warnings, rechecked including `plugin-sdk`.

## Phase 30 checklist

Development_plan's eighth and final named phase: Deep Linux
Integration, LionOS Integration, System-wide Assistant, Desktop
Automation, Native AI APIs, Full OS Control Layer. One new crate,
`os-integration`.

- [x] **LionOS Integration: named, and refused with full reasoning, not silently skipped.** Confirmed by checking this project's own documents directly rather than assumed: PRD SS2 goal 7, Archd SS18, and DIG SS19 all describe LionOS integration as a *future* aspiration; none of PRD, Archd, TDS, Apis, DBDMS, TCLIS, Ai_reasoning, DIG, UI-UX, Plugins, or Mms specifies any real LionOS API, IPC mechanism, or protocol to build against. The full reasoning for why this is refused outright -- not attempted, not stubbed, not represented by a speculatively-named extension point -- lives in `os-integration/src/lib.rs`'s own crate-level docs, on the view that for this one item the reasoning *is* the deliverable.
- [x] **"Full OS Control Layer": declined as written, reframed honestly.** Read literally this contradicts PRD SS3's first two non-goals ("no unrestricted autonomous root access", "no fully general 'run anything' shell control without safety checks") and everything every phase since Phase 9 has built toward. What "full" means in this crate instead: the broadest tool coverage this project ships, with every tool still passing through the exact same `ToolRouter::execute` pipeline as the first tool this workspace ever registered -- broader coverage, not broader trust, stated explicitly rather than left for a reader to infer.
- [x] **`windows.rs` -- Desktop Automation, confirmed working before implementation began**: real `wmctrl`/`xdotool` window management. Before writing any code, the full stack (Xvfb + a real if minimal window manager, `fluxbox`, + a real client window, `xterm`) was started by hand and genuinely list/find/activate/move/resize/minimize/close was confirmed to work end to end -- the same empirical-first discipline this project has followed since Phase 1, applied to a capability that turned out to be more real than the outset expected. `close_window` deliberately sends a polite `_NET_CLOSE_WINDOW` (`wmctrl -c`), not a forceful kill, so a well-behaved application gets the same chance to prompt for unsaved work a normal user-initiated close would trigger; a force-kill tool is deliberately not included, a named limitation rather than something routed around with a blunter instrument by default.
- [x] **A real parsing bug, caught by its own test**: `wmctrl -l`'s column-aligned output right-pads with multiple spaces for alignment; an early `parse_wmctrl_line` used `splitn(4, char::is_whitespace)`, which treats *each individual space* as its own split point, consuming the entire split budget on the padding before ever reaching the title. Caught immediately by `parses_a_real_wmctrl_line_with_a_multi_word_title`, fixed with `split_whitespace` (which correctly treats a run of whitespace as one separator) followed by rejoining the remaining tokens for the title.
- [x] **`systemd.rs` -- Deep Linux Integration, verification honestly partial**: real `systemctl --user` service management (list-units, show, start, stop). Confirmed directly, not assumed, that this sandbox is not booted with systemd as PID 1 (`ps -p 1` reports `process_api`, this sandbox's own supervisor), so there is no session bus for the happy path to reach at all. The exact real failure text -- `"Failed to connect to bus: No medium found"`, a clean, immediate, catchable error rather than a hang -- is captured and tested against directly in every function's own test, proving this module's error handling is real and correct even where the happy path can't be verified here. The same category of gap as Phase 20's Flatpak (network wall) and Snap (no tooling), and Phase 28's Android SDK (no installed target, no rustup, blocked distribution servers) -- named with the same precision, not glossed over.
- [x] **`dbus_api.rs` -- Native AI APIs, dependency choice made empirically**: `zbus` 4.x was tried first and rejected only after hitting a real, cascading `edition2024` dependency chain (three separate `toml`-ecosystem crates needing pins in a row, a different signature than this workspace's usual "one pin fixes it" pattern); `zbus` 3.x was tried next and rejected after real feature-flag issues under a minimal build. `dbus` + `dbus-crossroads` were adopted only once a genuine client/server round trip over a real session bus was confirmed working by hand, the same standard every dependency choice in this workspace has been held to. `ExecuteTool` routes through the same `ToolRouter::execute` pipeline as `mcp`'s and `remote-control`'s own transports -- a D-Bus caller gets no more trust than any other one, only a different door to the identical gate.
- [x] **A genuinely subtle bug, found by the real test suite and root-caused, not worked around**: an early version of `dbus_api_integration.rs` started five independent `dbus-daemon` session buses (one per test) and used `Connection::new_session()` (which reads the ambient `$DBUS_SESSION_BUS_ADDRESS`) to connect to each. Every test after the first failed -- and critically, still failed under `--test-threads=1`, which ruled out a Rust-level env-var race between concurrently-running tests before it was ever blamed on one. Traced to `libdbus` itself caching session-bus connection state process-wide on the first `dbus_bus_get`-style lookup, independent of later env-var changes within the same process. Fixed at the actual cause: `Connection::new_address` connects to an explicit address directly, with no ambient or cached state involved, and every connection in the test suite (and nowhere in the production `dbus_api.rs` code itself, which always took a pre-built `Connection` as a parameter and never needed to change) now uses it.
- [x] **`assets/ai-assistant.service` -- System-wide Assistant**: a real systemd user-service unit file. `Restart=on-failure` with a 5-second backoff, deliberately not `Restart=always` -- a crash loop should surface as an inspectable failed unit a person can `systemctl --user status` to actually see, not hide behind infinite silent retries.
- [x] **Nine new tools (`tools.rs`), each permission level reasoned explicitly rather than picked by default**: `list_windows`/`list_user_services`/`user_service_status` at `ReadOnly` (pure queries); `activate_window`/`move_window`/`resize_window` at `Safe` (a deliberate judgment call, stated in the module's own docs, that a window's on-screen position is transient UI state closer in spirit to "open application" than to TCLIS SS6 Level 2's file-move example); `close_window`/`start_user_service`/`stop_user_service` at `Moderate` (real, if generally recoverable, consequences -- lost unsaved work or a stopped background service -- worth requiring confirmation for rather than auto-approving). Proven against a real `PermissionManager` in `tests/permission_integration.rs`, including the specific, non-obvious property that `close_window`'s `Moderate` gate blocks the call *inside* `ToolRouter::execute`, before `wmctrl` is ever invoked -- provably true without needing a real display at all, since the underlying command is simply never reached in the denied case.
- [x] 731/731 tests passing (+36 over Phase 29), 43 verified crates (+1) -- `os-integration` (36: 21 unit + 5 real D-Bus round-trip over a real session bus + 5 permission-integration + 5 real window-management round-trip over a real Xvfb+fluxbox+xterm stack). `cargo fmt --all -- --check` clean workspace-wide; `clippy --all-targets -- -D warnings` clean across all 43 verified crates; `cargo doc --no-deps` zero warnings, rechecked including `os-integration`.

---

## Closing note: Development_plan's 30 phases, in full

Every phase from 1 through 30 -- the complete roadmap as originally
laid out -- has now been attempted. The pattern held the same way
across all thirty: confirm against something real before claiming it
works, write the code, prove it with a real test wherever a real test
was possible, and when it genuinely wasn't -- a toolchain wall, a
sandbox limitation, a target that doesn't exist yet, a request that
contradicted this project's own stated values -- say so precisely,
with the exact error text or the exact document section that made the
call, rather than quietly working around it or quietly not mentioning
it at all.

That discipline caught real, concrete problems no amount of reasoning
in the abstract would have: an MCP preamble flush that caused a
genuine hang (Phase 27), a route dispatcher that silently dropped a
query string (Phase 28), a `wmctrl` line parser that consumed its own
split budget on column padding (Phase 30), a `libdbus` connection
cache that broke every test after the first for reasons that had
nothing to do with the Rust code being tested (Phase 30) -- and it
named real limits just as plainly: Rust 1.75 as a hard toolchain
floor, Flathub's network wall, no Android SDK reachable from this
sandbox, no systemd as PID 1 here, and, this phase, a LionOS that does
not exist anywhere yet to integrate with.

43 verified crates. 731 passing tests. Zero clippy warnings. Zero doc
warnings. A clean-room-verified tarball at every phase boundary. What
comes next -- a Stable release under DIG SS17's own separate
criteria, or a new roadmap once LionOS is real enough to have one --
is a decision for Lion to make, not one this document presumes to make
for them.
