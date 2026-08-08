//! [`TelemetryClient`]: opt-in event buffering plus an explicit
//! [`TelemetryClient::flush`] that's the *only* place in this crate
//! that ever touches the network.
//!
//! Two independent gates, both must be true before a single byte leaves
//! the device:
//!
//! 1. `enabled` -- passed explicitly to [`TelemetryClient::new`]; this
//!    crate has no internal default, no config file it reads on its
//!    own, nothing that could turn this on without the caller deciding
//!    to. [`TelemetryClient::record`] is always safe to call regardless
//!    -- it silently no-ops when disabled, so call sites elsewhere in
//!    the codebase don't need an `if telemetry_enabled` check scattered
//!    around every call.
//! 2. `endpoint` -- `None` means "collect locally in memory only, never
//!    send," even if `enabled` is `true`. Both must hold for
//!    [`TelemetryClient::flush`] to make an HTTP call at all.

use crate::event::TelemetryEvent;
use common::AppResult;
use network::HttpClient;
use std::sync::Mutex;

/// How many events to keep buffered before the oldest are dropped to
/// make room for new ones -- a bound so a long-running process that
/// never flushes can't grow this without limit.
const DEFAULT_MAX_BUFFERED: usize = 500;

pub struct TelemetryClient {
    enabled: bool,
    endpoint: Option<String>,
    max_buffered: usize,
    events: Mutex<Vec<TelemetryEvent>>,
}

impl TelemetryClient {
    /// `enabled` and `endpoint` are both required, explicit arguments on
    /// purpose -- see this module's doc comment. There is no
    /// `TelemetryClient::default()` that turns anything on.
    pub fn new(enabled: bool, endpoint: Option<String>) -> Self {
        Self {
            enabled,
            endpoint,
            max_buffered: DEFAULT_MAX_BUFFERED,
            events: Mutex::new(Vec::new()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Buffers `event` if enabled; otherwise does nothing at all -- not
    /// even an allocation. Safe to call unconditionally from anywhere in
    /// the codebase.
    pub fn record(&self, event: TelemetryEvent) {
        if !self.enabled {
            return;
        }
        let mut events = self.events.lock().expect("telemetry buffer lock poisoned");
        if events.len() >= self.max_buffered {
            events.remove(0);
        }
        events.push(event);
    }

    pub fn buffered_count(&self) -> usize {
        self.events
            .lock()
            .expect("telemetry buffer lock poisoned")
            .len()
    }

    /// Sends everything currently buffered as one batch and clears the
    /// buffer, returning how many events were sent. Makes **no** network
    /// call, and returns `Ok(0)`, unless both `enabled` is `true` and an
    /// `endpoint` was configured -- the two gates from this module's doc
    /// comment, checked here, together, every time.
    pub fn flush(&self, client: &HttpClient) -> AppResult<usize> {
        if !self.enabled {
            return Ok(0);
        }
        let Some(endpoint) = &self.endpoint else {
            return Ok(0);
        };

        let mut events = self.events.lock().expect("telemetry buffer lock poisoned");
        if events.is_empty() {
            return Ok(0);
        }

        let batch = std::mem::take(&mut *events);
        let count = batch.len();
        drop(events);

        let _: serde_json::Value =
            client.post_json(endpoint, &serde_json::json!({ "events": batch }))?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::TelemetryEventKind;
    use network::testing::MockServer;

    #[test]
    fn disabled_client_never_buffers_anything() {
        let client = TelemetryClient::new(false, Some("http://127.0.0.1:1/ignored".to_string()));
        client.record(TelemetryEvent::new(TelemetryEventKind::SessionStarted));
        assert_eq!(client.buffered_count(), 0);
    }

    #[test]
    fn enabled_client_buffers_events() {
        let client = TelemetryClient::new(true, None);
        client.record(TelemetryEvent::new(TelemetryEventKind::SessionStarted));
        client.record(TelemetryEvent::new(TelemetryEventKind::SessionEnded));
        assert_eq!(client.buffered_count(), 2);
    }

    #[test]
    fn flush_is_a_true_no_op_when_disabled_even_with_an_endpoint_configured() {
        // The endpoint deliberately points at nothing listening: if
        // flush() tried to connect despite `enabled: false`, this test
        // would hang or error instead of cleanly returning Ok(0).
        let client =
            TelemetryClient::new(false, Some("http://127.0.0.1:1/unreachable".to_string()));
        client.record(TelemetryEvent::new(TelemetryEventKind::ToolExecuted));

        let http = HttpClient::new().unwrap();
        let sent = client.flush(&http).unwrap();
        assert_eq!(sent, 0);
    }

    #[test]
    fn flush_is_a_true_no_op_when_enabled_but_no_endpoint_is_configured() {
        let client = TelemetryClient::new(true, None);
        client.record(TelemetryEvent::new(TelemetryEventKind::ToolExecuted));

        let http = HttpClient::new().unwrap();
        let sent = client.flush(&http).unwrap();
        assert_eq!(sent, 0);
        // Not sent, but not dropped either -- still buffered for a
        // later flush against a real endpoint if one gets configured.
        assert_eq!(client.buffered_count(), 1);
    }

    #[test]
    fn flush_sends_the_batch_and_clears_the_buffer_when_fully_configured() {
        let server = MockServer::respond_once(200, "application/json", b"{}".to_vec());
        let client = TelemetryClient::new(true, Some(server.base_url.clone()));
        client.record(TelemetryEvent::new(TelemetryEventKind::SessionStarted));
        client.record(TelemetryEvent::new(TelemetryEventKind::ToolExecuted).with_label("Search"));

        let http = HttpClient::new().unwrap();
        let sent = client.flush(&http).unwrap();
        server.join();

        assert_eq!(sent, 2);
        assert_eq!(client.buffered_count(), 0);
    }

    #[test]
    fn flushing_an_empty_buffer_makes_no_network_call() {
        // No endpoint listener at all -- if flush() tried to send an
        // empty batch anyway, this would error instead of returning
        // Ok(0).
        let client = TelemetryClient::new(true, Some("http://127.0.0.1:1/unreachable".to_string()));
        let http = HttpClient::new().unwrap();
        assert_eq!(client.flush(&http).unwrap(), 0);
    }

    #[test]
    fn the_buffer_is_bounded() {
        let client = TelemetryClient::new(true, None);
        for _ in 0..(DEFAULT_MAX_BUFFERED + 10) {
            client.record(TelemetryEvent::new(TelemetryEventKind::ToolExecuted));
        }
        assert_eq!(client.buffered_count(), DEFAULT_MAX_BUFFERED);
    }
}
