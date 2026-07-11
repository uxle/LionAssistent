//! `app` -- binary entry point. Phase 1 gave us `common`, `config`,
//! `logger`, and `event-bus`; Phase 2 added the Slint UI shell (`ui`);
//! Phase 3 added `assistant-core`; Phase 4 adds a real local model
//! (`llm-runtime`, via `model-registry`), used automatically when found
//! and falling back to the Phase 3 stub with a clear log message when not.
//!
//! `main` is deliberately synchronous, not `#[tokio::main]`: native GUI
//! backends need the real OS main thread, and Slint's `run()` blocks it
//! until the window closes. A multi-threaded Tokio runtime is built up
//! front and entered so both assistant-core's background task and the
//! UI's reply listener (spawned inside `ui::run`) can run on it.

use assistant_core::{AssistantCore, EchoResponseGenerator, ResponseGenerator};
use common::{AppError, AppResult};
use config::Config;
use event_bus::{Event, EventBus};
use model_registry::ModelRegistry;
use std::path::Path;

fn load_backend(config: &Config) -> Box<dyn ResponseGenerator> {
    let models_dir = Path::new(&config.model.model_path);
    let mut registry = ModelRegistry::new();
    let found = registry.discover(
        models_dir,
        &config.model.active_model,
        "gemma",
        config.model.context_length,
    );

    if !found {
        tracing::warn!(
            dir = %models_dir.display(),
            model = %config.model.active_model,
            "no local model found ({model}.gguf + {model}.tokenizer.json) -- \
             using the Phase 3 stub backend. See docs/ROADMAP.md for how to add a real model.",
            model = config.model.active_model,
        );
        return Box::new(EchoResponseGenerator);
    }

    let info = registry
        .get(&config.model.active_model)
        .expect("just registered above");
    let generation = llm_runtime::GenerationConfig {
        temperature: config.model.temperature as f64,
        top_p: config.model.top_p as f64,
        max_new_tokens: config.model.max_tokens as usize,
        ..Default::default()
    };

    match llm_runtime::CandleGemmaBackend::load(info, generation) {
        Ok(backend) => {
            tracing::info!(model = %info.name, "loaded local model");
            Box::new(backend)
        }
        Err(e) => {
            tracing::warn!(error = %e, "found model files but failed to load them -- using the stub backend");
            Box::new(EchoResponseGenerator)
        }
    }
}

fn main() -> AppResult<()> {
    let config = Config::load("config/default.toml")?;
    logger::init(&config.logging_level);

    tracing::info!(
        model = %config.model.active_model,
        theme = %config.ui.theme,
        "starting Local Linux AI Assistant (Phase 4: LLM runtime)"
    );

    let runtime = tokio::runtime::Runtime::new()?;
    let _guard = runtime.enter();

    let bus = EventBus::default();

    let backend = load_backend(&config);
    let mut assistant = AssistantCore::new(backend);
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
