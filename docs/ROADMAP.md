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
| 4 | LLM Runtime | ⬜ Not started |
| 5 | Memory | ⬜ Not started |
| 6 | Planner & Reasoning | ⬜ Not started |
| 7 | Tool Framework | ⬜ Not started |
| 8 | Linux Integration | ⬜ Not started |
| 9 | Permission System | ⬜ Not started |
| 10 | Workflow Engine | ⬜ Not started |
| 11 | Plugin SDK | ⬜ Not started |
| 12 | Voice | ⬜ Not started |
| 13 | Search | ⬜ Not started |
| 14 | Networking | ⬜ Not started |
| 15 | Diagnostics | ⬜ Not started |
| 16 | Security | ⬜ Not started |
| 17 | UI Polish | ⬜ Not started |
| 18 | Performance | ⬜ Not started |
| 19 | Testing | ⬜ Not started |
| 20 | Packaging | ⬜ Not started |
| 21 | Documentation | ⬜ Not started |
| 22 | Stable Release (v1.0) | ⬜ Not started |

Phases 23+ (Vision, Coding Agent, Multi-Agent, RAG, MCP, Mobile Companion,
Developer Platform, LionOS integration) are post-v1.0 and not tracked here yet.

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
