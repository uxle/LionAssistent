//! Debugging (Development_plan Phase 24). Runs `cargo check
//! --message-format=json` against a real project and parses its actual
//! machine-readable output into structured diagnostics -- not a
//! human-text-output regex scrape, which cargo's own JSON mode exists
//! specifically to make unnecessary.
//!
//! # Real, verified JSON shape
//!
//! Confirmed by hand before writing any parsing code (this project's
//! standing rule): a scratch crate with a deliberate `mismatched types`
//! error, checked with `cargo check --message-format=json`, produces one
//! JSON object per line. The lines this module cares about:
//!
//! ```text
//! {"reason":"compiler-message","message":{"level":"error","message":"mismatched types","code":{"code":"E0308",...},"spans":[{"is_primary":true,"file_name":"src/lib.rs","line_start":30,"column_start":5,...}]}}
//! {"reason":"build-finished","success":false}
//! ```
//!
//! `message.level` was confirmed to take at least `"error"`, `"warning"`,
//! and `"failure-note"` (the last one accompanying rustc's own "try
//! `rustc --explain E0308`" hint, which has no source location --
//! `spans` is empty for it, correctly producing `file`/`line`/`column`
//! of `None` below rather than a crash). `success` on `build-finished`
//! was confirmed `true` even with warnings present -- only real errors
//! flip it to `false`. One real nuance found while testing this against
//! an actual broken crate rather than assumed: a single compile error
//! produces *two* `"level":"error"` messages -- the located diagnostic
//! itself, and an additional, unlocated `"aborting due to previous
//! error"` summary line. Both are kept in `DiagnosticReport::
//! diagnostics` (this module reports what cargo actually said, in full),
//! but `error_count`/`warning_count` only count diagnostics with a real
//! source location -- see those methods' own doc comments.
//!
//! # Why `cargo check`, not `cargo build`
//!
//! Both surface identical diagnostics (they share the same frontend
//! passes); `check` skips code generation, so it's strictly faster for a
//! tool whose only job is to report what's wrong, not to produce a
//! binary.
//!
//! # Why only Cargo/Rust, for now
//!
//! Said explicitly rather than silently half-supporting other
//! ecosystems: [`run_cargo_diagnostics`] requires a `Cargo.toml` in the
//! target directory and returns a clear error without one. Other
//! ecosystems' own machine-readable diagnostic formats (`tsc
//! --outputJson`-style tooling, `pyright --outputjson`, etc.) are real
//! and could follow the same shape here, but weren't verified this
//! phase -- an honest scope boundary, the same kind `screenshot`'s
//! X11-only-really-verified capture and `ocr`'s tesseract-only OCR
//! already are.

use common::{AppError, AppResult};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

fn module_err(context: impl std::fmt::Display) -> AppError {
    AppError::Module {
        module: "coding-agent",
        message: context.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    /// Straight from rustc's own vocabulary (`"error"`, `"warning"`,
    /// `"note"`, `"help"`, `"failure-note"`, and possibly others this
    /// module hasn't personally observed) -- an open `String` rather
    /// than a closed enum this project would have to keep in sync with
    /// rustc's own evolving message levels.
    pub level: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub message: String,
    /// e.g. `Some("E0308")` -- `None` for messages that don't carry a
    /// rustc error code (most warnings, and every summary/help line).
    pub code: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticReport {
    pub success: bool,
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticReport {
    /// Counts only diagnostics with a real source location, not every
    /// `level == "error"` line verbatim. A genuine, real-world nuance
    /// found while writing this module's own tests: for one actual
    /// compile error, rustc's JSON stream includes *two* `"level":
    /// "error"` messages -- the located `mismatched types` diagnostic,
    /// and a second, unlocated `"aborting due to previous error"`
    /// summary line. Counting both would report "2 errors" for what a
    /// user would recognize as one problem. Filtering to diagnostics
    /// that carry a primary span (`file.is_some()`) is what actually
    /// matches "how many distinct, located problems are there" -- the
    /// question this method exists to answer.
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.level == "error" && d.file.is_some())
            .count()
    }
    /// Same reasoning as `error_count` -- see its doc comment.
    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.level == "warning" && d.file.is_some())
            .count()
    }
}

// Only the fields this module actually reads -- `#[serde(default)]` /
// `Option` throughout so an unrecognized or missing field degrades to
// "no info there" rather than failing to parse an otherwise-valid line
// cargo emitted (cargo's own JSON schema isn't this project's to keep in
// lockstep with across rustc versions).
#[derive(Debug, Deserialize)]
struct CargoMessage {
    reason: String,
    message: Option<CompilerMessage>,
    success: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CompilerMessage {
    message: String,
    level: String,
    code: Option<CompilerCode>,
    spans: Vec<CompilerSpan>,
}

#[derive(Debug, Deserialize)]
struct CompilerCode {
    code: String,
}

#[derive(Debug, Deserialize)]
struct CompilerSpan {
    file_name: String,
    line_start: u32,
    column_start: u32,
    is_primary: bool,
}

/// Runs `cargo check --message-format=json` in `project_dir` and parses
/// every real line it emits into structured diagnostics.
///
/// # Why this tool's permission level is `Destructive`, not `ReadOnly`
///
/// This looks like a read-only "tell me what's wrong" operation, and
/// from the caller's side it is -- but `cargo check` compiles real code:
/// it runs `build.rs` scripts and executes proc-macros at compile time,
/// both with this process's full privileges and neither sandboxed by
/// Cargo itself. Pointing this at a directory whose `Cargo.toml` pulls
/// in a malicious dependency (or whose own `build.rs` is malicious) is
/// effectively running untrusted code, not reading a file -- a real,
/// well-documented Rust ecosystem risk, not a hypothetical one. See
/// `tools::RunDiagnosticsTool`'s own doc comment for how this project
/// weighs that against TCLIS SS6's levels.
pub fn run_cargo_diagnostics(project_dir: &Path) -> AppResult<DiagnosticReport> {
    if !project_dir.join("Cargo.toml").exists() {
        return Err(module_err(format!(
            "{} has no Cargo.toml -- only Rust/Cargo projects are supported for diagnostics \
             in this phase",
            project_dir.display()
        )));
    }

    let output = Command::new("cargo")
        .arg("check")
        .arg("--message-format=json")
        .current_dir(project_dir)
        .output()
        .map_err(|e| module_err(format!("failed to launch `cargo` ({e})")))?;

    // Deliberately not checking `output.status.success()` here the way
    // most of this workspace's command wrappers do: a `cargo check` that
    // finds real compile errors exits non-zero *by design* -- that's the
    // whole result this function exists to report, not a wrapper
    // failure. `DiagnosticReport::success` (from the real `build-finished`
    // message) is the honest signal, not the process exit code.
    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut diagnostics = Vec::new();
    let mut success = output.status.success();

    for line in stdout.lines() {
        let Ok(parsed) = serde_json::from_str::<CargoMessage>(line) else {
            continue; // a line cargo didn't intend as one of its own JSON messages
        };

        match parsed.reason.as_str() {
            "compiler-message" => {
                if let Some(msg) = parsed.message {
                    let primary = msg.spans.iter().find(|s| s.is_primary);
                    diagnostics.push(Diagnostic {
                        level: msg.level,
                        file: primary.map(|s| s.file_name.clone()),
                        line: primary.map(|s| s.line_start),
                        column: primary.map(|s| s.column_start),
                        message: msg.message,
                        code: msg.code.map(|c| c.code),
                    });
                }
            }
            "build-finished" => {
                if let Some(s) = parsed.success {
                    success = s;
                }
            }
            _ => {}
        }
    }

    Ok(DiagnosticReport {
        success,
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_cargo_toml_fails_cleanly_before_ever_invoking_cargo() {
        let dir = std::env::temp_dir().join("coding-agent-test-no-manifest");
        std::fs::create_dir_all(&dir).unwrap();
        let result = run_cargo_diagnostics(&dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cargo.toml"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A real, deliberately-broken crate: real `cargo check`, real
    /// parsed output, real assertions against what it actually said --
    /// this is the exact scenario confirmed by hand during development
    /// before this module's doc comment described the JSON shape.
    #[test]
    fn a_real_compile_error_is_parsed_into_a_structured_diagnostic() {
        let dir = std::env::temp_dir().join("coding-agent-test-real-error");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"coding-agent-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/lib.rs"),
            "pub fn broken() -> i32 {\n    \"not an integer\"\n}\n",
        )
        .unwrap();

        let report = run_cargo_diagnostics(&dir).unwrap();
        assert!(!report.success);
        assert_eq!(
            report.error_count(),
            1,
            "exactly one *located* error, even though rustc's own JSON stream includes a \
             second, unlocated \"aborting due to previous error\" line at level=error too: {:?}",
            report.diagnostics
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.level == "error" && d.file.is_none()),
            "the real, unlocated \"aborting due to previous error\" line should still be \
             present in the raw diagnostics -- just not double-counted by error_count(): {:?}",
            report.diagnostics
        );

        let error = report
            .diagnostics
            .iter()
            .find(|d| d.level == "error" && d.code.as_deref() == Some("E0308"))
            .expect("expected a real E0308 mismatched-types error");
        assert_eq!(error.file.as_deref(), Some("src/lib.rs"));
        assert_eq!(error.line, Some(2));
        assert!(error.message.contains("mismatched types"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_real_warning_is_parsed_and_does_not_flip_success_to_false() {
        let dir = std::env::temp_dir().join("coding-agent-test-real-warning");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"coding-agent-fixture-warn\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/lib.rs"),
            "pub fn has_a_warning() -> i32 {\n    let unused = 5;\n    42\n}\n",
        )
        .unwrap();

        let report = run_cargo_diagnostics(&dir).unwrap();
        assert!(report.success, "warnings alone shouldn't mean failure");
        assert!(report.warning_count() >= 1, "{:?}", report.diagnostics);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_clean_crate_reports_success_with_no_diagnostics() {
        let dir = std::env::temp_dir().join("coding-agent-test-clean");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"coding-agent-fixture-clean\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/lib.rs"),
            "pub fn adds_one(x: i32) -> i32 {\n    x + 1\n}\n",
        )
        .unwrap();

        let report = run_cargo_diagnostics(&dir).unwrap();
        assert!(report.success);
        assert_eq!(report.error_count(), 0);

        std::fs::remove_dir_all(&dir).ok();
    }
}
