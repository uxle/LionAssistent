# Changelog

All notable changes to this project are documented here. Format loosely
follows [Keep a Changelog](https://keepachangelog.com/); this project uses
Semantic Versioning once it reaches 0.1.0+ stability milestones.

## [Unreleased]

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
