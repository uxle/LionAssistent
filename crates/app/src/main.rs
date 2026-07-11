//! `app` -- binary entry point. Phase 1 gave us `common`, `config`,
//! `logger`, and `event-bus`; Phase 2 adds the Slint UI shell (`ui`),
//! which now owns the process's main loop.
//!
//! `main` is deliberately synchronous, not `#[tokio::main]`: native GUI
//! backends need the real OS main thread, and Slint's `run()` blocks it
//! until the window closes. A multi-threaded Tokio runtime is still built
//! up front and entered so later phases (LLM inference, async tool
//! execution) can spawn background work from inside UI callbacks via
//! `tokio::spawn`, without the UI thread itself ever needing to be async.

use common::{AppError, AppResult};
use config::Config;
use event_bus::EventBus;

fn main() -> AppResult<()> {
    let config = Config::load("config/default.toml")?;
    logger::init(&config.logging_level);

    tracing::info!(
        model = %config.model.active_model,
        theme = %config.ui.theme,
        "starting Local Linux AI Assistant (Phase 2: UI shell)"
    );

    // Background async runtime for future phases; entered but not yet
    // driving anything of our own since nothing in Phase 1/2 needs it.
    let runtime = tokio::runtime::Runtime::new()?;
    let _guard = runtime.enter();

    let bus = EventBus::default();

    ui::run(bus).map_err(|e| AppError::Module {
        module: "ui",
        message: e.to_string(),
    })?;

    tracing::info!("UI closed, shutting down");
    Ok(())
}
