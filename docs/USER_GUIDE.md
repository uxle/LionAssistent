# User Guide

**Honest scope, upfront:** this describes intended behavior based on the
actual code, not a walkthrough of a running app. `app`'s own binary has
never compiled in any environment this project has been developed in --
it depends on `ui` (needs Slint, which needs a newer Rust toolchain than
has been available) and `llm-runtime` (needs Candle, same issue). Every
individual piece described below (config loading, memory, tool
permissions, packaging) is real, compiled, and tested on its own; only
the assembled desktop application itself is unverified end-to-end. See
`docs/ROADMAP.md`'s phase-by-phase notes for exactly what's been checked
and how. This guide will stop needing this disclaimer once a modern
toolchain builds `app` for the first time -- check `README.md`'s status
line for whether that's happened yet.

## Installing

Once a real release build exists (`cargo build --release -p app` on a
machine with a modern Rust toolchain), five install options are built
and ready in `scripts/package/` (Development_plan Phase 20):

- **DEB** (Debian/Ubuntu): `./scripts/package/build-deb.sh`, then `sudo
  dpkg -i target/package/ai-assistant_*.deb`
- **RPM** (Fedora/openSUSE/etc.): `./scripts/package/build-rpm.sh`, then
  `sudo rpm -i target/package/ai-assistant-*.rpm`
- **AppImage** (any distro, no install): `./scripts/package/build-appimage.sh`,
  then run the resulting `.AppImage` directly
- **Portable tarball** (any distro, no install, no root):
  `./scripts/package/build-portable.sh`, extract, run `./run.sh`
- **Flatpak**/**Snap**: manifests exist (`packaging/flatpak/`,
  `packaging/snap/`) but haven't been build-tested end-to-end in this
  project's own dev environment -- see `docs/ROADMAP.md`'s Phase 20
  notes for exactly why (network access to Flathub/snapcraft, not a
  code problem)

All five/six formats install the same desktop entry and icon set, so
the app shows up in a normal application launcher regardless of which
one you use.

## First run and configuration

`app` reads `config/default.toml` on startup (`config::Config::load`).
Every field has a sensible default (`#[serde(default)]` throughout, so a
config file -- or a config file missing whole sections -- still loads
correctly), but the shipped defaults are:

```toml
[model]
active_model = "gemma-3-1b"
model_path = "models/gemma/"
context_length = 8192
temperature = 0.7
top_p = 0.9
max_tokens = 1024

[ui]
theme = "dark"              # or "light"
language = "en"
reduced_motion = false      # UI-UX_ SS13 accessibility
high_contrast = false
text_scale = 1.0            # 0.8-2.0, clamped if out of range

[performance]
embedding_cache_capacity = 256   # Phase 18
search_cache_capacity = 64
cache_ttl_seconds = 300          # 0 = never expire

enabled_tools = ["system_info", "open_application", "list_directory"]
enabled_plugins = []
logging_level = "info"
```

### Using a real local model

Without a model file present, `app` falls back to a stub backend (a
labeled placeholder reply, not a real answer) and logs a warning rather
than failing to start -- see `crates/app/src/main.rs`'s `load_backend`.
To use a real Gemma 3 1B model: download a GGUF file and its
`tokenizer.json`, name them `gemma-3-1b.gguf` and
`gemma-3-1b.tokenizer.json`, and place both under `models/gemma/` (or
wherever `model_path` points). Thanks to Phase 18's `LazyResponseGenerator`,
loading the model doesn't block startup -- it happens on the first
message you actually send, not before.

### Memory and privacy

Conversations and remembered facts are stored locally in SQLite at
`data/database/assistant.db` (created automatically on first run) --
nothing leaves the machine as part of the core assistant (PRD SS1/SS3:
no cloud dependency, local-first). Long-term memories are things the
assistant explicitly remembers (PRD 6.5's "Memories" -- preferences,
project context, etc.), separate from ordinary conversation history,
and are inspectable/editable in principle (Archd's Memory Viewer screen)
once the UI exists to do so.

### Permissions

Every system-affecting action goes through a five-level permission
system (Archd SS7), from `ReadOnly` (system info -- never asks) up to
`Administrator` (needs explicit approval every time). This isn't
optional or bypassable through the normal chat interface -- see
`crates/integration-tests/tests/security_adversarial.rs` for what *is*
and isn't actually enforced today, including one honestly-documented
gap: file-reading tools don't yet restrict *which* paths they can read
(that's deferred to an OS-level sandbox -- `sandbox::FilesystemSandbox`,
Landlock-based -- that nothing currently applies at startup). If you're
evaluating this project for anything security-sensitive, read that test
file's doc comments, not just this paragraph.

## Voice (optional)

Speech-to-text and text-to-speech (Phase 12) need `espeak-ng` and
`pocketsphinx` installed separately -- the DEB package's description
notes this; RPM's `%description` does too. Voice is off by default and
entirely optional; the core chat/tool experience doesn't need it.

## Uninstalling

- DEB: `sudo dpkg -r ai-assistant` (or `--purge` to also remove config)
- RPM: `sudo rpm -e ai-assistant`
- AppImage/portable: delete the file/directory -- nothing else was
  installed
- Local data (`data/database/assistant.db`, logs) isn't removed by any
  of the above; delete it manually if you want a clean slate

## Something not working?

This is still pre-release software with a known-incomplete piece (the
assembled desktop app itself). Before assuming something's broken:
check `docs/ROADMAP.md`'s phase table for whether the piece you're
looking at has actually been verified yet, and `CHANGELOG.md` for the
most recent phase's notes on what's known-unfinished. If you're building
from source, `docs/DEVELOPER_GUIDE.md` has the verified build commands
and explains which parts of the workspace need a newer toolchain than
others.
