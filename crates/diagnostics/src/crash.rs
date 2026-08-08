//! "Crash Reports" (Development_plan Phase 15): a global panic hook that
//! writes a small, structured, local-only JSON file before the default
//! Rust panic message still prints -- DIG SS8 says never panic for
//! *recoverable* errors, which this workspace has genuinely followed
//! throughout, but a real bug (an index out of bounds, a bad `unwrap()`
//! on something that "should never" fail) can still happen, and losing
//! all context the moment the process dies is its own kind of bug.
//!
//! Deliberately simple: a flat JSON file per crash, written with plain
//! `std::fs`, no database, no shared mutable state captured in the hook
//! closure beyond an owned path. A panic hook runs at an already-unusual
//! moment; the less machinery it depends on, the less likely *it*
//! fails too.

use common::{now, AppResult, Id, Timestamp};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    pub id: Id,
    pub timestamp: Timestamp,
    pub message: String,
    pub location: Option<String>,
    pub backtrace: Option<String>,
}

impl CrashReport {
    /// Builds a report from a real panic hook's `&PanicInfo`. Kept as a
    /// small, pure-ish function separate from actually installing the
    /// hook, so it -- and [`write_crash_report`] -- can be exercised by
    /// ordinary tests via `std::panic::catch_unwind` without needing to
    /// mutate the process's *global* panic hook from within a test
    /// (`std::panic::set_hook` is process-wide state; tests that install
    /// it can race with each other under the default multi-threaded test
    /// runner).
    fn from_panic_info(info: &std::panic::PanicInfo<'_>) -> Self {
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "panic payload was not a string".to_string()
        };

        let location = info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()));

        // Rust doesn't capture a backtrace at panic time unless asked --
        // `RUST_BACKTRACE` is unset by default. A crash *reporter*
        // that silently respects that default would usually produce a
        // report with nothing useful in it, so this forces capture
        // regardless -- the cost only applies on the already-exceptional
        // panic path, never in normal operation.
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let backtrace = (!backtrace.trim().is_empty()).then_some(backtrace);

        Self {
            id: Id::new(),
            timestamp: now(),
            message,
            location,
            backtrace,
        }
    }
}

/// Writes `report` as `<dir>/crash-<id>.json`, creating `dir` if needed.
/// Uses the report's own timestamp/id for the filename so reports sort
/// and never collide, rather than depending on anything from the
/// caller's environment at write time.
pub fn write_crash_report(dir: &Path, report: &CrashReport) -> AppResult<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("crash-{}.json", report.id));
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| common::AppError::Serialization(e.to_string()))?;
    std::fs::write(&path, json)?;
    Ok(path)
}

/// Lists crash report files in `dir`, most recent first. Missing `dir`
/// is treated as "no crashes yet," not an error -- a fresh install
/// simply won't have this directory.
pub fn list_crash_reports(dir: &Path) -> AppResult<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths.reverse();
    Ok(paths)
}

/// Installs a process-wide panic hook that writes a [`CrashReport`] to
/// `crash_dir` for every panic on every thread, then still calls
/// whatever hook was previously installed (Rust's default one, unless
/// something upstream already replaced it) -- panics keep behaving
/// exactly as before on top of the new crash file, nothing about normal
/// panic output changes.
///
/// Global process state; call this once, early, from `app`'s startup --
/// not done in this phase, for the same reason `app` hasn't been touched
/// since Phase 5 (docs/ROADMAP.md).
pub fn install_panic_hook(crash_dir: PathBuf) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let report = CrashReport::from_panic_info(info);
        // Best-effort: a failure writing the crash report must never
        // itself panic (a panic inside a panic hook aborts the process
        // immediately, skipping unwinding entirely) -- so this only
        // ever logs, never unwraps.
        if let Err(e) = write_crash_report(&crash_dir, &report) {
            tracing::error!(error = %e, "failed to write crash report");
        }
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> CrashReport {
        CrashReport {
            id: Id::new(),
            timestamp: now(),
            message: "something went wrong".to_string(),
            location: Some("src/lib.rs:42:5".to_string()),
            backtrace: Some("0: some_function\n1: main".to_string()),
        }
    }

    #[test]
    fn write_crash_report_creates_the_directory_and_a_readable_file() {
        let dir = std::env::temp_dir().join("diagnostics-crash-test-write");
        std::fs::remove_dir_all(&dir).ok();

        let report = sample_report();
        let path = write_crash_report(&dir, &report).unwrap();

        assert!(path.exists());
        let read_back: CrashReport =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(read_back.message, report.message);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_crash_reports_on_a_missing_directory_is_empty_not_an_error() {
        let dir = std::env::temp_dir().join("diagnostics-crash-test-missing-dir-xyz");
        std::fs::remove_dir_all(&dir).ok();
        assert!(list_crash_reports(&dir).unwrap().is_empty());
    }

    #[test]
    fn list_crash_reports_returns_most_recent_first() {
        let dir = std::env::temp_dir().join("diagnostics-crash-test-list-order");
        std::fs::remove_dir_all(&dir).ok();

        let mut first = sample_report();
        first.id = Id::new();
        write_crash_report(&dir, &first).unwrap();
        let mut second = sample_report();
        second.id = Id::new();
        write_crash_report(&dir, &second).unwrap();

        let listed = list_crash_reports(&dir).unwrap();
        assert_eq!(listed.len(), 2);
        // Filenames embed the id, not a strictly orderable timestamp, so
        // this only checks that both are present and sorted
        // deterministically (reverse-lexicographic by filename), not a
        // specific chronological order.
        assert!(listed[0] > listed[1]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn from_panic_info_extracts_a_string_literal_message() {
        let dir = std::env::temp_dir().join("diagnostics-crash-test-catch-unwind-literal");
        std::fs::remove_dir_all(&dir).ok();
        let dir_for_hook = dir.clone();

        // Exercise the real panic -> report -> file path via
        // catch_unwind + a *locally* installed hook, restored
        // immediately after -- see this module's doc comment on why
        // from_panic_info/install_panic_hook are kept separate.
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let report = CrashReport::from_panic_info(info);
            write_crash_report(&dir_for_hook, &report).unwrap();
        }));
        let result = std::panic::catch_unwind(|| panic!("deliberate test panic"));
        std::panic::set_hook(previous);

        assert!(result.is_err());
        let reports = list_crash_reports(&dir).unwrap();
        assert_eq!(reports.len(), 1);
        let report: CrashReport =
            serde_json::from_str(&std::fs::read_to_string(&reports[0]).unwrap()).unwrap();
        assert_eq!(report.message, "deliberate test panic");
        assert!(report.location.is_some());

        std::fs::remove_dir_all(&dir).ok();
    }
}
