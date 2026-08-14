# Release Notes: Foundation Complete (v0.1.0)

Development_plan Phase 22, "Stable Release (v1.0)" -- and the first
thing this document does is explain why the version number below is
**not** 1.0.0, because that decision is itself the most important thing
in this release.

## The honest headline

**This is not a v1.0.0 product release, and the version number
deliberately hasn't moved to signal that it's close to being one.**

Every crate in this workspace shares one version number
(`version.workspace = true`) -- including `app`, the actual binary a
person would run. `app` has never successfully compiled in this
project's development environment, on any commit, at any phase. It
depends on `ui` (needs Slint, which needs a newer Rust toolchain) and
`llm-runtime` (needs Candle, same issue, different version floor).
Bumping the shared version number to `1.0.0`, or even to something like
`0.9.0` meant to signal "almost there," would put that number on `app`
too -- a claim the code cannot back up, on the one crate where it would
matter most. The version stays at **0.1.0**. That's a deliberate choice,
not an oversight: an honest version number that undersells a genuinely
substantial foundation is a much smaller problem than a version number
that overstates a product's readiness.

## What "Definition of Done" actually looks like right now

PRD SS18 defines six criteria for the product being complete. Assessed
plainly, with evidence, not aspirationally:

| Criterion | Status |
|---|---|
| The codebase is modular | ✅ **Met.** 33 crates, each with a single clear responsibility, `#[cfg(test)]` coverage throughout, DIG SS6's file-size guidance followed (nothing over 1,500 lines, most well under). |
| Permissions and logs exist | ✅ **Met.** Five-level permission system (`permission-manager`, Phase 9) with real audit logging, structured logging throughout (`logger`, Phase 1), diagnostics/metrics/telemetry (Phases 15). All tested, including adversarial cases (Phase 19). |
| The assistant can safely run approved Linux actions | ⚠️ **Met at the library level, not end-to-end.** `tool-framework` + `linux-services` + `permission-manager` together form a real, tested pipeline -- real tools, real permission checks, real timeout enforcement, real audit logging (447 tests across the relevant crates, plus Phase 19's dedicated integration and adversarial security tests). What's missing is the automatic connection from a live conversation to a live tool call, which needs a working `llm-runtime` to extract structured tool calls from real model output -- blocked on the same Candle wall as everything else touching that crate. |
| The assistant can chat locally | ⚠️ **Met at the library level, not end-to-end.** `assistant-core`'s conversation pipeline is real and tested (session management, context building, prompt construction, streaming-shaped API). It has never been exercised through the actual `app` binary talking to a real model, because that binary doesn't compile yet. |
| The app has a working UI | ❌ **Not met.** `ui` has never compiled in this environment. Written carefully (valid Slint syntax as far as manual review and `cargo fmt --check` can confirm) since Phase 2, never run. |
| The MVP can be tested end to end | ❌ **Not met**, for the same reason. Every *piece* is end-to-end tested against its neighbors (`crates/integration-tests`, Phase 19); the assembled product has never been started, let alone tested. |

Two fully met, two substantially met with a clearly named gap, two not
met. That's the real state of this project as of Phase 22, and it's a
meaningfully different picture from "feature complete" -- worth saying
in exactly those terms rather than rounding up.

## What genuinely is done, and solid

- **32 non-UI, non-Candle crates, fully verified**: build, test, and (where relevant) actually run in this environment. 447 tests, zero known failures, re-confirmed via clean-room tarball extraction every phase since 18.
- **A real, tested tool-calling pipeline**: define a `Tool`, register it, and it goes through identical validation, permission-checking, and timeout enforcement whether it's built-in or plugin-contributed (Phase 19 proved the plugin case specifically, panics included).
- **Local memory with real semantic search**, caching (Phase 18), and a search-result cache that's provably invalidated on every write, not just tested for cache hits (Phase 18's most important test isn't "does the cache work," it's "does the cache ever lie").
- **Six packaging formats** (Phase 20): DEB, RPM, AppImage, and a portable tarball actually built and (three of four) installed/run/removed for real against a stand-in binary; Flatpak and Snap manifests are correct and (Flatpak) validated by real tooling, blocked only by this development environment's own network policy, not a code problem.
- **A filesystem sandbox now wired into `app`'s own startup** (this phase): `sandbox::FilesystemSandbox::restrict_current_process()` (Phase 16) is real, tested Landlock-restriction code that nothing actually *called* at process startup until now -- closing the exact gap Phase 19's security tests documented rather than just noting it. Verified compiles and runs correctly via a throwaway harness (this project's established technique for checking code that touches an otherwise-uncompilable crate); `app` itself remains unverified for the usual reason.
- **Documentation that matches the code**: `cargo doc` builds warning-free across all 33 verified crates; `docs/DEVELOPER_GUIDE.md`, `docs/SDK_GUIDE.md`, and `docs/USER_GUIDE.md` (Phase 21) are current, not aspirational.
- **A performance profile with real numbers, not projections** -- see `docs/PERFORMANCE_REVIEW.md` (this phase). Every measurable target from TDS/DBDMS is met with room to spare; every target that needs the assembled app is honestly marked unverifiable rather than assumed.

## Known limitations (the complete list, not a curated one)

- `ui`, `llm-runtime`'s Candle-dependent half, and `app` itself have never compiled in this environment. This is the single largest gap and the one everything else in this section is downstream of.
- File-reading tools have no path-scoping of their own by design (deferred to the Landlock sandbox above); Landlock enforcement itself is confirmed **not** enforced in this specific development sandbox (Phase 16's own finding) even though it's expected to work on a real target desktop -- the enforcement status is checked and logged, not assumed, wherever it matters.
- Calling `Tool::execute()` directly bypasses the router's permission/validation/audit pipeline entirely -- an architectural property of exposing `Arc<dyn Tool>`, not a bug, documented and tested in Phase 19.
- `vector_db`'s brute-force O(n) search has only been measured to 20,000 entries; linear extrapolation suggests it would miss DBDMS's 50ms search-time target somewhere before 1 million entries (see `docs/PERFORMANCE_REVIEW.md`). Not a problem at today's realistic scale; worth revisiting if it ever becomes one.
- Flatpak and Snap packaging are correct but unverified by an actual build in this environment (network/tooling access, not a code issue -- see `docs/ROADMAP.md`'s Phase 20 notes).
- No real Gemma 3 1B inference has ever run against this codebase. Everything downstream of `llm-runtime::CandleGemmaBackend` is written against Candle's documented API, never executed.

## Long-term support branch

The commit this document ships with is tagged as a stable reference
point: branch `lts/0.1.0-foundation`. It exists so future work that
*does* get a modern toolchain (unblocking Slint/Candle) has a clean,
fully-tested, fully-documented commit to build from or diff against,
without needing to replay this project's own phase-by-phase history to
find one.

## Path to an honest v1.0.0

In order, since each genuinely depends on the one before it:

1. A Rust toolchain with rustc 1.88+ (Candle's floor) and whatever Slint
   1.x currently needs (1.77+ at minimum, per Phase 17's investigation --
   worth re-checking, since that investigation is itself now several
   phases old).
2. `cargo build -p app` succeeding, for the first time.
3. Working through this project's actual "not done, deliberately" list
   with a real binary available -- starting with wiring `assistant-core`'s
   output into `tool-framework`'s real tool-call execution (the one
   piece every phase since 7 has correctly deferred, for a real reason).
4. Re-running everything in `docs/PERFORMANCE_REVIEW.md`'s ❌ and ⚠️ rows
   against the real, running app.
5. *Then* a version number that says 1.0.0, meaning it.
