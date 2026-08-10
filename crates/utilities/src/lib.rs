//! `utilities` -- small, dependency-light helper functions shared across
//! the workspace (Development_plan Phase 1, DIG SS4 repository layout).
//!
//! [`run_blocking`] (Development_plan Phase 18 "Async Improvements") is
//! the one function here that isn't dependency-free -- it needs `tokio`
//! and `common` -- but it belongs in this crate rather than a new one:
//! it's exactly the kind of small, shared helper this crate exists for,
//! just one that happens to need two already-everywhere workspace
//! dependencies to do its job.

use common::{AppError, AppResult};

/// Runs `f` on Tokio's blocking thread pool via
/// [`tokio::task::spawn_blocking`], so genuinely blocking work (SQLite
/// calls, model inference, anything synchronous-by-design -- see
/// `network::HttpClient`'s own doc comment for why several of this
/// workspace's stateful crates are deliberately synchronous rather than
/// async) never ties up an async-runtime worker thread while it runs.
///
/// `tool_framework::ToolRouter::execute` has done its own version of
/// this since Phase 7, bespoke, for tool execution specifically; this is
/// the same pattern promoted into one shared, tested helper for every
/// *other* call site that needs it -- starting with `app`'s background
/// task (Development_plan Phase 18 checklist), which called blocking
/// `assistant_core`/`memory` methods directly from inside a `tokio::spawn`d
/// task before this phase.
///
/// A panic inside `f` becomes a clean [`AppError`] instead of unwinding
/// across the task boundary (DIG SS8: never let a recoverable failure
/// crash the app) -- proven by
/// [`tests::a_panic_inside_becomes_a_clean_error_not_a_crash`], not just
/// asserted in this comment.
pub async fn run_blocking<T, F>(f: F) -> AppResult<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f).await.map_err(|e| {
        let detail = if e.is_panic() {
            "blocking task panicked"
        } else {
            "blocking task was cancelled"
        };
        AppError::Module {
            module: "utilities",
            message: format!("{detail}: {e}"),
        }
    })
}

/// Human-readable byte size formatting, used by system-info and file-size
/// related tools (TCLIS SS4 "File Management", "System").
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/// Truncates a string to at most `max_chars` characters, appending an
/// ellipsis if truncated. Used when summarizing conversation history for
/// the context builder (Ai_reasoning SS8).
pub fn truncate(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        input.to_string()
    } else {
        let truncated: String = input.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{truncated}\u{2026}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KB");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn truncates_long_strings() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hell\u{2026}");
    }

    #[tokio::test]
    async fn runs_blocking_work_and_returns_its_result() {
        let result = run_blocking(|| 2 + 2).await.unwrap();
        assert_eq!(result, 4);
    }

    #[tokio::test]
    async fn a_panic_inside_becomes_a_clean_error_not_a_crash() {
        let result = run_blocking(|| -> i32 { panic!("boom") }).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("panicked"));
    }

    #[tokio::test]
    async fn owned_state_can_be_moved_in_and_returned_back_out() {
        // Mirrors the real `app` shape: own some state, do blocking work
        // with it, get both the state and the outcome back so the next
        // loop iteration can keep using it.
        let state = vec![1, 2, 3];
        let (state, sum): (Vec<i32>, i32) = run_blocking(move || {
            let sum = state.iter().sum();
            (state, sum)
        })
        .await
        .unwrap();

        assert_eq!(sum, 6);
        assert_eq!(state.len(), 3);
    }

    #[tokio::test]
    async fn does_not_block_the_async_runtime_while_it_runs() {
        use std::time::Duration;

        // If run_blocking ran `f` inline on the calling task instead of
        // Tokio's blocking pool, this sleeping timer -- spawned first,
        // on the same runtime -- would never get a chance to fire before
        // the 200ms std::thread::sleep below finishes.
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fired_clone = fired.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            fired_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        run_blocking(|| std::thread::sleep(Duration::from_millis(200)))
            .await
            .unwrap();

        assert!(
            fired.load(std::sync::atomic::Ordering::SeqCst),
            "the concurrent timer should have fired while the blocking work ran"
        );
    }
}
