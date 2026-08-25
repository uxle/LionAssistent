//! Desktop window management (Development_plan Phase 30 "Desktop
//! Automation"), via real `wmctrl`/`xdotool` subprocess calls -- no
//! shell, argv arrays directly, the same pattern
//! `linux_services::terminal` already established for exactly this
//! reason (TCLIS SS4: "No unrestricted shell execution").
//!
//! Fully real, fully testable in this sandbox: confirmed empirically
//! (not assumed) that Xvfb plus a real, if minimal, window manager
//! (`fluxbox`) plus a real client window (`xterm`) is enough for
//! `wmctrl`/`xdotool` to genuinely list, find, move, resize, minimize,
//! and close windows -- `tests/window_management_integration.rs`
//! drives exactly that real stack, the same testing posture
//! `screenshot`'s own Xvfb-dependent tests already established in
//! Phase 23.
//!
//! [`close_window`] deliberately sends a polite `_NET_CLOSE_WINDOW`
//! request (`wmctrl -c`) -- the same request a window's own titlebar
//! "X" button sends -- rather than a forceful `XKillClient`
//! (`xdotool windowkill`). A well-behaved application handles this by
//! prompting to save unsaved work the same way it would for a normal
//! user-initiated close; a forceful kill bypasses that entirely. A
//! force-kill tool is deliberately not included here -- if a window
//! doesn't respond to a polite close, that's a real, named limitation
//! of this module, not something routed around by reaching for a
//! blunter instrument by default.

use common::{AppError, AppResult};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    /// `wmctrl`'s window id, e.g. `"0x0040000c"` -- opaque, passed back
    /// to the other functions in this module verbatim, never parsed.
    pub id: String,
    pub desktop: i32,
    pub title: String,
}

fn run(command: &str, args: &[&str]) -> AppResult<std::process::Output> {
    Command::new(command)
        .args(args)
        .output()
        .map_err(|e| AppError::Module {
            module: "os-integration::windows",
            message: format!("failed to run {command}: {e} (is it installed?)"),
        })
}

fn require_success(command: &str, output: &std::process::Output) -> AppResult<()> {
    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::Module {
            module: "os-integration::windows",
            message: format!(
                "{command} exited with {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        })
    }
}

/// Lists every window `wmctrl` currently knows about. Requires a
/// reachable X display (`$DISPLAY`) and a running window manager --
/// `wmctrl` itself returns a real, catchable error (not a hang) when
/// neither is available, surfaced here rather than swallowed.
pub fn list_windows() -> AppResult<Vec<WindowInfo>> {
    let output = run("wmctrl", &["-l"])?;
    require_success("wmctrl -l", &output)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().filter_map(parse_wmctrl_line).collect())
}

/// `wmctrl -l` output is whitespace-columnar:
/// `<id> <desktop> <client-machine> <title...>`. Uses
/// `split_whitespace` for the first three columns specifically because
/// it treats a *run* of whitespace as one separator -- `wmctrl`'s
/// real output right-pads the id column with extra spaces for
/// alignment, and a naive `splitn(4, char::is_whitespace)` treats each
/// individual space in that padding as its own split point, consuming
/// the split budget before ever reaching the title (an early version
/// of this function had exactly that bug, caught by its own test
/// against a real multi-word title). The title is rejoined with single
/// spaces rather than sliced from the original line's byte offsets --
/// simpler, and real window titles containing meaningful multiple
/// consecutive spaces are not a real case worth the extra complexity
/// of preserving exactly.
fn parse_wmctrl_line(line: &str) -> Option<WindowInfo> {
    let mut tokens = line.split_whitespace();
    let id = tokens.next()?.to_string();
    let desktop = tokens.next()?.parse().ok()?;
    let _client_machine = tokens.next()?;
    let title = tokens.collect::<Vec<_>>().join(" ");
    Some(WindowInfo { id, desktop, title })
}

/// Finds the first window whose title contains `substr`
/// (case-sensitive, matching `wmctrl`'s own default matching).
pub fn find_window_by_title(substr: &str) -> AppResult<Option<WindowInfo>> {
    Ok(list_windows()?
        .into_iter()
        .find(|w| w.title.contains(substr)))
}

/// Brings a window to the front and gives it input focus.
pub fn activate_window(id: &str) -> AppResult<()> {
    let output = run("wmctrl", &["-i", "-a", id])?;
    require_success("wmctrl -a", &output)
}

/// Moves a window so its top-left corner is at `(x, y)` in screen
/// coordinates.
pub fn move_window(id: &str, x: i32, y: i32) -> AppResult<()> {
    let output = run(
        "xdotool",
        &["windowmove", id, &x.to_string(), &y.to_string()],
    )?;
    require_success("xdotool windowmove", &output)
}

/// Resizes a window to `width` x `height` pixels.
pub fn resize_window(id: &str, width: u32, height: u32) -> AppResult<()> {
    let output = run(
        "xdotool",
        &["windowsize", id, &width.to_string(), &height.to_string()],
    )?;
    require_success("xdotool windowsize", &output)
}

/// Minimizes (iconifies) a window without closing it.
pub fn minimize_window(id: &str) -> AppResult<()> {
    let output = run("xdotool", &["windowminimize", id])?;
    require_success("xdotool windowminimize", &output)
}

/// Requests that a window close, politely -- see this module's own
/// docs for why this is `_NET_CLOSE_WINDOW` (`wmctrl -c`), not a
/// forceful kill.
pub fn close_window(id: &str) -> AppResult<()> {
    let output = run("wmctrl", &["-i", "-c", id])?;
    require_success("wmctrl -c", &output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_real_wmctrl_line_with_a_multi_word_title() {
        let line = "0x0040000c  0 vm TestWindow - a document.txt";
        let info = parse_wmctrl_line(line).unwrap();
        assert_eq!(info.id, "0x0040000c");
        assert_eq!(info.desktop, 0);
        assert_eq!(info.title, "TestWindow - a document.txt");
    }

    #[test]
    fn parses_a_line_with_no_title_as_an_empty_string() {
        let line = "0x00400001  1 vm";
        let info = parse_wmctrl_line(line).unwrap();
        assert_eq!(info.title, "");
    }

    #[test]
    fn a_negative_desktop_number_parses_fine_sticky_windows() {
        // wmctrl reports -1 for windows pinned to all desktops.
        let line = "0x00400002  -1 vm Sticky Window";
        let info = parse_wmctrl_line(line).unwrap();
        assert_eq!(info.desktop, -1);
    }

    #[test]
    fn rejects_a_malformed_line_missing_columns() {
        assert!(parse_wmctrl_line("0x0040000c").is_none());
        assert!(parse_wmctrl_line("").is_none());
    }

    #[test]
    fn a_missing_binary_produces_a_clear_error_not_a_panic() {
        let err = run("this-binary-does-not-exist-anywhere", &[]).unwrap_err();
        assert!(err.to_string().contains("is it installed"));
    }
}
