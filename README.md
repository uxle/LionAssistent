# Local Linux AI Assistant

A local-first AI assistant for Linux desktops: chat interface, safe system
control through an approved tool layer, memory, and (later) voice. Built in
Rust for performance and safety, with Slint for the UI and a local model
(Gemma 3 1B by default) for reasoning.

Full specs (PRD, Architecture, API, Database, Tool/Linux Integration, AI
Reasoning, Developer Implementation Guide, UI/UX, Plugin SDK, Model Runtime,
Roadmap) live in the project's planning documents. `docs/ROADMAP.md` tracks
build status phase by phase.

**Status:** Phase 8 (Linux Integration) done and fully verified. Phases 1,
3, 5, 6, 7, and 8 are fully verified (build+test+run). Phases 2 (UI) and 4
(LLM runtime) are written but not compile-verified in the sandbox this was
built in -- see `docs/ROADMAP.md` for exactly why (two unrelated
toolchain/network limits, not code problems) and what to do about it.

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
  ui/               Slint desktop shell: theme, sidebar, chat view, composer
  app/              binary entry point (bin name: ai-assistant)
config/
  default.toml default configuration loaded at startup
data/
  database/    assistant.db (SQLite, created on first run, gitignored)
```

Later phases add `planner`, `reasoning`, `tool-framework`,
`permission-manager`, `linux-services`, `plugin-manager`, and more, per the
full project structure.

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

Phase 1, 3, 5, 6, 7, and 8 together (no Slint, no Candle) are fully
verified and work even on old toolchains:

```bash
cargo build -p common -p utilities -p config -p logger -p event-bus \
  -p conversation -p session-manager -p context-manager -p prompt-builder \
  -p assistant-core -p model-registry -p vector-db -p embeddings -p memory \
  -p reasoning -p planner -p tool-framework -p linux-services
cargo test -p common -p utilities -p config -p logger -p event-bus \
  -p conversation -p session-manager -p context-manager -p prompt-builder \
  -p assistant-core -p model-registry -p vector-db -p embeddings -p memory \
  -p reasoning -p planner -p tool-framework -p linux-services
```

`linux-services`' `terminal` module only ever runs commands from a fixed
allowlist (see `crates/linux-services/src/terminal.rs`), never an
arbitrary shell string. `power` (lock/suspend/restart/shutdown) is
implemented but not registered as an automated tool anywhere in this
codebase -- see `docs/ROADMAP.md` Phase 8 for why.

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
