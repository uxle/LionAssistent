# Local Linux AI Assistant

A local-first AI assistant for Linux desktops: chat interface, safe system
control through an approved tool layer, memory, and (later) voice. Built in
Rust for performance and safety, with Slint for the UI and a local model
(Gemma 3 1B by default) for reasoning.

Full specs (PRD, Architecture, API, Database, Tool/Linux Integration, AI
Reasoning, Developer Implementation Guide, UI/UX, Plugin SDK, Model Runtime,
Roadmap) live in the project's planning documents. `docs/ROADMAP.md` tracks
build status phase by phase.

**Status:** Phase 3 (Assistant Core) done and fully verified. Phase 2 (UI)
is written but not compile-verified in the sandbox this was built in --
see `docs/ROADMAP.md` for exactly why and what to do about it. No real LLM
yet (Phase 4) -- assistant-core runs on a stub backend that proves the
pipeline works without faking a real reply.

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
  ui/               Slint desktop shell: theme, sidebar, chat view, composer
  app/              binary entry point (bin name: ai-assistant)
config/
  default.toml default configuration loaded at startup
```

Later phases add `llm-runtime`, `memory`, `tool-framework`,
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

Phase 1 and Phase 3 together (no Slint) are fully verified and work even
on old toolchains:

```bash
cargo build -p common -p utilities -p config -p logger -p event-bus \
  -p conversation -p session-manager -p context-manager -p prompt-builder -p assistant-core
cargo test -p common -p utilities -p config -p logger -p event-bus \
  -p conversation -p session-manager -p context-manager -p prompt-builder -p assistant-core
```

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
