# Local Linux AI Assistant

A local-first AI assistant for Linux desktops: chat interface, safe system
control through an approved tool layer, memory, and (later) voice. Built in
Rust for performance and safety, with Slint for the UI and a local model
(Gemma 3 1B by default) for reasoning.

Full specs (PRD, Architecture, API, Database, Tool/Linux Integration, AI
Reasoning, Developer Implementation Guide, UI/UX, Plugin SDK, Model Runtime,
Roadmap) live in the project's planning documents. `docs/ROADMAP.md` tracks
build status phase by phase.

**Status:** Phase 2 (UI Framework) written. Phase 1 is fully verified
(build+test+run). Phase 2 could not be compile-verified in the sandbox
this was built in -- see `docs/ROADMAP.md` for exactly why and what to do
about it. No LLM or Linux tool execution yet.

## Workspace layout

```
crates/
  common/      shared types + central error system (AppError/AppResult)
  utilities/   small dependency-light helpers
  config/      TOML config loading & saving
  logger/      tracing/tracing-subscriber init
  event-bus/   broadcast-based pub/sub event bus
  ui/          Slint desktop shell: theme, sidebar, chat view, composer
  app/         binary entry point (bin name: ai-assistant)
config/
  default.toml default configuration loaded at startup
```

Later phases add `assistant-core`, `llm-runtime`, `memory`,
`tool-framework`, `permission-manager`, `linux-services`, `plugin-manager`,
and more, per the full project structure.

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

Phase 1 alone (no Slint) is fully verified and works even on old
toolchains:

```bash
cargo build -p common -p utilities -p config -p logger -p event-bus
cargo test -p common -p utilities -p config -p logger -p event-bus
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
