//! Screen capture (Development_plan Phase 23 "Screenshot Analysis";
//! anticipated generally as "Vision support" in Archd SS18). Like the rest
//! of this workspace's Linux integration (Archd Layer 5, TCLIS SS4), this
//! wraps one specific external tool per session type rather than talking
//! to the display server directly -- the same "safe wrapper, not raw
//! access" shape `linux-services` already established for audio (`pactl`),
//! notifications (`notify-send`), and clipboard (`xclip`/`wl-copy`).
//!
//! X11 sessions capture through ImageMagick's `import -window root`, which
//! works uniformly across GNOME/KDE/Xfce/Cinnamon on X11 (TCLIS SS14 wants
//! exactly this: one code path, no per-desktop branching) because X11
//! itself exposes one root window regardless of which window manager is
//! drawing to it. Wayland sessions capture through `grim`, which is
//! **not** the same kind of universal answer: `grim` only works on
//! wlroots-based compositors (Sway, and similar). GNOME Shell's and KDE
//! Plasma's own Wayland compositors deliberately block direct screen
//! access for security and require going through the XDG Desktop Portal
//! `org.freedesktop.portal.Screenshot` D-Bus interface instead, which
//! isn't implemented here. This is a real, documented gap, not an
//! oversight -- see docs/ROADMAP.md.
//!
//! # Verification story
//!
//! Unlike `linux-services`' audio/display/notification/clipboard modules
//! (Phase 8's sandbox had no sound server, backlight device, notification
//! daemon, or clipboard selection to succeed against, so those only prove
//! graceful failure), this crate's success path **is** verified for real
//! here: Xvfb provides a genuine, if headless, X11 server, and
//! `capture_x11` was confirmed end-to-end against it during development
//! (real `import` invocation, real PNG written, real dimensions read back
//! matching the configured virtual screen size). Running its tests for
//! real requires the same setup:
//!
//! ```bash
//! apt-get install -y imagemagick xvfb
//! Xvfb :99 -screen 0 1024x768x24 &
//! export DISPLAY=:99
//! cargo test -p screenshot
//! ```
//!
//! Without a reachable `DISPLAY`, the capture-dependent tests fail
//! outright rather than silently skipping -- matching this project's
//! standing preference (see `voice`'s README note) for an honest failure
//! over a green checkmark that didn't prove anything. The Wayland path's
//! *success* case has no equivalent here (there's no wlroots compositor
//! to stand up in this sandbox the way Xvfb stands in for a real X
//! server); only its failure mode (missing `grim` binary) is exercised.

use common::{AppError, AppResult};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub mod tools;

/// Wraps a `std::io::Error` (or an equivalent failure) with which
/// operation it came from, matching `linux-services`'s `io_err` helper
/// (Apis SS16: every error should carry `module`/`message`).
fn module_err(context: impl std::fmt::Display) -> AppError {
    AppError::Module {
        module: "screenshot",
        message: context.to_string(),
    }
}

/// The result of a successful capture (TCLIS SS8 "Standard Result
/// Format": callers get structured data back, not just a path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotInfo {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub captured_at_unix_ms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionKind {
    X11,
    Wayland,
}

/// Chooses which capture path to use. Wayland is checked first: a session
/// with both `WAYLAND_DISPLAY` and `DISPLAY` set (common under XWayland
/// compatibility shims) is a native Wayland session with X11 available
/// only for legacy app compatibility, so capturing through the native
/// protocol is the more faithful choice. Neither variable set means no
/// display server is reachable at all -- this sandbox's own default
/// state, and any real headless session's.
fn detect_session_from(
    wayland_display: Option<&str>,
    display: Option<&str>,
) -> AppResult<SessionKind> {
    if wayland_display.is_some() {
        Ok(SessionKind::Wayland)
    } else if display.is_some() {
        Ok(SessionKind::X11)
    } else {
        Err(module_err(
            "no WAYLAND_DISPLAY or DISPLAY is set -- there's no display server for this \
             session to capture from (TCLIS SS14 desktop integration assumes a desktop \
             session exists; a headless session like this sandbox's own default state doesn't \
             have one)",
        ))
    }
}

fn detect_session() -> AppResult<SessionKind> {
    detect_session_from(
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        std::env::var("DISPLAY").ok().as_deref(),
    )
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Captures the full screen and writes a PNG to `output_dir` (created if
/// missing), returning its path and real, read-back dimensions.
pub fn capture_full_screen(output_dir: &Path) -> AppResult<ScreenshotInfo> {
    std::fs::create_dir_all(output_dir).map_err(|e| {
        module_err(format!(
            "creating output directory {}: {e}",
            output_dir.display()
        ))
    })?;

    let session = detect_session()?;
    let path = output_dir.join(format!("screenshot-{}.png", now_ms()));

    match session {
        SessionKind::X11 => capture_x11(&path)?,
        SessionKind::Wayland => capture_wayland(&path)?,
    }

    let (width, height) = read_png_dimensions(&path)?;
    Ok(ScreenshotInfo {
        path,
        width,
        height,
        captured_at_unix_ms: now_ms(),
    })
}

/// `import -window root` grabs X11's root window, which is the whole
/// screen regardless of which window manager or desktop environment is
/// running -- the one place X11 genuinely gives every desktop the same
/// answer (TCLIS SS14).
fn capture_x11(path: &Path) -> AppResult<()> {
    let output = Command::new("import")
        .args(["-window", "root", "-silent"])
        .arg(path)
        .output()
        .map_err(|e| {
            module_err(format!(
                "failed to launch `import` (is imagemagick installed? {e})"
            ))
        })?;

    if !output.status.success() {
        return Err(module_err(format!(
            "`import` exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

/// `grim` with no arguments captures every output composited together.
/// Only works on wlroots-based compositors -- see this module's doc
/// comment for why that's a real limitation, not a bug.
fn capture_wayland(path: &Path) -> AppResult<()> {
    let output = Command::new("grim").arg(path).output().map_err(|e| {
        module_err(format!(
            "failed to launch `grim` ({e}). Note: `grim` only captures on wlroots-based \
             compositors (e.g. Sway) -- GNOME Shell and KDE Plasma's Wayland sessions block \
             direct capture and need the XDG Desktop Portal screenshot interface instead, \
             which this crate doesn't implement yet (see docs/ROADMAP.md)"
        ))
    })?;

    if !output.status.success() {
        return Err(module_err(format!(
            "`grim` exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

/// Reads width/height straight out of the PNG's `IHDR` chunk rather than
/// pulling in an image-decoding crate for two integers. This isn't a
/// shortcut specific to this tool's output: the PNG spec *requires* IHDR
/// to be the very first chunk, at a fixed byte offset, in every valid
/// PNG -- so this is exact for any encoder, not just ImageMagick/grim's.
/// `vision::analyze_image` (Phase 23's other new crate) needs real pixel
/// data too, not just dimensions, so it pulls in the `image` crate
/// properly rather than duplicating this trick.
fn read_png_dimensions(path: &Path) -> AppResult<(u32, u32)> {
    let bytes = std::fs::read(path)
        .map_err(|e| module_err(format!("reading back {}: {e}", path.display())))?;

    const SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < 24 || bytes[0..8] != SIGNATURE {
        return Err(module_err(format!(
            "{} doesn't look like a PNG (bad signature)",
            path.display()
        )));
    }
    if &bytes[12..16] != b"IHDR" {
        return Err(module_err(format!(
            "{} doesn't look like a PNG (first chunk isn't IHDR)",
            path.display()
        )));
    }

    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    Ok((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_is_preferred_when_both_are_set() {
        assert_eq!(
            detect_session_from(Some("wayland-0"), Some(":0")).unwrap(),
            SessionKind::Wayland
        );
    }

    #[test]
    fn x11_is_used_when_only_display_is_set() {
        assert_eq!(
            detect_session_from(None, Some(":0")).unwrap(),
            SessionKind::X11
        );
    }

    #[test]
    fn wayland_alone_is_still_wayland() {
        assert_eq!(
            detect_session_from(Some("wayland-0"), None).unwrap(),
            SessionKind::Wayland
        );
    }

    #[test]
    fn no_display_at_all_is_a_clean_error_not_a_panic() {
        let err = detect_session_from(None, None).unwrap_err();
        assert!(err.to_string().contains("no WAYLAND_DISPLAY or DISPLAY"));
    }

    /// A hand-built, minimal-but-valid one-pixel PNG: signature + an IHDR
    /// chunk with a known width/height, nothing else. Deliberately not
    /// using the `image` crate (or a captured screenshot) to build this
    /// fixture -- the whole point of this test is to prove
    /// `read_png_dimensions` reads the spec-mandated byte offsets
    /// directly, independent of any encoder.
    fn minimal_png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        bytes.extend_from_slice(&13u32.to_be_bytes()); // IHDR length
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]); // depth, color type, compression, filter, interlace
        bytes.extend_from_slice(&[0, 0, 0, 0]); // CRC (unchecked by our reader, spec-valid readers do check it)
        bytes
    }

    #[test]
    fn reads_real_width_and_height_from_a_valid_png() {
        let dir = std::env::temp_dir().join("screenshot-crate-test-dimensions");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fixture.png");
        std::fs::write(&path, minimal_png(1920, 1080)).unwrap();

        let (w, h) = read_png_dimensions(&path).unwrap();
        assert_eq!((w, h), (1920, 1080));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_a_file_that_isnt_a_png() {
        let dir = std::env::temp_dir().join("screenshot-crate-test-not-png");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("not-a-png.png");
        std::fs::write(&path, b"definitely not a png").unwrap();

        assert!(read_png_dimensions(&path).is_err());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn wayland_capture_fails_cleanly_when_grim_is_not_installed() {
        // This sandbox has no wlroots compositor and no `grim` binary --
        // real for *this* environment, the same honest category
        // `linux-services`' audio/notification tests already document
        // for their own missing services.
        let dir = std::env::temp_dir().join("screenshot-crate-test-wayland");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.png");

        let result = capture_wayland(&path);
        assert!(result.is_err());
        assert!(!path.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// The one test in this crate that needs a *real* display server.
    /// See this module's doc comment for the `Xvfb` setup this expects --
    /// without it, this fails outright (by design) rather than skipping.
    #[test]
    fn real_capture_against_a_live_x11_display_produces_a_correctly_sized_png() {
        let Ok(display) = std::env::var("DISPLAY") else {
            panic!(
                "DISPLAY is not set -- this test needs a real (or Xvfb) X11 server; \
                 see screenshot's crate-level doc comment for setup"
            );
        };
        assert!(!display.is_empty());

        let dir = std::env::temp_dir().join("screenshot-crate-test-real-capture");
        let info = capture_full_screen(&dir).expect("real capture against Xvfb should succeed");

        assert!(info.width > 0, "captured width should be nonzero");
        assert!(info.height > 0, "captured height should be nonzero");
        assert!(info.path.exists());

        std::fs::remove_dir_all(&dir).ok();
    }
}
