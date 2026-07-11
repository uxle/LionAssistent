//! `ui` -- the Slint desktop UI shell: main window, theme system, sidebar
//! navigation, chat view, and composer (Development_plan Phase 2).
//!
//! This crate does not talk to a real assistant core or LLM yet (Phase 3
//! and Phase 4 respectively). Sending a message publishes
//! `Event::UserMessageReceived` on the shared event bus -- so the wiring
//! is real -- and appends a clearly-labeled placeholder reply locally, so
//! the chat bubble rendering (user / assistant / system) can be exercised
//! end-to-end before there's a real backend to respond.
//!
//! Subscribing to `Event::ResponseGenerated` so a real backend can push
//! replies back into this same window is deliberately deferred to Phase 3,
//! where there will actually be a producer to build and test it against.

slint::include_modules!();

use event_bus::{Event, EventBus};
use std::rc::Rc;

/// Launches the window and blocks the calling thread until it is closed.
/// Must be called from the process's main thread.
pub fn run(bus: EventBus) -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;

    let messages = Rc::new(slint::VecModel::<ChatMessage>::from(vec![ChatMessage {
        role: "system".into(),
        content: "Phase 1 + Phase 2 online. No assistant core yet -- \
                  replies below are placeholders."
            .into(),
        time: format_time(),
    }]));
    window.set_messages(messages.into());

    {
        let window_weak = window.as_weak();
        let session_id = common::Id::new();
        window.on_send_message(move |text| {
            let text = text.to_string();
            if text.trim().is_empty() {
                return;
            }
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            let model = window.get_messages();
            let model = model
                .as_any()
                .downcast_ref::<slint::VecModel<ChatMessage>>()
                .expect("messages model is always the VecModel set in ui::run");

            model.push(ChatMessage {
                role: "user".into(),
                content: text.clone().into(),
                time: format_time(),
            });

            bus.publish(Event::UserMessageReceived {
                session_id,
                content: text,
            });

            model.push(ChatMessage {
                role: "assistant".into(),
                content: "(placeholder reply -- assistant-core lands in Phase 3)".into(),
                time: format_time(),
            });
        });
    }

    window.run()
}

/// Local wall-clock time for message bubble timestamps. `common::now()`
/// stays UTC for storage; display formatting is a UI-only concern.
fn format_time() -> slint::SharedString {
    chrono::Local::now().format("%H:%M").to_string().into()
}
