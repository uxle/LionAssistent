//! `ui` -- the Slint desktop UI shell: main window, theme system, sidebar
//! navigation, chat view, and composer (Development_plan Phase 2).
//!
//! As of Phase 3, this crate no longer fakes a reply itself: sending a
//! message publishes `Event::UserMessageReceived` on the shared event
//! bus, and this crate separately subscribes for `Event::ResponseGenerated`
//! to render whatever answers it -- assistant-core's stub backend today,
//! a real LLM from Phase 4 on, without this crate needing to change.

slint::include_modules!();

use common::Id;
use event_bus::{Event, EventBus};
use std::rc::Rc;

/// Launches the window and blocks the calling thread until it is closed.
/// Must be called from the process's main thread, with a Tokio runtime
/// already entered (see `app`'s `main`), since this spawns a background
/// task to listen for replies.
pub fn run(bus: EventBus, session_id: Id) -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;

    let messages = Rc::new(slint::VecModel::<ChatMessage>::from(vec![ChatMessage {
        role: "system".into(),
        content: "Connected to the assistant core (stub backend -- llm-runtime lands in Phase 4)."
            .into(),
        time: format_time(),
    }]));
    window.set_messages(messages.into());

    // Outgoing: composer -> event bus, with an immediate optimistic
    // render of the user's own message (no reason to wait on anything
    // for text the user just typed themselves).
    {
        let window_weak = window.as_weak();
        let bus = bus.clone();
        window.on_send_message(move |text| {
            let text = text.to_string();
            if text.trim().is_empty() {
                return;
            }
            let Some(window) = window_weak.upgrade() else {
                return;
            };
            push_message(&window, "user", &text);
            bus.publish(Event::UserMessageReceived {
                session_id,
                content: text,
            });
        });
    }

    // Incoming: event bus -> chat view, for whatever eventually answers
    // UserMessageReceived. `invoke_from_event_loop` is required here since
    // this closure runs on a background Tokio task, not the UI thread.
    {
        let mut receiver = bus.subscribe();
        let window_weak = window.as_weak();
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv().await {
                let Event::ResponseGenerated {
                    session_id: reply_session,
                    content,
                } = event
                else {
                    continue;
                };
                if reply_session != session_id {
                    continue;
                }
                let window_weak = window_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(window) = window_weak.upgrade() else {
                        return;
                    };
                    push_message(&window, "assistant", &content);
                });
            }
        });
    }

    window.run()
}

fn push_message(window: &AppWindow, role: &str, content: &str) {
    let model = window.get_messages();
    let model = model
        .as_any()
        .downcast_ref::<slint::VecModel<ChatMessage>>()
        .expect("messages model is always the VecModel set in ui::run");
    model.push(ChatMessage {
        role: role.into(),
        content: content.into(),
        time: format_time(),
    });
}

/// Local wall-clock time for message bubble timestamps. `common::now()`
/// stays UTC for storage; display formatting is a UI-only concern.
fn format_time() -> slint::SharedString {
    chrono::Local::now().format("%H:%M").to_string().into()
}
