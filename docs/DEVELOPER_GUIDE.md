# Developer Guide

Practical, codebase-accurate notes for working on this project --
complementing `DIG` (the aspirational Developer Implementation Guide in
the project's specification set) with what the actual workspace looks
like today, 20 phases in. If something here and `docs/ROADMAP.md`
disagree, trust `docs/ROADMAP.md`: it's updated every phase, this is
updated when it drifts noticeably out of date.

## Before you touch anything: know what's verified

Every crate under `crates/` falls into one of three categories, and it
matters which one before you start:

1. **Fully verified** -- builds, tests, and (where it makes sense) runs
   for real in a sandboxed environment with rustc 1.75. This is most of
   the workspace: `common` through `sandbox`/`cache`/`integration-tests`
   in the build command below. Change these with normal confidence --
   `cargo test -p <crate>` will tell you if you broke something.
2. **`ui`** -- needs Slint, which needs rustc 1.77+ for its windowing/
   accessibility dependencies, deeper than that per `docs/ROADMAP.md`'s
   Phase 17 investigation. Never compiled in a sandboxed dev environment
   as of this writing. Written carefully (`cargo fmt --check` at least
   confirms valid syntax), never run.
3. **`llm-runtime`, `app`** -- `llm-runtime` needs Candle, which needs
   rustc 1.88+ (a hard `zip` dependency version pin). `app` depends on
   both `ui` and `llm-runtime`, so it inherits both walls.

If you're not sure which category something falls in, check
`docs/ROADMAP.md`'s phase table at the top -- every phase's row says
plainly what got verified and what didn't.

**When you can't compile the real thing, don't skip verification --
substitute for the blocked part instead.** This project's two proven
techniques, both worth reusing:

- **A throwaway harness** (Phase 18, for `app`'s wiring): a small crate
  *outside* the workspace, depending on the same local path-crates via
  absolute paths, reproducing the new code's patterns with the blocked
  dependency (Candle, Slint) stubbed out or omitted. Compiled and run
  for real, then deleted -- never committed.
- **A stand-in artifact** (Phase 20, for packaging): when the thing
  being verified is a *process* around an artifact (a packaging script
  around a binary) rather than the artifact's own logic, build a trivial
  substitute (`rustc -O` on a five-line `fn main()`) and run the real
  process against it. Proves the mechanism, not the payload -- say so
  explicitly wherever you use this.

## Build and test

See the README's "Building" section for the exact, copy-pasteable
commands (`cargo build`/`cargo test` with the full `-p` list) -- kept
there rather than duplicated here so there's exactly one place it can go
stale. Two more useful ones for day-to-day work:

```bash
cargo doc --no-deps -p <crate>              # rustdoc HTML for one crate; keep this warning-free
cargo bench -p vector-db  # or -p cache      # hand-rolled benchmarks; see their own doc comments for why not criterion
```

**Gotcha, found the hard way during Phase 21's own tarball verification**:
running `cargo test -p plugin-manager` *by itself* (or any other subset
that doesn't happen to trigger building `example-plugin`'s cdylib
target) can fail `plugin_manager::dynamic`'s tests with "cannot open
shared object file: No such file or directory" on a fresh `target/`.
Those tests dynamically load `example-plugin`'s compiled `.so` by a
filesystem path, not through a normal Rust `use` -- `example-plugin`
being listed in `plugin-manager`'s `[dev-dependencies]` is enough for
Cargo to know the crate exists, but since nothing actually links against
it at the Rust source level, Cargo doesn't reliably rebuild its cdylib
artifact just from that dependency edge alone. The README's full
documented `cargo build ... -p example-plugin ...` command (run before
`cargo test`, as instructed, not skipped) always produces it correctly
-- confirmed by deleting `target/` entirely and running the exact
documented two-step sequence end to end. If you ever hit this shared
object error while iterating on a subset of crates, `cargo build -p
example-plugin` fixes it immediately; this isn't a flaky test, just an
implicit build-order dependency the test's own filesystem-path approach
doesn't declare to Cargo.

## Where things are, and the conventions that hold across all of them

```
crates/<name>/
  src/lib.rs       crate-level //! doc comment (always explains *why*, references PRD/Archd/TCLIS/DIG section numbers), re-exports
  src/*.rs         one file per submodule/responsibility (DIG SS6: prefer 300-800 lines, hard cap 1500)
  tests/*.rs       crate-local integration tests, when a crate has cross-module behavior worth testing outside src/ (e.g. permission_integration.rs)
  examples/*.rs    runnable usage demonstrations (Phase 21) -- `cargo run -p <crate> --example <name>`
  benches/*.rs     hand-rolled timing harnesses where they exist (vector-db, cache) -- see "Why no criterion" in their doc comments
```

Conventions that hold everywhere, not just in one crate:

- **Error mapping**: every crate that touches something fallible (SQLite,
  filesystem, a parser) has a small private `fn foo_err(e: SomeError) ->
  AppError` helper near the top of the file that needs it, used
  consistently via `.map_err(foo_err)`. Look at `memory::long_term`'s
  `db_err` or `config`'s equivalent before inventing a new pattern.
- **`register_all`**: every crate that contributes real `Tool`s exposes
  a single `pub fn register_all(registry: &mut ToolRegistry) ->
  AppResult<()>` that registers every tool it owns in one call --
  `linux_services::tools::register_all`, `search::tools::register_all`,
  `updater`'s equivalent. Follow this exactly for a new tool-contributing
  crate; don't invent a different registration shape.
- **`Arc<Mutex<_>>` for cross-thread sharing, not baked into the type**:
  stateful, non-trivially-Sync types (`memory::MemoryStore`,
  `permission_manager::PermissionManager`) stay unshared internally and
  let *callers* wrap them (`search::MemorySearchProvider` does this for
  `MemoryStore`). Don't add internal locking to a type unless it
  genuinely needs to be its own owner of thread-safety -- `cache::SharedCache`
  is the one place that pattern was actually the right call, and even
  there it's a thin wrapper around an unshared `Cache`.
- **Doc comments explain the *why*, with citations**: a `//!`/`///`
  comment referencing a spec section (`PRD 6.5`, `Archd SS7`, `TCLIS SS3`,
  `Apis SS6`) is normal and expected, not decoration -- it's how a
  reader confirms a design choice traces back to an actual requirement
  rather than being invented on the spot. When you're deviating from
  what a spec doc says (and there are legitimate reasons to, e.g. this
  sandbox's toolchain constraints), say so explicitly in the comment
  rather than silently diverging.
- **Every phase's build sweep**: `cargo fmt` (workspace-wide, including
  the unverifiable crates -- `cargo fmt --check` only needs valid syntax,
  not a working toolchain, so it's cheap insurance even there), `cargo
  clippy --all-targets -- -D warnings` per touched crate, then the full
  verified-crate `cargo test` sweep, before any commit.

## Adding a new tool

1. Implement `tool_framework::Tool` (six methods: `name`, `description`,
   `category`, `permission_level`, `timeout`, `input_schema`, plus
   `execute`). See `crates/tool-framework/examples/custom_tool.rs` for a
   complete, runnable minimal example, or any tool in
   `crates/linux-services/src/*.rs` for a real one.
2. Pick a `PermissionLevel` honestly (Archd SS7: `ReadOnly` = 0 through
   `Administrator` = 4) -- this is what actually gates confirmation
   prompts once wired through a real `PermissionManager`, not a
   formality.
3. Add it to your crate's `register_all`.
4. If the crate's tools interact with anything the permission/security
   model cares about (file paths, process control, network), add it to
   that crate's `tests/permission_integration.rs` if one exists, or
   consider whether `crates/integration-tests/tests/security_adversarial.rs`
   should grow a case for it -- see that file's own doc comment for the
   kind of adversarial testing this project does (bypass attempts,
   oversized input, not just the happy path).

## Adding a new crate

1. `mkdir -p crates/<name>/src`, a `Cargo.toml` following any existing
   crate's shape (`version.workspace = true` etc.), a `src/lib.rs` with
   a `//!` doc comment that says which Development_plan phase it's for
   and why it exists.
2. Add it to the root `Cargo.toml`'s `members` list (roughly
   chronological by phase, not alphabetical -- look at where the list
   currently ends for the pattern) and `[workspace.dependencies]` if
   other crates will depend on it.
3. Prefer zero or minimal dependencies unless there's a real reason not
   to (`vector-db`, `cache`, `embeddings` are all dependency-free by
   design; `search` is deliberately "dependency-free relevance
   scoring"). DIG SS11: avoid introducing unrelated dependencies.
4. Write the tests *as* you write the logic, not after -- every crate in
   this workspace has thorough `#[cfg(test)] mod tests` alongside its
   implementation, not a separate pass.

## Testing philosophy, briefly

- **Real timing, not mocked**: where a property is genuinely about time
  (a timeout, a TTL, an async task not blocking a worker thread), the
  test actually sleeps and measures, with a generous margin to avoid
  flakiness (see `cache::lru::tests` or
  `utilities::tests::does_not_block_the_async_runtime_while_it_runs`)
  rather than mocking a clock.
- **Prove equivalence, don't just assert it**: when an optimization is
  supposed to preserve old behavior (Phase 18's `vector_db::search`
  rewrite), write a test that checks it against a naive reference
  implementation across many inputs, not just a couple of hand-picked
  cases -- ties included, since ties are exactly where subtle bugs
  (unstable sort algorithms reordering things differently) hide.
- **Invalidation tests matter more than hit tests** for anything cached:
  `memory`'s search cache has tests proving a fresh `remember`/`forget`
  is immediately visible even with a warm cache, not just tests proving
  the cache returns the right thing when nothing has changed.
- **`crates/integration-tests`** is where cross-crate behavior,
  adversarial/security testing, and load/stress testing live -- see its
  own `src/lib.rs` doc comment before adding a new crate-local
  integration test file elsewhere; check whether it belongs there
  instead.
