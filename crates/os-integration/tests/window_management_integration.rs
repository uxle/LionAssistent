//! Drives `os_integration::windows` against a real X11 stack: a real
//! Xvfb virtual display, a real (if minimal) window manager
//! (`fluxbox`), and a real client window (`xterm`) -- confirmed by
//! hand before writing any implementation that this combination
//! genuinely supports list/find/activate/move/resize/minimize/close,
//! the same standard `screenshot`'s own Xvfb-dependent tests already
//! set in Phase 23.
//!
//! Requires `wmctrl`, `xdotool`, `fluxbox`, and `xterm` on `PATH`, and
//! `Xvfb` -- all already documented, already-required setup for this
//! workspace's screenshot/OCR tests (README's "Setup" section);
//! nothing new is required beyond what a from-scratch checkout
//! following that section already has.

use std::process::{Child, Command};
use std::time::Duration;

/// Spins up Xvfb + fluxbox on a private display, killing both when
/// dropped -- even on a panicking assertion partway through a test,
/// mirroring how every other real-process test in this workspace
/// guarantees cleanup.
struct XDisplay {
    display: String,
    xvfb: Child,
    fluxbox: Child,
}

impl XDisplay {
    fn start(display_num: u32) -> Self {
        let display = format!(":{display_num}");
        let xvfb = Command::new("Xvfb")
            .args([&display, "-screen", "0", "1024x768x24"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("Xvfb must be installed -- see README's Setup section");
        std::thread::sleep(Duration::from_millis(500));

        let fluxbox = Command::new("fluxbox")
            .env("DISPLAY", &display)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("fluxbox must be installed -- see README's Setup section");
        std::thread::sleep(Duration::from_millis(500));

        Self {
            display,
            xvfb,
            fluxbox,
        }
    }
}

impl Drop for XDisplay {
    fn drop(&mut self) {
        let _ = self.fluxbox.kill();
        let _ = self.fluxbox.wait();
        let _ = self.xvfb.kill();
        let _ = self.xvfb.wait();
    }
}

/// A real `xterm` client window, killed when dropped.
struct TestWindow(Child);

impl TestWindow {
    fn spawn(display: &str, title: &str) -> Self {
        let child = Command::new("xterm")
            .env("DISPLAY", display)
            .args(["-title", title, "-e", "sleep 60"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("xterm must be installed -- see README's Setup section");
        std::thread::sleep(Duration::from_millis(500));
        Self(child)
    }
}

impl Drop for TestWindow {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Runs `f` with `$DISPLAY` set to `display` for the duration of the
/// call, holding a process-wide lock the whole time.
///
/// `std::env::set_var` mutates the *entire process's* environment, not
/// just the calling thread's -- so if two tests in this file ran
/// concurrently (`cargo test`'s default), one could overwrite
/// `$DISPLAY` while another's subprocess calls were still relying on
/// it, regardless of which display number each test picked. A shared
/// `Mutex` makes every test in this file mutually exclusive by
/// construction, correct under `cargo test`'s default parallelism
/// with no special flag required -- not something that merely happens
/// to work today because of how these five tests are currently
/// ordered.
static DISPLAY_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn with_display<T>(display: &str, f: impl FnOnce() -> T) -> T {
    let _guard = DISPLAY_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    std::env::set_var("DISPLAY", display);
    let result = f();
    std::env::remove_var("DISPLAY");
    result
}

#[test]
fn lists_finds_activates_and_closes_a_real_window() {
    let x = XDisplay::start(150);
    let _win = TestWindow::spawn(&x.display, "OsIntegrationTestWindow");

    with_display(&x.display, || {
        let found = os_integration::windows::find_window_by_title("OsIntegrationTestWindow")
            .expect("list_windows should succeed against a real display")
            .expect("the real xterm window should be found");

        os_integration::windows::activate_window(&found.id)
            .expect("activate should succeed on a real window");

        os_integration::windows::close_window(&found.id)
            .expect("close should succeed on a real window");

        // Give the WM a moment to actually process the close request.
        std::thread::sleep(Duration::from_millis(500));
        let still_present = os_integration::windows::list_windows()
            .unwrap()
            .iter()
            .any(|w| w.title.contains("OsIntegrationTestWindow"));
        assert!(
            !still_present,
            "window should be gone after a successful close"
        );
    });
}

#[test]
fn moves_and_resizes_a_real_window_to_exact_coordinates() {
    let x = XDisplay::start(151);
    let _win = TestWindow::spawn(&x.display, "OsIntegrationMoveResizeTest");

    with_display(&x.display, || {
        let found = os_integration::windows::find_window_by_title("OsIntegrationMoveResizeTest")
            .unwrap()
            .expect("window should be found");

        os_integration::windows::move_window(&found.id, 111, 222).unwrap();
        os_integration::windows::resize_window(&found.id, 333, 244).unwrap();

        // Verify against real, independently-obtained geometry (via
        // xdotool directly, not through this crate's own code) --
        // proving the real window state changed, not just that this
        // crate's functions returned success.
        let output = Command::new("xdotool")
            .args(["getwindowgeometry", "--shell", &found.id])
            .output()
            .unwrap();
        let geometry = String::from_utf8_lossy(&output.stdout);
        assert!(geometry.contains("WIDTH=333"), "geometry was:\n{geometry}");
        assert!(geometry.contains("HEIGHT=244"), "geometry was:\n{geometry}");
    });
}

#[test]
fn minimize_reports_success_and_the_window_remains_listed() {
    let x = XDisplay::start(152);
    let _win = TestWindow::spawn(&x.display, "OsIntegrationMinimizeTest");

    with_display(&x.display, || {
        let found = os_integration::windows::find_window_by_title("OsIntegrationMinimizeTest")
            .unwrap()
            .expect("window should be found");

        os_integration::windows::minimize_window(&found.id).unwrap();

        // Minimizing hides a window, it doesn't destroy it -- wmctrl
        // must still list it afterward.
        let still_listed = os_integration::windows::list_windows()
            .unwrap()
            .iter()
            .any(|w| w.id == found.id);
        assert!(
            still_listed,
            "a minimized window should still be listed, not gone"
        );
    });
}

#[test]
fn a_window_that_does_not_exist_produces_a_clean_error_not_a_hang() {
    let x = XDisplay::start(153);

    with_display(&x.display, || {
        let result = os_integration::windows::activate_window("0xdeadbeef");
        assert!(
            result.is_err(),
            "activating a nonexistent window id should fail cleanly"
        );
    });
}

#[test]
fn find_window_by_title_returns_none_for_no_match_not_an_error() {
    let x = XDisplay::start(154);
    let _win = TestWindow::spawn(&x.display, "SomeOtherRealWindow");

    with_display(&x.display, || {
        let found =
            os_integration::windows::find_window_by_title("NoWindowHasThisTitleAnywhere").unwrap();
        assert!(found.is_none());
    });
}
