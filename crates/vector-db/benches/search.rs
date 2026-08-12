//! Benchmark for `VectorIndex::search` (Development_plan Phase 19
//! "Benchmarks": "Memory search"). Establishes a baseline for the Phase
//! 18 `select_nth_unstable_by` partial-selection rewrite at realistic
//! scales -- the equivalence test in `src/lib.rs` proves it's
//! *correct*; this measures what it actually costs.
//!
//! No `criterion`: this workspace's single shared `Cargo.lock` unifies
//! dependency versions across every member, including `llm-runtime`'s
//! `candle-core`, which pins a `half` requirement that needs rustc
//! 1.81+ -- confirmed by actually trying it (`cargo check --benches`
//! failed there, then again after pinning three different `half`/
//! `rayon-core` versions by hand), the same kind of empirical toolchain
//! wall already documented for Slint elsewhere in this project. Since
//! `vector-db` is a zero-dependency crate by design (see its Cargo.toml
//! description) and DIG SS11 says to avoid unrelated dependencies
//! anyway, a small hand-rolled timing harness is the more consistent
//! choice here, not just a fallback. `harness = false` in Cargo.toml
//! means this file only needs a plain `fn main()` -- no framework
//! required for `cargo bench -p vector-db` to run it.
//!
//! Run with `cargo bench -p vector-db --release` (release matters far
//! more here than for the test suite: this is timing real work).

use std::time::{Duration, Instant};
use vector_db::VectorIndex;

const DIMENSION: usize = 64;
const WARMUP_ITERS: usize = 10;
const TIMED_ITERS: usize = 200;

fn pseudo_random_vector(seed: &mut u64, dim: usize) -> Vec<f32> {
    (0..dim)
        .map(|_| {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((*seed >> 33) as u32) as f32 / u32::MAX as f32
        })
        .collect()
}

fn index_with(n: usize) -> (VectorIndex<u32>, Vec<f32>) {
    let mut seed = 0x2545F491_4F6CDD1Du64;
    let mut index = VectorIndex::new();
    for id in 0..n as u32 {
        index.insert(id, pseudo_random_vector(&mut seed, DIMENSION));
    }
    let query = pseudo_random_vector(&mut seed, DIMENSION);
    (index, query)
}

struct Stats {
    min: Duration,
    median: Duration,
    mean: Duration,
}

fn time_it(mut f: impl FnMut()) -> Stats {
    for _ in 0..WARMUP_ITERS {
        f();
    }
    let mut samples = Vec::with_capacity(TIMED_ITERS);
    for _ in 0..TIMED_ITERS {
        let start = Instant::now();
        f();
        samples.push(start.elapsed());
    }
    samples.sort();
    let total: Duration = samples.iter().sum();
    Stats {
        min: samples[0],
        median: samples[samples.len() / 2],
        mean: total / samples.len() as u32,
    }
}

fn fmt_duration(d: Duration) -> String {
    let micros = d.as_secs_f64() * 1_000_000.0;
    if micros < 1000.0 {
        format!("{micros:.2}\u{b5}s")
    } else {
        format!("{:.3}ms", micros / 1000.0)
    }
}

fn print_row(label: &str, stats: &Stats) {
    println!(
        "  {label:<28} min {:>10}   median {:>10}   mean {:>10}",
        fmt_duration(stats.min),
        fmt_duration(stats.median),
        fmt_duration(stats.mean),
    );
}

fn main() {
    println!("vector_db::VectorIndex::search -- {TIMED_ITERS} timed iterations after {WARMUP_ITERS} warmup, {DIMENSION}-dim vectors\n");

    println!("Fixed top_k = 10, store size growing:");
    for n in [100usize, 1_000, 5_000, 20_000] {
        let (index, query) = index_with(n);
        let stats = time_it(|| {
            std::hint::black_box(index.search(std::hint::black_box(&query), 10));
        });
        print_row(&format!("n = {n}"), &stats);
    }

    println!("\nFixed store size (n = 5,000), top_k growing:");
    let (index, query) = index_with(5_000);
    for top_k in [1usize, 10, 100, 1_000, 5_000] {
        let stats = time_it(|| {
            std::hint::black_box(index.search(std::hint::black_box(&query), top_k));
        });
        print_row(&format!("top_k = {top_k}"), &stats);
    }
}
