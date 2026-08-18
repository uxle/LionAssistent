# API Documentation

The real API reference is generated rustdoc, not hand-maintained
markdown -- every public item in this workspace already carries a doc
comment (most explain *why*, with a spec-section citation, not just
*what*), and keeping a second, hand-written copy in sync with 39 crates
across 26 phases would drift the moment either one changed. Generate it
with:

```bash
cargo doc --no-deps -p <crate>        # one crate
cargo doc --no-deps <full -p list from README's "Building" section>   # everything verified
```

Output lands in `target/doc/<crate_name>/index.html`. As of Phase 21
this builds with **zero warnings** across every verified crate --
`rustdoc::broken_intra_doc_links` and `rustdoc::redundant_explicit_links`
are both on by default and both catch real mistakes (three were found
and fixed getting to zero: two broken links from doc comments into
`#[cfg(test)]`-only test functions that don't exist in a normal doc
build, one redundant explicit link target). Still zero as of Phase 23's
three new crates (`screenshot`, `ocr`, `vision`), Phase 24's
`coding-agent`, Phase 25's `agents`, and Phase 26's `rag` -- checked
again each time rather than assumed to still hold. Treat a new warning
here the same as a new `clippy` warning: worth fixing before it ships,
not ignoring.

## Finding the crate you want

Grouped by what they're *for*, not alphabetically -- the workspace
members list in the root `Cargo.toml` is already roughly chronological
by Development_plan phase if you want that view instead.

**Core conversation pipeline**: `assistant-core` (orchestration,
`LazyResponseGenerator`), `conversation`, `session-manager`,
`context-manager`, `prompt-builder`, `planner`, `reasoning`,
`llm-runtime`* (`ModelRegistry` half only -- see below), `model-registry`

**Tools, permissions, and Linux integration**: `tool-framework` (the
`Tool` trait, `ToolRouter`, `ToolRegistry` -- start here if you're
extending what the assistant can do), `permission-manager`,
`linux-services` (real tool implementations), `sandbox` (Landlock/rlimit)

**Memory and search**: `memory`, `embeddings` (`Embedder` trait,
`CachingEmbedder`), `vector-db`, `search`, `cache` (generic, used by both
`embeddings` and `memory`)

**Extensibility**: `plugin-manager`, `example-plugin` -- see
`docs/SDK_GUIDE.md` before diving into the source

**Automation**: `workflow-engine`

**Voice**: `voice` (speech-to-text, text-to-speech, wake word)

**Vision**: `screenshot` (full-screen capture), `ocr` (tesseract-backed
text extraction), `vision` (`VisionBackend` trait, real image analysis,
`analyze_image`), `vision-runtime`* (Candle CLIP backend implementing
`VisionBackend` -- see below)

**Development**: `coding-agent` (`analyze_project`, `run_cargo_diagnostics`,
`generate_from_template`/`generate_freeform` -- start here for anything
involving the assistant reading, checking, or writing code)

**Multi-Agent**: `agents` (`Coordinator`, `AgentMessage`, and five
agents -- `planner`, `memory`, `research` [local search only, no web],
`code`, `security` -- each wrapping an already-real crate rather than
reimplementing it; start here for anything involving multiple
capabilities cooperating on one task)

**RAG**: `rag` (`KnowledgeBase`, `chunk_text`, `extract_pdf_text` --
document indexing and semantic search over a local knowledge base,
separate from `memory`'s short personal facts; start here for anything
involving indexing or searching larger documents/PDFs)

**Infrastructure**: `common` (shared types, `AppError`/`AppResult` --
read this one first, everything depends on it), `config`, `logger`,
`event-bus`, `utilities`, `metrics`, `telemetry`, `diagnostics`,
`security`, `network`, `updater`

**Testing**: `integration-tests` -- no library code, just cross-crate
`tests/*.rs`; see its own doc comment for what belongs there versus in a
crate-local test file

**Unverified** (`ui`, `llm-runtime`'s Candle half, `vision-runtime`,
`app`): doc comments exist and are written carefully, but `cargo doc`
has never actually run against these in this project's development
environment -- see `docs/ROADMAP.md` for why and
`docs/DEVELOPER_GUIDE.md`'s "Before you touch anything" section for how
this project verifies changes to them anyway.

## Runnable examples

Prose and generated rustdoc both stop short of proving code actually
works the way it's described. Where it mattered most, this project adds
`examples/*.rs` instead (Phase 21) -- real, compiled, run-for-real
demonstrations, not documentation-only snippets that could silently
bit-rot:

```bash
cargo run -p cache --example basic_usage
cargo run -p tool-framework --example custom_tool
cargo run -p memory --example remember_and_search
cargo run -p assistant-core --example minimal_chat_loop
cargo run -p vision --example analyze_a_real_image
```

`tool-framework`'s is worth running first if you're new to this
codebase -- defining and executing a custom `Tool` through the real
validation/permission/timeout pipeline is close to the single most
central pattern the whole project is organized around.
