# Local Linux AI Assistant

A local-first AI assistant for Linux desktops: chat interface, safe system
control through an approved tool layer, memory, and (later) voice. Built in
Rust for performance and safety, with Slint for the UI and a local model
(Gemma 3 1B by default) for reasoning.

Full specs (PRD, Architecture, API, Database, Tool/Linux Integration, AI
Reasoning, Developer Implementation Guide, UI/UX, Plugin SDK, Model Runtime,
Roadmap) live in the project's planning documents. `docs/ROADMAP.md` tracks
build status phase by phase.

**Status:** Phase 17 (UI Polish) written; mostly not compile-verified, by
explicit choice -- see `docs/ROADMAP.md`'s Phase 17 notes for a much
deeper investigation of the Slint blocker than earlier phases documented,
and why it's a bigger wall than the dependency-pin fixes that resolved
Phases 14/16. Phases 1, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, and 16
are fully verified (build+test+run); Phase 17's `config::UiConfig`
additions join that list (real, compiled, tested Rust with no Slint
dependency) while everything touching `.slint` files or `ui/src/`
remains unverified, same as Phase 2 always has been.

## Workspace layout

```
crates/
  common/           shared types + central error system (AppError/AppResult)
  utilities/        small dependency-light helpers
  config/           TOML config loading & saving
  logger/           tracing/tracing-subscriber init
  event-bus/        broadcast-based pub/sub event bus
  conversation/      message + conversation model
  session-manager/  in-memory session tracking
  context-manager/  keeps history within a character budget
  prompt-builder/   formats history into a model-specific prompt (Gemma template)
  assistant-core/   orchestrates the above behind a swappable ResponseGenerator
  model-registry/   filesystem-based discovery of installed models (no ML deps)
  llm-runtime/      Candle-based Gemma 3 GGUF backend (real API, unverified build)
  vector-db/        brute-force cosine-similarity vector search
  embeddings/       text-to-vector embedding behind a swappable trait
  memory/           SQLite-backed session persistence + long-term memory + search
  reasoning/        intent + reasoning-level classification (rule-based)
  planner/          dependency-graph plans with cycle detection
  tool-framework/   tool registry, router, validation, timeout enforcement (stub tools; real ones are Phase 8)
  linux-services/   files/system/processes/terminal/packages/network (real); clipboard/audio/notifications/display/applications (real code, graceful failure here); power (implemented, never auto-executed)
  permission-manager/ real Permission Manager: policy, confirmation, remembered decisions, audit log
  workflow-engine/  named, persisted workflows over real tool calls: builder, scheduler, pause/resume/cancel, executor
  plugin-manager/   plugin manifests, lifecycle, permission policy; in-process Plugin trait (primary) + real dlopen loading (metadata only)
  example-plugin/   a real cdylib plugin.so proving plugin-manager's dynamic loader actually works
  voice/            TTS (espeak-ng, real) / STT (pocketsphinx, real pipeline) / VAD (energy-based) / WAV I/O
  search/           file/memory/command search + a Global Search aggregator, exposed as real ToolCategory::Search tools
  network/          HttpClient (ureq + system TLS trust store), streaming downloads, a std-only mock server for tests
  updater/          version checking + update downloads against a manifest URL (check_for_updates, download_update tools) -- never auto-installs
  metrics/          counters/gauges/timers registry + a PerformanceMonitor sampling real CPU/memory/uptime via linux-services
  telemetry/        strictly opt-in usage telemetry -- double-gated (enabled + endpoint), a closed non-identifying event vocabulary, disabled unless explicitly turned on
  diagnostics/      a global panic hook writing local crash reports + a get_diagnostics_report tool tying metrics/telemetry/crashes together
  security/         AES-256-GCM encryption, Argon2 key derivation, an encrypted SQLite secret store -- no tools exposed on purpose, see the crate's docs
  sandbox/          rlimit-based process resource limits (verified enforced) + a Landlock filesystem sandbox (written; enforcement unverifiable in this sandbox, see docs/ROADMAP.md)
  ui/               Slint desktop shell: theme, sidebar, chat view, composer, animations, accessibility (Phase 17)
  app/              binary entry point (bin name: ai-assistant)
config/
  default.toml default configuration loaded at startup
data/
  database/    assistant.db (SQLite, created on first run, gitignored)
```

Later phases (17+: UI polish, performance, testing, and on) add more,
per the full project structure in `Development_plan`.

## Building

Requires a Rust toolchain. **Get current stable Rust via
[rustup](https://rustup.rs) (1.85+)** -- this repo was scaffolded in a
sandbox limited to `apt`'s Rust 1.75 with no path to rustup's servers, so
Phase 2 (`ui`, and `app` which now depends on it) was written but not
compiled there. On a real machine with rustup this should just build; see
`docs/ROADMAP.md` for the full explanation if it doesn't on the first try.

```bash
cargo build --workspace
cargo test --workspace
cargo run -p app
```

Phase 1, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, and 16 together (no
Slint, no Candle) are fully verified and work even on old toolchains:

```bash
cargo build -p common -p utilities -p config -p logger -p event-bus \
  -p conversation -p session-manager -p context-manager -p prompt-builder \
  -p assistant-core -p model-registry -p vector-db -p embeddings -p memory \
  -p reasoning -p planner -p tool-framework -p linux-services \
  -p permission-manager -p workflow-engine -p plugin-manager -p example-plugin \
  -p voice -p search -p network -p updater -p metrics -p telemetry -p diagnostics \
  -p security -p sandbox
cargo test -p common -p utilities -p config -p logger -p event-bus \
  -p conversation -p session-manager -p context-manager -p prompt-builder \
  -p assistant-core -p model-registry -p vector-db -p embeddings -p memory \
  -p reasoning -p planner -p tool-framework -p linux-services \
  -p permission-manager -p workflow-engine -p plugin-manager -p example-plugin \
  -p voice -p search -p network -p updater -p metrics -p telemetry -p diagnostics \
  -p security -p sandbox
```

`voice`'s tests shell out to `espeak-ng`, `pocketsphinx_continuous`, and
`sox` -- install them first (`apt install espeak-ng pocketsphinx
pocketsphinx-en-us sox libsox-fmt-all` on Debian/Ubuntu) or those
specific tests will fail with "not available" errors rather than being
skipped.

`linux-services`' `terminal` module only ever runs commands from a fixed
allowlist (see `crates/linux-services/src/terminal.rs`), never an
arbitrary shell string. `power` (lock/suspend/restart/shutdown) is
implemented but not registered as an automated tool anywhere in this
codebase -- see `docs/ROADMAP.md` Phase 8 for why. Destructive tools
(`delete_file`) now go through `permission-manager`'s real confirmation
flow -- see `crates/linux-services/tests/permission_integration.rs` for
the full pipeline exercised end to end.

To use a real model once you're building `llm-runtime`/`app` on a modern
toolchain: download a Gemma 3 1B GGUF file and its `tokenizer.json`, name
them `gemma-3-1b.gguf` and `gemma-3-1b.tokenizer.json`, and put both in
`models/gemma/` (or wherever `config.model.model_path` points). `app`
picks them up automatically; without them it falls back to a stub backend
with a logged warning rather than failing.

## Development

```bash
cargo fmt --all              # format
cargo clippy --workspace --all-targets -- -D warnings   # lint
cargo test --workspace       # unit tests
```

Coding standards (file size limits, naming conventions, error handling,
testing strategy) are defined in the Developer Implementation Guide.

## License

MIT OR Apache-2.0 (add `LICENSE-MIT` / `LICENSE-APACHE` files with your
chosen license text before publishing).
