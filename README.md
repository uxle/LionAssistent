# Local Linux AI Assistant

A local-first AI assistant for Linux desktops: chat interface, safe system
control through an approved tool layer, memory, and (later) voice. Built in
Rust for performance and safety, with Slint for the UI and a local model
(Gemma 3 1B by default) for reasoning.

Full specs (PRD, Architecture, API, Database, Tool/Linux Integration, AI
Reasoning, Developer Implementation Guide, UI/UX, Plugin SDK, Model Runtime,
Roadmap) live in the project's planning documents. `docs/ROADMAP.md` tracks
build status phase by phase. `docs/API.md`, `docs/DEVELOPER_GUIDE.md`,
`docs/SDK_GUIDE.md`, and `docs/USER_GUIDE.md` (Phase 21) are the
codebase-accurate, actively-maintained counterparts to those planning
documents -- start there for anything practical. `docs/RELEASE_NOTES.md`
and `docs/PERFORMANCE_REVIEW.md` (Phase 22) are the honest, evidence-based
assessment of where this project actually stands -- read
`docs/RELEASE_NOTES.md` first if you're deciding whether to build on this.

**Status:** Phase 25 (Multi-Agent System) done -- `agents` is real,
compiled, tested Rust: a `Coordinator`/`AgentMessage` dispatch layer
(mirrors `ToolRouter`'s pipeline one layer up) plus five concrete
agents, each wrapping an already-real crate; 542/542 workspace tests,
38 verified crates. `ResearchAgent` is explicitly local-only (no
web search anywhere in this workspace, per PRD SS3's no-cloud-dependency
non-goal). Phase 24 (Coding Agent) is done before that -- `coding-agent`
is real, compiled, tested Rust with no unverified half; 512/512 tests,
37 crates. `run_diagnostics` is this workspace's first `Destructive`-
level tool -- a real `build.rs`-executing scratch crate proved the
permission gate actually stops it from running, not just returns a
denial (`integration-tests/tests/security_adversarial.rs`). Phase 23
(Vision Support) is done before that -- `screenshot`, `ocr`, and
`vision` are real, compiled, tested Rust with no Slint/Candle dependency;
`vision-runtime` (a Candle-based CLIP backend) joins `llm-runtime`/`ui`/
`app` as written but unverified in this sandbox. Phase 22 (Stable
Release) is done before that -- and deliberately **not** tagged v1.0.0;
see `docs/RELEASE_NOTES.md` for exactly why, and for an honest assessment
against PRD SS18's Definition of Done (two of six criteria fully met,
two met at the library level but not end-to-end, two not met -- the app
itself has never run). That phase also closed a real, previously-
documented gap: Phase 16's Landlock filesystem sandbox is now actually
wired into `app`'s startup (verified via a throwaway harness, the
established technique for code that touches an otherwise-uncompilable
crate), and added a data-backed `docs/PERFORMANCE_REVIEW.md` checking
every measurable target from TDS/DBDMS against real benchmark numbers.
The Phase 22 commit also ships as branch `lts/0.1.0-foundation`, a
stable reference point for whenever a modern toolchain unblocks Slint and
Candle. Phases 1, 3, 5, 6, 7,
8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 19, 20, 21, 22, 23 (`screenshot`/
`ocr`/`vision` only), 24, and 25 are fully verified (build+test+run);
Phase 17's `config::UiConfig` additions join that list (real, compiled,
tested Rust with no Slint dependency) while everything touching `.slint`
files or `ui/src/` remains unverified, same as Phase 2 always has been.

## Workspace layout

```
docs/               API.md, DEVELOPER_GUIDE.md, SDK_GUIDE.md, USER_GUIDE.md, ROADMAP.md (Phase 21)
LICENSE-MIT, LICENSE-APACHE   dual MIT/Apache-2.0 license text (Phase 20)
assets/
  icons/            ai-assistant.svg placeholder icon + rendered PNGs (16-512px)
packaging/
  shared/           .desktop entry + AppStream metainfo, reused by every format below
  deb/, rpm/, appimage/, flatpak/, snap/   per-format manifests/control files
  portable/         (build output only -- see scripts/package/build-portable.sh)
scripts/package/    render-icons.sh + one build-*.sh per format + build-all.sh
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
  embeddings/       text-to-vector embedding behind a swappable trait + a memoizing CachingEmbedder
  memory/           SQLite-backed session persistence + long-term memory + search (with a cached, invalidation-safe search path)
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
  cache/            capacity- and TTL-bounded Cache<K, V> + a thread-safe SharedCache wrapper -- backs embeddings::CachingEmbedder and memory's search-result cache
  integration-tests/  cross-crate integration, security, and stress tests (Phase 19) -- no library code, tests/ only
  screenshot/       full-screen capture via import(X11)/grim(Wayland) (real; success verified for real against Xvfb, a first for desktop-session-dependent Linux integration in this workspace)
  ocr/              tesseract CLI wrapper for text extraction (real, fully verified -- no display server needed, just the binary)
  vision/           real image analysis (dimensions/format/color via the image crate) + VisionBackend trait + an honest metadata-only stub
  vision-runtime/   Candle-based CLIP zero-shot image classifier implementing vision::VisionBackend (real API reasoned from source, unverified build -- same category as llm-runtime)
  coding-agent/     project analysis, cargo diagnostics, and code generation/templating (real, fully verified -- run_diagnostics is this workspace's first Destructive-level tool)
  agents/           Coordinator + AgentMessage dispatch (mirrors ToolRouter one layer up) plus five agents wrapping already-real crates: planner, memory, research (local search only), code, security
  ui/               Slint desktop shell: theme, sidebar, chat view, composer, animations, accessibility (Phase 17)
  app/              binary entry point (bin name: ai-assistant)
config/
  default.toml default configuration loaded at startup
data/
  database/    assistant.db (SQLite, created on first run, gitignored)
```

Later phases (19+: testing, packaging, documentation, and on) add more,
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

Phase 1, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 18, 19, 23
(`screenshot`/`ocr`/`vision` only -- `vision-runtime` needs Candle just
like `llm-runtime` does), 24, and 25 together (no Slint, no Candle) are
fully verified and work even on old toolchains:

```bash
cargo build -p common -p utilities -p config -p logger -p event-bus \
  -p conversation -p session-manager -p context-manager -p prompt-builder \
  -p assistant-core -p model-registry -p vector-db -p embeddings -p memory \
  -p reasoning -p planner -p tool-framework -p linux-services \
  -p permission-manager -p workflow-engine -p plugin-manager -p example-plugin \
  -p voice -p search -p network -p updater -p metrics -p telemetry -p diagnostics \
  -p security -p sandbox -p cache -p integration-tests \
  -p screenshot -p ocr -p vision -p coding-agent -p agents
cargo test -p common -p utilities -p config -p logger -p event-bus \
  -p conversation -p session-manager -p context-manager -p prompt-builder \
  -p assistant-core -p model-registry -p vector-db -p embeddings -p memory \
  -p reasoning -p planner -p tool-framework -p linux-services \
  -p permission-manager -p workflow-engine -p plugin-manager -p example-plugin \
  -p voice -p search -p network -p updater -p metrics -p telemetry -p diagnostics \
  -p security -p sandbox -p cache -p integration-tests \
  -p screenshot -p ocr -p vision -p coding-agent -p agents

# Benchmarks (dependency-free, no criterion -- see docs/ROADMAP.md Phase 19 notes):
cargo bench -p vector-db
cargo bench -p cache
```

`voice`'s tests shell out to `espeak-ng`, `pocketsphinx_continuous`, and
`sox` -- install them first (`apt install espeak-ng pocketsphinx
pocketsphinx-en-us sox libsox-fmt-all` on Debian/Ubuntu) or those
specific tests will fail with "not available" errors rather than being
skipped.

`ocr`'s tests need `tesseract-ocr`/`tesseract-ocr-eng` (the actual
capability under test) plus `imagemagick` (test-fixture generation
only -- `ocr` itself never shells out to `convert`): `apt install
tesseract-ocr tesseract-ocr-eng imagemagick`. `screenshot`'s tests need
`imagemagick` too (this time for real, `capture_screenshot` shells out to
`import`) and, unlike every other Linux-integration test in this
workspace, an actual running X server for the duration of `cargo test`,
not just an installed binary -- a real desktop session works, or stand up
a virtual one:

```bash
apt install imagemagick xvfb
Xvfb :99 -screen 0 1024x768x24 &
export DISPLAY=:99
cargo test -p screenshot
```

Without a reachable `DISPLAY`, `screenshot`'s capture-dependent tests
fail outright rather than skipping silently -- same "explicit failure
over a green checkmark that didn't prove anything" preference as
`voice`'s note above.

`linux-services`' `terminal` module only ever runs commands from a fixed
allowlist (see `crates/linux-services/src/terminal.rs`), never an
arbitrary shell string. `power` (lock/suspend/restart/shutdown) is
implemented but not registered as an automated tool anywhere in this
codebase -- see `docs/ROADMAP.md` Phase 8 for why. Destructive tools
(`delete_file`, `coding_agent::run_diagnostics` -- `cargo check`
executes `build.rs`/proc-macros with full privileges, a real risk, not
a hypothetical one) and `screenshot::capture_screenshot` (`Moderate` --
exposes whatever's on screen without the caller naming anything, unlike
a caller-named file path; see that tool's own doc comment) now go
through `permission-manager`'s real confirmation flow -- see
`crates/linux-services/tests/permission_integration.rs`,
`crates/screenshot/tests/permission_integration.rs`, and
`crates/coding-agent/tests/permission_integration.rs` for the full
pipeline exercised end to end, and
`crates/integration-tests/tests/security_adversarial.rs`'s `build.rs`
canary tests for concrete proof that denying `run_diagnostics` actually
stops the build from starting, not just returns an error code.

To use a real model once you're building `llm-runtime`/`app` on a modern
toolchain: download a Gemma 3 1B GGUF file and its `tokenizer.json`, name
them `gemma-3-1b.gguf` and `gemma-3-1b.tokenizer.json`, and put both in
`models/gemma/` (or wherever `config.model.model_path` points). `app`
picks them up automatically; without them it falls back to a stub backend
with a logged warning rather than failing.

## Packaging

Once a real `cargo build --release -p app` produces
`target/release/ai-assistant` on a machine with a modern toolchain:

```bash
./scripts/package/render-icons.sh    # only needed once, or after editing assets/icons/ai-assistant.svg
./scripts/package/build-all.sh       # DEB + RPM + AppImage + portable tarball, all at once
```

or build one format at a time with `scripts/package/build-deb.sh`,
`build-rpm.sh`, `build-appimage.sh` (needs `appimagetool` on `PATH` or
`$APPIMAGETOOL` set -- see `packaging/appimage/`), or
`build-portable.sh`. Every script accepts an explicit binary path as its
one argument if it's not at the default `target/release/ai-assistant`.
Output lands in `target/package/`.

Flatpak: `flatpak-builder --force-clean target/package/flatpak-build
packaging/flatpak/com.lionos.AiAssistant.yml` (needs the
`org.freedesktop.Platform`//`Sdk` 23.08 runtimes installed from Flathub
first -- not reachable from this project's own dev sandbox, see
`docs/ROADMAP.md`'s Phase 20 notes). Snap:
`snapcraft` from `packaging/snap/` (needs `snapcraft` itself, also not
available in this project's own dev sandbox -- same notes).

All six formats' manifests were written for a real toolchain, but only
DEB/RPM/AppImage/portable have actually been built, installed, run, and
(DEB) removed here, against a stand-in binary -- see `docs/ROADMAP.md`'s
Phase 20 notes for why and how.

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
