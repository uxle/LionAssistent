//! `logger` -- structured logging initialization (TDS SS7, Apis SS12).
//!
//! Wraps `tracing` + `tracing-subscriber` so the rest of the workspace only
//! needs to call [`init`] once at startup and then use the `tracing` macros
//! (`tracing::info!`, `tracing::warn!`, ...) directly.

use tracing_subscriber::EnvFilter;

/// Initializes the global tracing subscriber. Should be called once, early
/// in `main()`. `level` accepts standard `tracing` filter syntax, e.g.
/// `"info"` or `"assistant_core=debug,warn"` (TDS SS6 `logging_level`).
pub fn init(level: &str) {
    let filter = EnvFilter::try_new(level).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
