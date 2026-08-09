//! Standalone dev runner: `cargo run -p ui` launches just the window,
//! wired to its own throwaway event bus and runtime (nothing will ever
//! answer, since assistant-core isn't wired up here -- that's `app`).
//! Useful for iterating on the UI in isolation.

fn main() -> Result<(), slint::PlatformError> {
    let runtime = tokio::runtime::Runtime::new().expect("failed to start Tokio runtime");
    let _guard = runtime.enter();
    ui::run(
        event_bus::EventBus::default(),
        common::Id::new(),
        &config::UiConfig::default(),
    )
}
