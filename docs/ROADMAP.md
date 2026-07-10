# Roadmap & Phase Status

Full detail lives in the project's `Development_plan` document. This file
tracks build status per phase as the code lands, so it's always clear what
actually exists versus what's still planned.

| Phase | Name | Status |
|---|---|---|
| 1 | Foundation | 🟡 In progress -- workspace, `common`, `config`, `logger`, `event-bus`, `app` stub done. CI/build scripts done. |
| 2 | UI Framework (Slint) | ⬜ Not started |
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
