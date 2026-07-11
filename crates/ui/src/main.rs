//! Standalone dev runner: `cargo run -p ui` launches just the window,
//! wired to its own throwaway event bus. Useful for iterating on the UI
//! without going through the full `app` binary. The real entry point is
//! `crates/app`, which shares one event bus across the whole process.

fn main() -> Result<(), slint::PlatformError> {
    ui::run(event_bus::EventBus::default())
}
