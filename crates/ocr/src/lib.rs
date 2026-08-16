//! Text extraction from images (Development_plan Phase 23 "OCR"; TCLIS
//! SS15 names OCR explicitly as planned future expansion). Wraps the
//! `tesseract` CLI the same way `linux-services::audio` wraps `pactl` or
//! `linux-services::notifications` wraps `notify-send`: one specific
//! external binary, fixed argv (no shell, no string interpolation into a
//! shell command), output captured and returned as structured data.
//!
//! Unlike screen capture, this needs nothing session-specific -- no
//! display server, no desktop environment. Installing the binary is
//! enough (`apt-get install tesseract-ocr tesseract-ocr-eng`), so this
//! crate's success path is fully, unconditionally verified for real: a
//! synthetic text image generated in the test itself (see
//! `tests::a_real_generated_text_image_is_read_back_correctly`) is handed
//! to a real `tesseract` binary and the real recognized text is asserted
//! against, not mocked.
//!
//! No internal timeout wrapping (unlike `terminal::execute`'s allowlist
//! runner, which manages its child process's lifecycle directly because
//! it's the general-purpose arbitrary-command executor). This follows
//! `audio`/`display`'s simpler pattern instead -- a single `.output()`
//! call -- and leans on `Tool::timeout()` plus `ToolRouter::execute`'s
//! own `spawn_blocking` + `tokio::time::timeout` race for the
//! router-level guarantee every other wrapped tool in this workspace
//! already relies on.

use common::{AppError, AppResult};
use std::path::Path;
use std::process::Command;

pub mod tools;

fn module_err(context: impl std::fmt::Display) -> AppError {
    AppError::Module {
        module: "ocr",
        message: context.to_string(),
    }
}

pub const DEFAULT_LANGUAGE: &str = "eng";

/// Tesseract language codes are short, `+`-joined, lowercase-letter
/// tokens (`"eng"`, `"eng+fra"`). Validating the shape before it reaches
/// `Command::arg` isn't about shell injection -- `std::process::Command`
/// never invokes a shell, so there's no metacharacter-escaping problem to
/// begin with -- it's TCLIS SS13's general "tool inputs are sanitized
/// and validated" requirement: a clearly-malformed value should fail
/// fast with a specific message rather than being handed to `tesseract`
/// to reject on its own terms.
fn validate_language(language: &str) -> AppResult<()> {
    let plausible = !language.is_empty()
        && language
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '+' || c == '_');
    if plausible {
        Ok(())
    } else {
        Err(module_err(format!(
            "{language:?} doesn't look like a tesseract language code (expected lowercase \
             letters, optionally `+`-joined, e.g. \"eng\" or \"eng+fra\")"
        )))
    }
}

/// Runs `tesseract <path> stdout -l <language>`. `stdout` as the output
/// base tells tesseract to write recognized text to its own standard
/// output rather than `<base>.txt`, so nothing touches disk beyond the
/// input image already sitting there.
///
/// An image with no recognizable text is a legitimate `Ok("")`, not an
/// error -- TCLIS SS10's error categories are about the *tool* failing
/// (missing binary, bad input, timeout), not about the answer being
/// uninteresting.
pub fn extract_text(image_path: &Path, language: &str) -> AppResult<String> {
    validate_language(language)?;

    if !image_path.exists() {
        return Err(module_err(format!(
            "{} does not exist",
            image_path.display()
        )));
    }

    let output = Command::new("tesseract")
        .arg(image_path)
        .arg("stdout")
        .args(["-l", language])
        .output()
        .map_err(|e| {
            module_err(format!(
                "failed to launch `tesseract` (is tesseract-ocr installed? {e})"
            ))
        })?;

    if !output.status.success() {
        return Err(module_err(format!(
            "tesseract exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    // Real tesseract output already ends with its own trailing newline;
    // trimming gives callers a clean string rather than making every
    // caller repeat the same `.trim()`.
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plausible_language_codes() {
        assert!(validate_language("eng").is_ok());
        assert!(validate_language("eng+fra").is_ok());
        assert!(validate_language("chi_sim").is_ok());
    }

    #[test]
    fn rejects_empty_or_implausible_language_codes() {
        assert!(validate_language("").is_err());
        assert!(validate_language("ENG").is_err());
        assert!(validate_language("eng;rm -rf").is_err());
        assert!(validate_language("--outputbase=x").is_err());
    }

    #[test]
    fn missing_file_fails_cleanly_before_ever_invoking_tesseract() {
        let result = extract_text(Path::new("/nonexistent/path/to/nothing.png"), "eng");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    /// Generates a real PNG containing real, known text (via
    /// ImageMagick's `convert -annotate`, a test-fixture-only dependency
    /// -- see this crate's README note, mirroring `voice`'s existing
    /// precedent of documenting test-only external tool prerequisites
    /// separately from runtime ones) and reads it back with the real
    /// `tesseract` binary this crate actually wraps. Confirmed by hand
    /// during development against exactly this pipeline before being
    /// written down as a test. Both `convert` and `tesseract` are hard
    /// requirements here, matching `voice`'s established precedent of
    /// failing loudly rather than silently skipping when a documented
    /// prerequisite is missing -- a green run of this test is only
    /// meaningful if it actually proved something.
    #[test]
    fn a_real_generated_text_image_is_read_back_correctly() {
        let dir = std::env::temp_dir().join("ocr-crate-test-fixture");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fixture.png");

        let convert = Command::new("convert")
            .args(["-size", "400x100", "xc:white"])
            .args(["-font", "DejaVu-Sans-Bold", "-pointsize", "28"])
            .args(["-fill", "black", "-gravity", "center"])
            .args(["-annotate", "+0+0", "Phase 23 Vision"])
            .arg(&path)
            .output()
            .expect("`convert` (imagemagick) must be installed to build this test's fixture");
        assert!(
            convert.status.success(),
            "`convert` failed to build the fixture: {}",
            String::from_utf8_lossy(&convert.stderr)
        );

        let text = extract_text(&path, DEFAULT_LANGUAGE)
            .expect("tesseract-ocr must be installed for this test to verify anything real");
        assert!(
            text.to_lowercase().contains("vision"),
            "expected recognizable text, got {text:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
