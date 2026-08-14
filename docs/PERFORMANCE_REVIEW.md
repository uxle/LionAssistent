# Performance Review

Development_plan Phase 22 "Performance Review". Every number below is
from an actual run in this development environment on this date
(2026-08-14), not estimated or extrapolated -- re-run the commands
yourself (`cargo bench -p vector-db`, `cargo bench -p cache`) to
reproduce them. Where a stated target from `TDS`, `DBDMS`, or `Archd`
can't be checked yet, that's said plainly rather than guessed at.

## What was measured

| Target (spec source) | Status | Evidence |
|---|---|---|
| Memory retrieval under 100ms for common queries (TDS SS9) | ✅ Met, by a wide margin | `vector_db::VectorIndex::search` at 5,000 entries: 403-444µs (top_k 1-100). Even at 20,000 entries and top_k=10: 1.65-1.78ms. Both ~50-200x under target. |
| Database startup under 100ms (DBDMS SS13) | ✅ Met, by a wide margin | `MemoryStore::in_memory` open (schema creation included) over 20 runs: 179µs min, 222µs median, 485µs max. ~200-500x under target. (Measured with the in-memory variant; a real file-backed `open` adds actual disk I/O, not measured here.) |
| Indexed searches under 50ms for common queries (DBDMS SS13) | ✅ Met | Same search data as above -- every measured configuration is under 2ms, let alone 50ms. |
| Tool execution is asynchronous (TDS SS9) | ✅ Met, verified | `tool_framework::ToolRouter::execute` has run real timeout-enforcement tests since Phase 7 (a tool that actually sleeps past its timeout is actually cancelled, not simulated). Phase 18 extended the same non-blocking discipline to `app`'s background task via `utilities::run_blocking`, itself tested for genuinely not blocking the async runtime (`utilities::tests::does_not_block_the_async_runtime_while_it_runs`). |
| No blocking operations on the UI thread (TDS SS9) | ⚠️ Partially assessable | The tool-execution and message-processing paths are verified non-blocking (above). Whether `ui`'s own Slint event loop ever blocks can't be assessed -- `ui` has never compiled in this environment. |
| Support at least 1 million messages without major degradation (DBDMS SS13) | ⚠️ Not verified at that scale | Phase 19's stress test covers 5,000 memory entries; this benchmark suite goes to 20,000 vectors. Neither approaches 1M. `vector_db::VectorIndex::search`'s O(n) scoring pass means search time scales roughly linearly with entry count (confirmed by the n=100→20,000 progression above, a ~200x increase in `n` producing a ~190x increase in search time) -- extrapolating linearly, 1M entries would be in the ~90ms range for a single search, which would *not* comfortably clear the 50ms "common queries" target above. This is an honest projection from measured scaling behavior, not a measurement -- if this project's real-world memory counts ever approach that range, `vector_db`'s own doc comment already names the fix (an approximate-nearest-neighbor index) as future work, not a surprise. |
| Startup under 3 seconds excluding model load (Archd SS19) | ❌ Not verifiable | `app` has never compiled in this environment (Slint/Candle toolchain walls -- see `docs/ROADMAP.md`). What *is* known: Phase 18 made real model loading lazy specifically so it doesn't sit on the startup critical path, and every measured startup-adjacent operation (config load, database open, filesystem sandbox setup as of this phase) is sub-millisecond. Nothing currently known would threaten a 3-second budget, but "nothing known would" isn't the same claim as "measured at under 3 seconds." |
| Streaming responses / responsive UI during inference (Archd SS19) | ❌ Not verifiable | Needs a working `llm-runtime` (Candle) and `ui` (Slint), neither compiles here. |
| Low idle CPU usage (Archd SS19) | ❌ Not measured | Needs a running process to profile; `app` doesn't run here. |

## Cache and search benchmarks, in full

From `cargo bench -p vector-db` and `cargo bench -p cache` (dependency-free
hand-rolled harnesses -- see each crate's `benches/` doc comment for why
not `criterion`; Phase 19 hit the same Candle/rustc-version wall as
Slint trying to add it):

```
vector_db::VectorIndex::search -- 200 timed iterations, 64-dim vectors

Fixed top_k = 10, store size growing:
  n = 100        median     8.83µs
  n = 1,000      median    86.73µs
  n = 5,000      median   444.41µs
  n = 20,000     median  1768.00µs

Fixed store size (n = 5,000), top_k growing:
  top_k = 1      median   437.00µs
  top_k = 10     median   442.50µs
  top_k = 100    median   451.29µs
  top_k = 1,000  median   469.92µs
  top_k = 5,000  median   841.52µs
```

The near-flat line across `top_k = 1` through `1,000` (437-470µs) versus
the much steeper `n` scaling is exactly the Phase 18 optimization's
signature: cost is dominated by the unavoidable O(n) cosine-similarity
scoring pass, not by how many results are actually kept. The old
(pre-Phase-18) implementation would show `top_k` scaling too, since it
sorted the entire result set regardless of how few results were kept.

```
cache::Cache / SharedCache -- 100 timed iterations

put() that triggers eviction, by cache capacity:
  capacity = 16      median    0.160µs
  capacity = 128     median    0.396µs
  capacity = 1,024   median    1.860µs
  capacity = 8,192   median   13.497µs

get() at capacity 1,024:
  hit    median   0.073µs
  miss   median   0.042µs

Cache vs. SharedCache (Mutex overhead), single-threaded hit:
  Cache (bare)                  median   0.058µs
  SharedCache (Arc<Mutex<_>>)   median   0.068µs
```

`cache`'s own doc comment justifies its brute-force (O(n) scan)
eviction strategy by arguing it's "fast enough" at this project's actual
target scale (tens to a few hundred entries per cache). The data backs
that up with room to spare: even at 8,192 entries -- far beyond any
configured default (`config::PerformanceConfig`'s defaults are 256 and
64) -- eviction costs under 14 microseconds.

## Build and test suite timings

Real numbers from this phase's own clean-room verifications (deleting
`target/` entirely and rebuilding from scratch), not cached/incremental
runs:

- Full `cargo build` (all 33 verified crates, from scratch): ~2m50s-3m14s across several runs this development cycle.
- Full `cargo test` (same 33 crates, from scratch, including the build): ~1m13s-3m07s depending on how much was already cached from the preceding build step.
- 447 tests total, `cargo test`'s own reported per-crate execution times are all under half a second; the wall-clock time above is almost entirely compilation, not test execution.

## What this review doesn't and can't claim

This document is honest about its own limits the same way every other
verification in this project is: it reports what's been measured, not
what's been assumed. The single biggest gap is that every number here
comes from individual crates or a synthetic benchmark harness, not from
the assembled `ai-assistant` binary under realistic desktop use --
because that binary doesn't exist yet in this environment. Once a
modern toolchain builds `app` for the first time, the highest-value
follow-up to this document is running it and checking the ❌ and ⚠️ rows
above for real.
