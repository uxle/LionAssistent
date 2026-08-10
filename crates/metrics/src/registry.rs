//! "Metrics" (Development_plan Phase 15): a small in-process registry
//! for counters, gauges, and timers -- what TDS SS9's performance
//! targets and DIG SS9's benchmarks (startup time, tool latency, memory
//! search, model loading) need somewhere to actually record to. Nothing
//! here leaves the process; exporting/sending metrics anywhere is a
//! different concern (see the `telemetry` crate, which is opt-in and
//! separate on purpose).

use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::RwLock;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default)]
struct TimerStats {
    count: u64,
    total: Duration,
    min: Duration,
    max: Duration,
}

impl TimerStats {
    fn record(&mut self, sample: Duration) {
        if self.count == 0 {
            self.min = sample;
            self.max = sample;
        } else {
            self.min = self.min.min(sample);
            self.max = self.max.max(sample);
        }
        self.count += 1;
        self.total += sample;
    }

    fn summary(&self) -> TimerSummary {
        let avg_ms = if self.count == 0 {
            0.0
        } else {
            self.total.as_secs_f64() * 1000.0 / self.count as f64
        };
        TimerSummary {
            count: self.count,
            avg_ms,
            min_ms: self.min.as_secs_f64() * 1000.0,
            max_ms: self.max.as_secs_f64() * 1000.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TimerSummary {
    pub count: u64,
    pub avg_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
}

/// A point-in-time read of every metric currently recorded. `BTreeMap`
/// (not `HashMap`) so snapshots -- and anything that serializes or
/// diffs them -- have a deterministic, sorted order.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MetricsSnapshot {
    pub counters: BTreeMap<String, u64>,
    pub gauges: BTreeMap<String, f64>,
    pub timers: BTreeMap<String, TimerSummary>,
}

impl MetricsSnapshot {
    /// The `n` timers with the highest average duration, slowest first
    /// -- e.g. for a Development_plan Phase 18 "Startup Optimization"
    /// report answering "where did startup time actually go", built on
    /// [`MetricsRegistry::time`]'s per-phase timers rather than a
    /// separate startup-specific data structure. Ties break by name for
    /// deterministic output, matching `MetricsSnapshot`'s own
    /// `BTreeMap`-ordered fields.
    pub fn slowest_timers(&self, n: usize) -> Vec<(&str, TimerSummary)> {
        let mut entries: Vec<(&str, TimerSummary)> = self
            .timers
            .iter()
            .map(|(name, summary)| (name.as_str(), *summary))
            .collect();
        entries.sort_by(|a, b| b.1.avg_ms.total_cmp(&a.1.avg_ms).then_with(|| a.0.cmp(b.0)));
        entries.truncate(n);
        entries
    }
}

/// Thread-safe metrics storage: any number of readers and writers from
/// any thread, matching how tool execution actually happens in this
/// workspace (`ToolRouter::execute` runs each tool via `spawn_blocking`
/// on the async runtime's blocking pool, potentially many at once).
#[derive(Default)]
pub struct MetricsRegistry {
    counters: RwLock<HashMap<String, u64>>,
    gauges: RwLock<HashMap<String, f64>>,
    timers: RwLock<HashMap<String, TimerStats>>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn increment(&self, name: &str) {
        self.increment_by(name, 1);
    }

    pub fn increment_by(&self, name: &str, delta: u64) {
        let mut counters = self
            .counters
            .write()
            .expect("metrics counters lock poisoned");
        *counters.entry(name.to_string()).or_insert(0) += delta;
    }

    pub fn set_gauge(&self, name: &str, value: f64) {
        let mut gauges = self.gauges.write().expect("metrics gauges lock poisoned");
        gauges.insert(name.to_string(), value);
    }

    pub fn record_duration(&self, name: &str, duration: Duration) {
        let mut timers = self.timers.write().expect("metrics timers lock poisoned");
        timers.entry(name.to_string()).or_default().record(duration);
    }

    /// Times `f`, records the elapsed wall time under `name` via
    /// [`MetricsRegistry::record_duration`], and returns `f`'s result --
    /// Development_plan Phase 18 "Startup Optimization"'s instrumentation
    /// primitive, letting a real caller (`app`'s startup sequence, one
    /// call per phase: config load, backend load, memory open) find out
    /// where its own time actually goes instead of guessing. Reuses this
    /// registry's existing timer storage rather than a separate
    /// startup-specific structure, so [`MetricsSnapshot::slowest_timers`]
    /// and `diagnostics::get_diagnostics_report` (Phase 15) see these
    /// phases for free, no new tool needed.
    pub fn time<T>(&self, name: &str, f: impl FnOnce() -> T) -> T {
        let start = std::time::Instant::now();
        let result = f();
        self.record_duration(name, start.elapsed());
        result
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let counters = self
            .counters
            .read()
            .expect("metrics counters lock poisoned");
        let gauges = self.gauges.read().expect("metrics gauges lock poisoned");
        let timers = self.timers.read().expect("metrics timers lock poisoned");
        MetricsSnapshot {
            counters: counters.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            gauges: gauges.iter().map(|(k, v)| (k.clone(), *v)).collect(),
            timers: timers
                .iter()
                .map(|(k, v)| (k.clone(), v.summary()))
                .collect(),
        }
    }

    /// Clears every counter, gauge, and timer. Mainly useful for tests
    /// and for a possible future "reset stats" settings action -- not
    /// called anywhere in this crate itself.
    pub fn reset(&self) {
        self.counters
            .write()
            .expect("metrics counters lock poisoned")
            .clear();
        self.gauges
            .write()
            .expect("metrics gauges lock poisoned")
            .clear();
        self.timers
            .write()
            .expect("metrics timers lock poisoned")
            .clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn counters_accumulate() {
        let registry = MetricsRegistry::new();
        registry.increment("requests");
        registry.increment("requests");
        registry.increment_by("requests", 3);
        assert_eq!(registry.snapshot().counters["requests"], 5);
    }

    #[test]
    fn gauges_hold_the_latest_value() {
        let registry = MetricsRegistry::new();
        registry.set_gauge("memory_used_kb", 100.0);
        registry.set_gauge("memory_used_kb", 250.0);
        assert_eq!(registry.snapshot().gauges["memory_used_kb"], 250.0);
    }

    #[test]
    fn timers_summarize_count_min_max_avg() {
        let registry = MetricsRegistry::new();
        registry.record_duration("tool_latency", Duration::from_millis(10));
        registry.record_duration("tool_latency", Duration::from_millis(30));
        registry.record_duration("tool_latency", Duration::from_millis(20));

        let summary = registry.snapshot().timers["tool_latency"];
        assert_eq!(summary.count, 3);
        assert_eq!(summary.min_ms, 10.0);
        assert_eq!(summary.max_ms, 30.0);
        assert_eq!(summary.avg_ms, 20.0);
    }

    #[test]
    fn unknown_metrics_are_absent_not_zero() {
        let registry = MetricsRegistry::new();
        let snapshot = registry.snapshot();
        assert!(!snapshot.counters.contains_key("never_recorded"));
    }

    #[test]
    fn reset_clears_everything() {
        let registry = MetricsRegistry::new();
        registry.increment("a");
        registry.set_gauge("b", 1.0);
        registry.record_duration("c", Duration::from_millis(1));
        registry.reset();

        let snapshot = registry.snapshot();
        assert!(snapshot.counters.is_empty());
        assert!(snapshot.gauges.is_empty());
        assert!(snapshot.timers.is_empty());
    }

    #[test]
    fn snapshot_ordering_is_deterministic() {
        let registry = MetricsRegistry::new();
        registry.increment("zebra");
        registry.increment("alpha");
        registry.increment("mike");

        let snapshot = registry.snapshot();
        let keys: Vec<&String> = snapshot.counters.keys().collect();
        assert_eq!(keys, vec!["alpha", "mike", "zebra"]);
    }

    #[test]
    fn time_records_a_duration_and_returns_the_closure_result() {
        let registry = MetricsRegistry::new();
        let result = registry.time("startup_phase", || {
            std::thread::sleep(Duration::from_millis(20));
            42
        });

        assert_eq!(result, 42);
        let summary = registry.snapshot().timers["startup_phase"];
        assert_eq!(summary.count, 1);
        // Generous lower bound (not an exact-ms check) to avoid flaking
        // on a loaded CI box, matching this crate's existing
        // real-timing tests in `monitor.rs`.
        assert!(
            summary.avg_ms >= 15.0,
            "expected roughly a 20ms sample, got {}",
            summary.avg_ms
        );
    }

    #[test]
    fn time_accumulates_multiple_calls_under_the_same_name() {
        let registry = MetricsRegistry::new();
        registry.time("phase", || ());
        registry.time("phase", || ());
        registry.time("phase", || ());

        assert_eq!(registry.snapshot().timers["phase"].count, 3);
    }

    #[test]
    fn slowest_timers_orders_descending_by_average_duration() {
        let registry = MetricsRegistry::new();
        registry.record_duration("fast", Duration::from_millis(5));
        registry.record_duration("slow", Duration::from_millis(50));
        registry.record_duration("medium", Duration::from_millis(20));

        let snapshot = registry.snapshot();
        let slowest = snapshot.slowest_timers(10);
        let names: Vec<&str> = slowest.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, vec!["slow", "medium", "fast"]);
    }

    #[test]
    fn slowest_timers_respects_the_requested_count() {
        let registry = MetricsRegistry::new();
        registry.record_duration("a", Duration::from_millis(1));
        registry.record_duration("b", Duration::from_millis(2));
        registry.record_duration("c", Duration::from_millis(3));

        assert_eq!(registry.snapshot().slowest_timers(2).len(), 2);
        assert_eq!(registry.snapshot().slowest_timers(0).len(), 0);
        assert_eq!(registry.snapshot().slowest_timers(100).len(), 3);
    }

    #[test]
    fn slowest_timers_breaks_ties_by_name_for_deterministic_output() {
        let registry = MetricsRegistry::new();
        registry.record_duration("zebra", Duration::from_millis(10));
        registry.record_duration("alpha", Duration::from_millis(10));

        let snapshot = registry.snapshot();
        let slowest = snapshot.slowest_timers(10);
        let names: Vec<&str> = slowest.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, vec!["alpha", "zebra"]);
    }

    #[test]
    fn concurrent_increments_from_many_threads_all_land() {
        let registry = Arc::new(MetricsRegistry::new());
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let registry = registry.clone();
                thread::spawn(move || {
                    for _ in 0..100 {
                        registry.increment("concurrent");
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(registry.snapshot().counters["concurrent"], 800);
    }
}
