//! `app` -- binary entry point tying together the Phase 1 foundation
//! crates: `common`, `config`, `logger`, and `event-bus`
//! (Development_plan Phase 1).
//!
//! This binary does not yet include the Slint UI (Phase 2), the assistant
//! core (Phase 3), or the LLM runtime (Phase 4). It exists to prove that
//! the foundation compiles, wires together, and runs end-to-end before any
//! of that is built on top of it.

use common::AppResult;
use config::Config;
use event_bus::{Event, EventBus};

#[tokio::main]
async fn main() -> AppResult<()> {
    let config = Config::load("config/default.toml")?;
    logger::init(&config.logging_level);

    tracing::info!(
        model = %config.model.active_model,
        theme = %config.ui.theme,
        "starting Local Linux AI Assistant (Phase 1 foundation)"
    );

    let bus = EventBus::default();
    let mut events = bus.subscribe();

    bus.publish(Event::ApplicationStarted);
    if let Ok(event) = events.try_recv() {
        tracing::info!(?event, "event received");
    }

    tracing::info!(
        "Phase 1 foundation is alive: config, logger, and event bus are wired together."
    );

    bus.publish(Event::ApplicationStopped);

    Ok(())
}
