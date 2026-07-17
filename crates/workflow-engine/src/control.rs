//! Cooperative pause/resume/cancel for a running workflow (Ai_reasoning
//! SS14: "Cancellation", "Pause and resume"). Checked *between* steps,
//! not mid-step -- there's no way to interrupt a tool call already in
//! flight without the tool itself supporting cancellation, which
//! `tool_framework::Tool` doesn't (Phase 7 made that trait deliberately
//! simple). Pausing between steps is still a real, useful, testable
//! guarantee: a long workflow genuinely stops advancing to its next step
//! until resumed, and genuinely stops for good when cancelled.

use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Running,
    Paused,
    Cancelled,
}

/// A cheaply-cloneable handle: clone it to hand one copy to whatever's
/// executing the workflow and keep another to control it from elsewhere.
#[derive(Debug, Clone)]
pub struct RunControl {
    state: Arc<Mutex<State>>,
}

impl Default for RunControl {
    fn default() -> Self {
        Self::new()
    }
}

impl RunControl {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State::Running)),
        }
    }

    pub fn pause(&self) {
        let mut state = self.state.lock().expect("RunControl mutex poisoned");
        if *state == State::Running {
            *state = State::Paused;
        }
    }

    pub fn resume(&self) {
        let mut state = self.state.lock().expect("RunControl mutex poisoned");
        if *state == State::Paused {
            *state = State::Running;
        }
    }

    pub fn cancel(&self) {
        *self.state.lock().expect("RunControl mutex poisoned") = State::Cancelled;
    }

    pub fn is_cancelled(&self) -> bool {
        *self.state.lock().expect("RunControl mutex poisoned") == State::Cancelled
    }

    pub fn is_paused(&self) -> bool {
        *self.state.lock().expect("RunControl mutex poisoned") == State::Paused
    }

    /// Blocks (async) while paused. Returns `true` once clear to
    /// proceed, `false` if cancelled while waiting -- the executor
    /// checks this before starting each step.
    pub async fn proceed(&self) -> bool {
        loop {
            let state = *self.state.lock().expect("RunControl mutex poisoned");
            match state {
                State::Running => return true,
                State::Cancelled => return false,
                State::Paused => tokio::time::sleep(Duration::from_millis(10)).await,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn starts_running_and_proceeds_immediately() {
        assert!(RunControl::new().proceed().await);
    }

    #[tokio::test]
    async fn cancel_makes_proceed_return_false() {
        let control = RunControl::new();
        control.cancel();
        assert!(!control.proceed().await);
        assert!(control.is_cancelled());
    }

    #[tokio::test]
    async fn pause_genuinely_blocks_until_resumed() {
        let control = RunControl::new();
        control.pause();
        assert!(control.is_paused());

        let waiter = {
            let control = control.clone();
            tokio::spawn(async move { control.proceed().await })
        };

        // Give the waiter a moment to actually start waiting, then
        // confirm it hasn't finished -- it's genuinely blocked, not
        // returning immediately.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!waiter.is_finished());

        control.resume();
        let proceeded = tokio::time::timeout(Duration::from_millis(200), waiter)
            .await
            .expect("waiter should finish shortly after resume")
            .unwrap();
        assert!(proceeded);
    }

    #[test]
    fn pause_while_already_cancelled_does_not_un_cancel() {
        let control = RunControl::new();
        control.cancel();
        control.pause();
        assert!(control.is_cancelled());
        assert!(!control.is_paused());
    }
}
