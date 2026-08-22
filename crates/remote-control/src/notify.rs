//! Bridges `event_bus::EventBus` to connected SSE clients
//! (Development_plan Phase 28 "Notifications").
//!
//! This is deliberately *not* the same thing "push notifications" means
//! on a phone in the usual sense (an OS-level alert delivered even
//! while the companion app/tab isn't open) -- that virtually always
//! means routing through a cloud push service (FCM for Android, APNs
//! for iOS), which PRD SS3's own non-goal #3 rules out for this
//! assistant ("No cloud dependency for the core assistant"). What this
//! module gives instead is live push *while connected*: any device
//! with an open `/events` stream sees `event_bus` activity the moment
//! it happens, the same guarantee a desktop notification gives, just
//! over the network instead of D-Bus. That is the honest, local-first
//! reading of "Notifications" for this phase, not a lesser substitute
//! quietly standing in for the cloud version -- see `lib.rs` for the
//! same reasoning applied to "Android App".
//!
//! Structurally, this exists to cross an async/sync boundary cleanly:
//! `event_bus::EventBus::subscribe` gives a `tokio::sync::broadcast`
//! receiver (async), but each SSE connection is handled on a plain
//! blocking `tiny_http` worker thread (sync, deliberately -- see
//! `server.rs`'s module docs on why this crate stays synchronous at
//! the transport layer). One background async task drains the
//! `EventBus` and fans each event out to every currently-subscribed
//! `std::sync::mpsc::Sender`, so an SSE thread only ever does an
//! ordinary blocking `recv()`, no runtime access required.

use event_bus::{Event, EventBus};
use serde_json::{json, Value};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

fn event_to_json(event: &Event) -> Value {
    match event {
        Event::ApplicationStarted => json!({"type": "application_started"}),
        Event::ApplicationStopped => json!({"type": "application_stopped"}),
        Event::UserMessageReceived {
            session_id,
            content,
        } => json!({
            "type": "user_message_received",
            "session_id": session_id.to_string(),
            "content": content,
        }),
        Event::ResponseGenerated {
            session_id,
            content,
        } => json!({
            "type": "response_generated",
            "session_id": session_id.to_string(),
            "content": content,
        }),
        Event::ToolExecutionStarted { tool } => json!({
            "type": "tool_execution_started",
            "tool": tool,
        }),
        Event::ToolExecutionFinished { tool, success } => json!({
            "type": "tool_execution_finished",
            "tool": tool,
            "success": success,
        }),
        Event::PermissionRequested { tool } => json!({
            "type": "permission_requested",
            "tool": tool,
        }),
        Event::PermissionGranted { tool } => json!({
            "type": "permission_granted",
            "tool": tool,
        }),
        Event::PermissionDenied { tool } => json!({
            "type": "permission_denied",
            "tool": tool,
        }),
        Event::MemoryUpdated => json!({"type": "memory_updated"}),
    }
}

type Subscribers = Arc<Mutex<Vec<mpsc::Sender<String>>>>;

/// A live handle on the relay; cloning shares the same subscriber list
/// and background task.
#[derive(Clone)]
pub struct NotificationRelay {
    subscribers: Subscribers,
}

impl NotificationRelay {
    /// Spawns the background relay task on `handle` and returns a
    /// handle callers use to [`Self::subscribe`]. The returned
    /// `NotificationRelay` can be cloned freely and handed to every
    /// `server.rs` request handler via `SharedState`.
    pub fn spawn(bus: &EventBus, handle: &tokio::runtime::Handle) -> Self {
        let subscribers: Subscribers = Arc::new(Mutex::new(Vec::new()));
        let mut receiver = bus.subscribe();
        let task_subscribers = Arc::clone(&subscribers);

        handle.spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(event) => {
                        let payload = event_to_json(&event).to_string();
                        let mut guard = task_subscribers.lock().unwrap_or_else(|p| p.into_inner());
                        // Fan out, pruning any subscriber whose SSE
                        // thread has already gone away (write failed,
                        // receiver dropped) -- send() returning Err
                        // here means exactly that, never a transient
                        // condition worth retrying.
                        guard.retain(|tx| tx.send(payload.clone()).is_ok());
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "notification relay lagged behind event-bus; some events dropped for slow SSE subscribers");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        Self { subscribers }
    }

    /// Registers a new subscriber and returns the receiving end -- an
    /// ordinary blocking channel, safe to `.recv()` on any thread with
    /// no `tokio::runtime::Handle` required (see module docs).
    pub fn subscribe(&self) -> mpsc::Receiver<String> {
        let (tx, rx) = mpsc::channel();
        let mut guard = self.subscribers.lock().unwrap_or_else(|p| p.into_inner());
        guard.push(tx);
        rx
    }

    #[cfg(test)]
    fn subscriber_count(&self) -> usize {
        self.subscribers.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_published_event_reaches_a_subscriber_as_json() {
        let bus = EventBus::default();
        let relay = NotificationRelay::spawn(&bus, &tokio::runtime::Handle::current());
        let rx = relay.subscribe();

        bus.publish(Event::ToolExecutionFinished {
            tool: "system_info".to_string(),
            success: true,
        });

        // The relay task runs on the same runtime; yield so it gets a
        // turn before we block-recv on a std channel.
        tokio::task::yield_now().await;
        let payload =
            tokio::task::spawn_blocking(move || rx.recv_timeout(std::time::Duration::from_secs(2)))
                .await
                .unwrap()
                .unwrap();
        let parsed: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed["type"], json!("tool_execution_finished"));
        assert_eq!(parsed["tool"], json!("system_info"));
        assert_eq!(parsed["success"], json!(true));
    }

    #[tokio::test]
    async fn multiple_subscribers_each_get_their_own_copy() {
        let bus = EventBus::default();
        let relay = NotificationRelay::spawn(&bus, &tokio::runtime::Handle::current());
        let rx_a = relay.subscribe();
        let rx_b = relay.subscribe();

        bus.publish(Event::MemoryUpdated);
        tokio::task::yield_now().await;

        let (a, b) = tokio::task::spawn_blocking(move || {
            (
                rx_a.recv_timeout(std::time::Duration::from_secs(2)),
                rx_b.recv_timeout(std::time::Duration::from_secs(2)),
            )
        })
        .await
        .unwrap();
        assert!(a.is_ok());
        assert!(b.is_ok());
    }

    #[tokio::test]
    async fn a_dropped_receiver_is_pruned_on_the_next_publish() {
        let bus = EventBus::default();
        let relay = NotificationRelay::spawn(&bus, &tokio::runtime::Handle::current());
        let rx = relay.subscribe();
        assert_eq!(relay.subscriber_count(), 1);
        drop(rx);

        bus.publish(Event::ApplicationStarted); // send() fails, triggers prune
        tokio::task::yield_now().await;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert_eq!(relay.subscriber_count(), 0);
    }

    #[test]
    fn every_event_variant_serializes_to_a_typed_json_object() {
        use common::Id;
        let events = vec![
            Event::ApplicationStarted,
            Event::ApplicationStopped,
            Event::UserMessageReceived {
                session_id: Id::new(),
                content: "hi".into(),
            },
            Event::ResponseGenerated {
                session_id: Id::new(),
                content: "hello".into(),
            },
            Event::ToolExecutionStarted { tool: "t".into() },
            Event::ToolExecutionFinished {
                tool: "t".into(),
                success: false,
            },
            Event::PermissionRequested { tool: "t".into() },
            Event::PermissionGranted { tool: "t".into() },
            Event::PermissionDenied { tool: "t".into() },
            Event::MemoryUpdated,
        ];
        for event in events {
            let json = event_to_json(&event);
            assert!(
                json.get("type").and_then(Value::as_str).is_some(),
                "{event:?} missing type"
            );
        }
    }
}
