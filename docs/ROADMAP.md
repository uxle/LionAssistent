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
