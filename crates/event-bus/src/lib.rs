//! `event-bus` -- the application-wide event bus (Archd SS9, Apis SS13).
//!
//! All modules communicate through events rather than calling each other
//! directly, keeping the assistant core, UI, tools, and plugins loosely
//! coupled (Archd SS3 "event-driven").

use common::Id;
use tokio::sync::broadcast;

/// Core application events. This is a starting subset of Apis SS13; extend
/// it as the Assistant Core, Tool Framework, and Permission Manager land in
/// later phases.
#[derive(Debug, Clone)]
pub enum Event {
    ApplicationStarted,
    ApplicationStopped,
    UserMessageReceived { session_id: Id, content: String },
    ResponseGenerated { session_id: Id, content: String },
    ToolExecutionStarted { tool: String },
    ToolExecutionFinished { tool: String, success: bool },
    PermissionRequested { tool: String },
    PermissionGranted { tool: String },
    PermissionDenied { tool: String },
    MemoryUpdated,
}

/// Thin wrapper around a broadcast channel so callers don't need to depend
/// on `tokio::sync` directly. Cloning an `EventBus` gives another handle to
/// the same underlying channel.
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publishes an event to every current subscriber. Publishing with no
    /// active subscribers is not an error -- it simply means nobody is
    /// listening right now.
    pub fn publish(&self, event: Event) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_and_receive() {
        let bus = EventBus::default();
        let mut rx = bus.subscribe();
        bus.publish(Event::ApplicationStarted);
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, Event::ApplicationStarted));
    }

    #[tokio::test]
    async fn multiple_subscribers_all_receive() {
        let bus = EventBus::default();
        let mut rx_a = bus.subscribe();
        let mut rx_b = bus.subscribe();
        bus.publish(Event::MemoryUpdated);
        assert!(matches!(rx_a.recv().await.unwrap(), Event::MemoryUpdated));
        assert!(matches!(rx_b.recv().await.unwrap(), Event::MemoryUpdated));
    }
}
