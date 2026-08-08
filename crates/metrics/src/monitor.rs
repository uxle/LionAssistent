//! "Performance Monitor" (Development_plan Phase 15): periodically
//! samples real system state via Phase 8's `linux_services::system::system_info`
//! (no reimplemented `/proc` parsing here), records it into a
//! [`MetricsRegistry`] as gauges, and keeps a bounded rolling history for
//! trend-style questions ("has memory pressure been climbing?") that a
//! single point-in-time `system_info()` call can't answer on its own.

use crate::registry::MetricsRegistry;
use common::{now, AppResult, Timestamp};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SystemSample {
    pub timestamp: Timestamp,
    pub memory_used_kb: u64,
    pub memory_total_kb: u64,
    pub load_average_1m: f64,
    pub uptime_seconds: f64,
}

pub struct PerformanceMonitor {
    registry: Arc<MetricsRegistry>,
    history: RwLock<VecDeque<SystemSample>>,
    max_history: usize,
}

impl PerformanceMonitor {
    pub fn new(registry: Arc<MetricsRegistry>, max_history: usize) -> Self {
        Self {
            registry,
            history: RwLock::new(VecDeque::with_capacity(max_history.min(1024))),
            max_history: max_history.max(1),
        }
    }

    /// Takes one real sample right now: reads CPU/memory/uptime/load via
    /// `linux_services::system::system_info`, records the numeric ones
    /// as gauges on the shared registry, and appends to this monitor's
    /// own rolling history (oldest sample dropped once `max_history` is
    /// exceeded).
    pub fn sample_now(&self) -> AppResult<SystemSample> {
        let info = linux_services::system::system_info()?;
        let memory_used_kb = info
            .memory_total_kb
            .saturating_sub(info.memory_available_kb);

        self.registry
            .set_gauge("memory_used_kb", memory_used_kb as f64);
        self.registry
            .set_gauge("memory_total_kb", info.memory_total_kb as f64);
        self.registry
            .set_gauge("load_average_1m", info.load_average_1m);
        self.registry
            .set_gauge("uptime_seconds", info.uptime_seconds);

        let sample = SystemSample {
            timestamp: now(),
            memory_used_kb,
            memory_total_kb: info.memory_total_kb,
            load_average_1m: info.load_average_1m,
            uptime_seconds: info.uptime_seconds,
        };

        let mut history = self
            .history
            .write()
            .expect("performance history lock poisoned");
        if history.len() >= self.max_history {
            history.pop_front();
        }
        history.push_back(sample);

        Ok(sample)
    }

    /// A snapshot (clone) of the current rolling history, oldest first.
    pub fn history(&self) -> Vec<SystemSample> {
        self.history
            .read()
            .expect("performance history lock poisoned")
            .iter()
            .copied()
            .collect()
    }
}

/// Samples `monitor` on a fixed interval forever, until the returned
/// task is aborted or the process exits. A ready-to-use building block,
/// not something this crate starts on its own anywhere -- PRD SS3 rules
/// out "unsafe hidden background behavior," so nothing here spawns
/// itself; whatever composes the real application (`app`, once Phase
/// 2's UI unblocks it) decides if and when to call this.
pub async fn run_periodic_sampling(
    monitor: Arc<PerformanceMonitor>,
    interval: std::time::Duration,
) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        if let Err(e) = monitor.sample_now() {
            tracing::warn!(error = %e, "performance sample failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_now_records_a_real_sample_and_updates_gauges() {
        let registry = Arc::new(MetricsRegistry::new());
        let monitor = PerformanceMonitor::new(registry.clone(), 10);

        let sample = monitor.sample_now().unwrap();
        assert!(sample.memory_total_kb > 0);

        let snapshot = registry.snapshot();
        assert_eq!(
            snapshot.gauges["memory_total_kb"],
            sample.memory_total_kb as f64
        );
    }

    #[test]
    fn history_accumulates_in_order() {
        let registry = Arc::new(MetricsRegistry::new());
        let monitor = PerformanceMonitor::new(registry, 10);

        monitor.sample_now().unwrap();
        monitor.sample_now().unwrap();
        monitor.sample_now().unwrap();

        let history = monitor.history();
        assert_eq!(history.len(), 3);
        assert!(history.windows(2).all(|w| w[0].timestamp <= w[1].timestamp));
    }

    #[test]
    fn history_is_bounded_and_drops_the_oldest() {
        let registry = Arc::new(MetricsRegistry::new());
        let monitor = PerformanceMonitor::new(registry, 3);

        for _ in 0..5 {
            monitor.sample_now().unwrap();
        }

        assert_eq!(monitor.history().len(), 3);
    }

    #[tokio::test]
    async fn run_periodic_sampling_actually_samples_on_the_configured_interval() {
        let registry = Arc::new(MetricsRegistry::new());
        let monitor = Arc::new(PerformanceMonitor::new(registry, 10));

        let handle = tokio::spawn(run_periodic_sampling(
            monitor.clone(),
            std::time::Duration::from_millis(20),
        ));
        tokio::time::sleep(std::time::Duration::from_millis(90)).await;
        handle.abort();

        // At ~20ms ticks over ~90ms, expect a handful of samples --
        // assert loosely (>= 2) to avoid a flaky exact-count check
        // against real wall-clock scheduling.
        assert!(monitor.history().len() >= 2, "{:?}", monitor.history());
    }
}
