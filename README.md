# Local Linux AI Assistant

A local-first AI assistant for Linux desktops: chat interface, safe system
control through an approved tool layer, memory, and (later) voice. Built in
Rust for performance and safety, with Slint for the UI and a local model
(Gemma 3 1B by default) for reasoning.

Full specs (PRD, Architecture, API, Database, Tool/Linux Integration, AI
Reasoning, Developer Implementation Guide, UI/UX, Plugin SDK, Model Runtime,
Roadmap) live in the project's planning documents. `docs/ROADMAP.md` tracks
build status phase by phase.

**Status:** Phase 1 (Foundation) in progress. No UI, LLM, or Linux tool
execution yet -- see `docs/ROADMAP.md`.

## Workspace layout

```
crates/
  common/      shared types + central error system (AppError/AppResult)
  utilities/   small dependency-light helpers
  config/      TOML config loading & saving
  logger/      tracing/tracing-subscriber init
  event-bus/   broadcast-based pub/sub event bus
  app/         binary entry point (bin name: ai-assistant)
config/
  default.toml default configuration loaded at startup
```

Later phases add `ui` (Slint), `assistant-core`, `llm-runtime`, `memory`,
`tool-framework`, `permission-manager`, `linux-services`, `plugin-manager`,
and more, per the full project structure.

## Building

Requires a Rust toolchain. This repo currently targets **edition 2021**
since it was scaffolded and tested against Rust 1.75 (the version available
via `apt` on Ubuntu 24.04 in the build sandbox). If you install a current
stable Rust via [rustup](https://rustup.rs) (recommended for real
development -- 1.85+), you can bump `edition` to `"2024"` in the root
`Cargo.toml` and run `cargo update` to pick up newer dependency versions.

```bash
cargo build --workspace
cargo test --workspace
cargo run -p app
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
