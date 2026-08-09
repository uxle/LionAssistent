//! `ui` -- the Slint desktop UI shell: main window, theme system, sidebar
//! navigation, chat view, and composer (Development_plan Phase 2; polish
//! -- animations, accessibility, responsive layout, the `ListView`
//! performance swap -- from Phase 17).
//!
//! As of Phase 3, this crate no longer fakes a reply itself: sending a
//! message publishes `Event::UserMessageReceived` on the shared event
//! bus, and this crate separately subscribes for `Event::ResponseGenerated`
//! to render whatever answers it -- assistant-core's stub backend today,
//! a real LLM from Phase 4 on, without this crate needing to change.
//!
//! Like every other file in this crate, none of this has been
//! compile-verified in this sandbox -- Slint itself has been blocked
//! here since Phase 2 (rustc 1.77+ required; docs/ROADMAP.md has the
//! full story, including a fresh, much deeper investigation done for
//! Phase 17 specifically). The one piece of Phase 17 that *is* compiled
//! and tested here is `config::UiConfig`'s new
//! `reduced_motion`/`high_contrast`/`text_scale` fields (`config` has no
//! Slint dependency) -- see `docs/ROADMAP.md`'s Phase 17 notes for why
//! that split matters.

slint::include_modules!();

use common::Id;
use event_bus::{Event, EventBus};
use std::rc::Rc;

/// Launches the window and blocks the calling thread until it is closed.
/// Must be called from the process's main thread, with a Tokio runtime
/// already entered (see `app`'s `main`), since this spawns a background
/// task to listen for replies.
///
/// `ui_config` (Phase 17) seeds the `Palette`/`Motion` globals --
/// `config::UiConfig::effective_text_scale()`, not the raw
/// `text_scale`, is what actually gets applied (see that method's doc
/// comment for why the two can differ).
pub fn run(
    bus: EventBus,
    session_id: Id,
    ui_config: &config::UiConfig,
) -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;

    window
        .global::<Palette>()
        .set_dark(ui_config.theme != "light");
    window
        .global::<Palette>()
        .set_high_contrast(ui_config.high_contrast);
    window
        .global::<Palette>()
        .set_text_scale(ui_config.effective_text_scale());
    window
        .global::<Motion>()
        .set_reduced(ui_config.reduced_motion);

    let messages = Rc::new(slint::VecModel::<ChatMessage>::from(vec![ChatMessage {
        role: "system".into(),
        content: "Connected to the assistant core (stub backend -- llm-runtime lands in Phase 4)."
            .into(),
        time: format_time(),
    }]));
    window.set_messages(messages.into());

    // Outgoing: composer -> event bus, with an immediate optimistic
    // render of the user's own message (no reason to wait on anything
    // for text the user just typed themselves) and `waiting: true` so
    // the thinking indicator (Phase 17) shows until a reply lands.
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
            window.set_waiting(true);
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
                    window.set_waiting(false);
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
