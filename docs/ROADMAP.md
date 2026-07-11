# Roadmap & Phase Status

Full detail lives in the project's `Development_plan` document. This file
tracks build status per phase as the code lands, so it's always clear what
actually exists versus what's still planned.

| Phase | Name | Status |
|---|---|---|
| 1 | Foundation | 🟢 Done -- workspace, `common`, `config`, `logger`, `event-bus`, `app` stub. CI/build scripts done. Fully verified (build+test+run) in-sandbox. |
| 2 | UI Framework (Slint) | 🟡 Written, NOT compile-verified in this sandbox -- see note below. Main window, theme (dark/light), sidebar nav, chat bubbles, composer done. |
| 3 | Assistant Core | ⬜ Not started |
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
