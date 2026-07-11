//! `app` -- binary entry point. Phase 1 gave us `common`, `config`,
//! `logger`, and `event-bus`; Phase 2 added the Slint UI shell (`ui`);
//! Phase 3 adds `assistant-core`, running as a background task that
//! answers whatever the UI publishes.
//!
//! `main` is deliberately synchronous, not `#[tokio::main]`: native GUI
//! backends need the real OS main thread, and Slint's `run()` blocks it
//! until the window closes. A multi-threaded Tokio runtime is built up
//! front and entered so both assistant-core's background task and the
//! UI's reply listener (spawned inside `ui::run`) can run on it.

use assistant_core::{AssistantCore, EchoResponseGenerator};
use common::{AppError, AppResult};
use config::Config;
use event_bus::{Event, EventBus};

fn main() -> AppResult<()> {
    let config = Config::load("config/default.toml")?;
    logger::init(&config.logging_level);

    tracing::info!(
        model = %config.model.active_model,
        theme = %config.ui.theme,
        "starting Local Linux AI Assistant (Phase 3: assistant core)"
    );

    let runtime = tokio::runtime::Runtime::new()?;
    let _guard = runtime.enter();

    let bus = EventBus::default();

    let mut assistant = AssistantCore::new(Box::new(EchoResponseGenerator));
    let session_id = assistant.create_session(&config.model.active_model);

    // Background task: assistant-core answers whatever arrives on the
    // bus. `assistant` moves in whole -- nothing else needs it, so no
    // Arc<Mutex<_>> required.
    {
        let mut events = bus.subscribe();
        let reply_bus = bus.clone();
        tokio::spawn(async move {
            while let Ok(event) = events.recv().await {
                let Event::UserMessageReceived {
                    session_id,
                    content,
                } = event
                else {
                    continue;
                };
                match assistant.process_message(session_id, content) {
                    Ok(reply) => {
                        reply_bus.publish(Event::ResponseGenerated {
                            session_id,
                            content: reply,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "assistant-core failed to process message")
                    }
                }
            }
        });
    }

    ui::run(bus, session_id).map_err(|e| AppError::Module {
        module: "ui",
        message: e.to_string(),
    })?;

    tracing::info!("UI closed, shutting down");
    Ok(())
}
