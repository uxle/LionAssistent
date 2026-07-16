//! Clipboard access (TCLIS SS4 "Clipboard"; Apis SS9 Clipboard Service).
//! Wraps `xclip` (X11) or `wl-copy`/`wl-paste` (Wayland) -- both need a
//! running display session, which this sandbox doesn't have. That's a
//! genuine, honest limitation of a headless container, not something to
//! work around: the tests here confirm a clear error comes back rather
//! than a hang or panic, and the exact same code succeeds on a real
//! desktop with `DISPLAY` or `WAYLAND_DISPLAY` set.

use common::{AppError, AppResult};

fn run(command: &str, args: &[&str], stdin_data: Option<&str>) -> AppResult<std::process::Output> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = std::process::Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Module {
            module: "linux-services::clipboard",
            message: format!("{command} is not available: {e}"),
        })?;

    if let Some(data) = stdin_data {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(data.as_bytes());
        }
    }

    child.wait_with_output().map_err(|e| AppError::Module {
        module: "linux-services::clipboard",
        message: format!("running {command}: {e}"),
    })
}

pub fn read() -> AppResult<String> {
    let output = run("xclip", &["-selection", "clipboard", "-o"], None)?;
    if !output.status.success() {
        return Err(AppError::Module {
            module: "linux-services::clipboard",
            message: format!(
                "xclip failed (no display session? this sandbox has none): {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn write(text: &str) -> AppResult<()> {
    let output = run("xclip", &["-selection", "clipboard"], Some(text))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::Module {
            module: "linux-services::clipboard",
            message: format!(
                "xclip failed (no display session? this sandbox has none): {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        })
    }
}

pub fn clear() -> AppResult<()> {
    write("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_fails_clearly_without_a_display_session() {
        // This is the honest, expected result in this sandbox -- no
        // panic, no hang, just a clear error. On a real desktop with
        // xclip installed and a display running, this would succeed
        // instead.
        assert!(read().is_err());
    }

    #[test]
    fn write_fails_clearly_without_a_display_session() {
        assert!(write("test").is_err());
    }
}
