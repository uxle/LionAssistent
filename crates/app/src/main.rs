//! `app` -- binary entry point. Phase 1 gave us `common`, `config`,
//! `logger`, and `event-bus`; Phase 2 added the Slint UI shell (`ui`);
//! Phase 3 added `assistant-core`; Phase 4 added a real local model
//! (`llm-runtime`, via `model-registry`); Phase 5 added persistence
//! (`memory`) -- every processed message is saved to SQLite as it
//! happens, so conversation history survives a restart. Phase 18 changes
//! *when* and *how* that work happens without changing what it does:
//! model loading is deferred until the first real message
//! (`assistant_core::LazyResponseGenerator`) instead of blocking
//! startup, the background task's blocking calls move off the async
//! runtime (`utilities::run_blocking`) instead of running inline, the
//! embedder and memory search results are cached
//! (`embeddings::CachingEmbedder`, `memory::MemoryStore`'s own cache,
//! both sized from `config::PerformanceConfig`), and a `metrics::MetricsRegistry`
//! times the startup phases that are left so the tradeoff is visible,
//! not just asserted.
//!
//! `main` is deliberately synchronous, not `#[tokio::main]`: native GUI
//! backends need the real OS main thread, and Slint's `run()` blocks it
//! until the window closes. A multi-threaded Tokio runtime is built up
//! front and entered so both assistant-core's background task and the
//! UI's reply listener (spawned inside `ui::run`) can run on it.

use assistant_core::{
    AssistantCore, EchoResponseGenerator, LazyResponseGenerator, ResponseGenerator,
};
use common::{AppError, AppResult};
use config::{Config, ModelConfig, PerformanceConfig};
use embeddings::{CachingEmbedder, HashingEmbedder};
use event_bus::{Event, EventBus};
use memory::MemoryStore;
use metrics::MetricsRegistry;
use model_registry::ModelRegistry;
use std::path::Path;
use std::sync::Arc;

/// Discovers and loads the configured local model. Takes an owned
/// `ModelConfig` rather than `&Config` so it can be moved, whole, into
/// the `'static` closure `main` hands to `LazyResponseGenerator` --
/// deferred to first use, this now runs well after `main`'s own stack
/// frame has moved on to other startup work, so it can't borrow from it.
fn load_backend(model_config: ModelConfig) -> Box<dyn ResponseGenerator> {
    let models_dir = Path::new(&model_config.model_path);
    let mut registry = ModelRegistry::new();
    let found = registry.discover(
        models_dir,
        &model_config.active_model,
        "gemma",
        model_config.context_length,
    );

    if !found {
        tracing::warn!(
            dir = %models_dir.display(),
            model = %model_config.active_model,
            "no local model found ({model}.gguf + {model}.tokenizer.json) -- \
             using the Phase 3 stub backend. See docs/ROADMAP.md for how to add a real model.",
            model = model_config.active_model,
        );
        return Box::new(EchoResponseGenerator);
    }

    let info = registry
        .get(&model_config.active_model)
        .expect("just registered above");
    let generation = llm_runtime::GenerationConfig {
        temperature: model_config.temperature as f64,
        top_p: model_config.top_p as f64,
        max_new_tokens: model_config.max_tokens as usize,
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

fn open_memory(performance: &PerformanceConfig) -> AppResult<MemoryStore> {
    let db_path = Path::new("data/database/assistant.db");
    if let Some(dir) = db_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let embedder = CachingEmbedder::with_config(
        HashingEmbedder::default(),
        cache::CacheConfig {
            capacity: performance.embedding_cache_capacity,
            ttl: performance.cache_ttl(),
        },
    );
    let search_cache_config = cache::CacheConfig {
        capacity: performance.search_cache_capacity,
        ttl: performance.cache_ttl(),
    };
    MemoryStore::open_with_search_cache(db_path, Box::new(embedder), search_cache_config)
}

fn main() -> AppResult<()> {
    let metrics_registry = Arc::new(MetricsRegistry::new());

    let config = metrics_registry.time("startup_config_load", || {
        Config::load("config/default.toml")
    })?;
    logger::init(&config.logging_level);

    tracing::info!(
        model = %config.model.active_model,
        theme = %config.ui.theme,
        "starting Local Linux AI Assistant (Phase 18: performance)"
    );

    let runtime = tokio::runtime::Runtime::new()?;
    let _guard = runtime.enter();

    let bus = EventBus::default();

    // Model loading is deferred to the first real message instead of
    // happening here -- see `LazyResponseGenerator`'s doc comment for
    // why, and `startup phase timing` in the logs after `ui::run`
    // returns for how long it actually took once it did run.
    let backend = {
        let model_config = config.model.clone();
        let metrics_registry = metrics_registry.clone();
        Box::new(LazyResponseGenerator::new(move || {
            metrics_registry.time("model_load_deferred", || load_backend(model_config))
        }))
    };
    let mut assistant = AssistantCore::new(backend);
    let session_id = assistant.create_session(&config.model.active_model);

    let memory_store =
        metrics_registry.time("startup_memory_open", || open_memory(&config.performance))?;
    tracing::info!(
        sessions = memory_store.list_sessions()?.len(),
        "opened persistent memory at data/database/assistant.db"
    );

    // Background task: assistant-core answers whatever arrives on the
    // bus, and every processed turn is immediately persisted. `assistant`
    // and `memory_store` both move in whole -- nothing else needs them,
    // so no Arc<Mutex<_>> required. Each turn's actual work (model
    // inference -- possibly the deferred load above, on the very first
    // turn -- and the SQLite write) runs via `utilities::run_blocking`
    // instead of directly inline, so neither one ties up this task's
    // async worker thread while it runs; `assistant`/`memory_store` move
    // into that blocking closure and back out again each time so the
    // next turn can keep using them.
    {
        let mut events = bus.subscribe();
        let reply_bus = bus.clone();
        tokio::spawn(async move {
            let mut assistant = assistant;
            let mut memory_store = memory_store;
            while let Ok(event) = events.recv().await {
                let Event::UserMessageReceived {
                    session_id,
                    content,
                } = event
                else {
                    continue;
                };

                let outcome = utilities::run_blocking(move || {
                    let result = assistant.process_message(session_id, content);
                    let persist_error = match &result {
                        Ok(_) => assistant
                            .session(session_id)
                            .and_then(|session| memory_store.save_session(session).err()),
                        Err(_) => None,
                    };
                    (assistant, memory_store, result, persist_error)
                })
                .await;

                let (returned_assistant, returned_memory_store, result, persist_error) =
                    match outcome {
                        Ok(returned) => returned,
                        Err(e) => {
                            tracing::error!(
                                error = %e,
                                "background message-processing task panicked; stopping"
                            );
                            break;
                        }
                    };
                assistant = returned_assistant;
                memory_store = returned_memory_store;

                if let Some(e) = persist_error {
                    tracing::warn!(error = %e, "failed to persist session");
                }
                match result {
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

    let startup_snapshot = metrics_registry.snapshot();
    for (phase, summary) in startup_snapshot.slowest_timers(5) {
        tracing::info!(phase, avg_ms = summary.avg_ms, "startup phase timing");
    }

    ui::run(bus, session_id, &config.ui).map_err(|e| AppError::Module {
        module: "ui",
        message: e.to_string(),
    })?;

    tracing::info!("UI closed, shutting down");
    Ok(())
}
