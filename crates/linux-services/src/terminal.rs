//! Approved terminal command execution (TCLIS SS4 "Terminal": "No
//! unrestricted shell execution. High-risk commands require
//! confirmation."). Only commands in [`ALLOWED_COMMANDS`] can ever run,
//! and arguments are passed directly as an argv array to the child
//! process -- never through a shell -- so there is no pipe, redirect, or
//! `;`/`&&` injection surface at all: passing `"; rm -rf /"` as an
//! *argument* to `echo` just prints that literal string.
//!
//! Commands that take a path (`cat`, `rm`, ...) are deliberately left
//! out: file access goes through [`crate::files`], which does its own
//! existence and confirmation checks. This module is only for small,
//! informational, side-effect-free commands.

use common::{AppError, AppResult};
use std::time::{Duration, Instant};

/// The complete, fixed set of commands this module will ever run. `sleep`
/// is here purely so the timeout mechanism below has something to
/// genuinely test against -- it's otherwise inert.
pub const ALLOWED_COMMANDS: &[&str] = &[
    "date", "uptime", "whoami", "uname", "hostname", "echo", "sleep",
];

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub success: bool,
}

pub fn execute(command: &str, args: &[String], timeout: Duration) -> AppResult<CommandOutput> {
    if !ALLOWED_COMMANDS.contains(&command) {
        return Err(AppError::Module {
            module: "linux-services::terminal",
            message: format!(
                "{command:?} is not in the approved command allowlist {ALLOWED_COMMANDS:?}"
            ),
        });
    }

    let mut child = std::process::Command::new(command)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Module {
            module: "linux-services::terminal",
            message: format!("spawning {command}: {e}"),
        })?;

    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| AppError::Module {
            module: "linux-services::terminal",
            message: format!("waiting for {command}: {e}"),
        })? {
            break status;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::Module {
                module: "linux-services::terminal",
                message: format!("{command} did not complete within {timeout:?}"),
            });
        }
        std::thread::sleep(Duration::from_millis(5));
    };

    // The process has already exited, so its pipes are EOF'd -- these
    // reads return promptly with whatever was written, never block.
    use std::io::Read;
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_string(&mut stdout);
    }
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut stderr);
    }

    Ok(CommandOutput {
        stdout,
        stderr,
        exit_code: status.code(),
        success: status.success(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_an_allowed_command() {
        let output = execute("date", &[], Duration::from_secs(5)).unwrap();
        assert!(output.success);
        assert!(!output.stdout.trim().is_empty());
    }

    #[test]
    fn arguments_pass_through_literally() {
        let output = execute("echo", &["hello".to_string()], Duration::from_secs(5)).unwrap();
        assert_eq!(output.stdout.trim(), "hello");
    }

    #[test]
    fn disallowed_commands_are_rejected_before_ever_spawning() {
        // The key safety property of this module: even something as
        // dangerous as this never runs, because it's not on the list.
        let result = execute(
            "rm",
            &["-rf".to_string(), "/".to_string()],
            Duration::from_secs(5),
        );
        assert!(result.is_err());
    }

    #[test]
    fn shell_metacharacters_in_an_argument_are_inert() {
        // No shell ever interprets this -- it's just literal argv text.
        let output = execute(
            "echo",
            &["hi; rm -rf /".to_string()],
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(output.stdout.trim(), "hi; rm -rf /");
    }

    #[test]
    fn a_command_that_runs_too_long_is_genuinely_timed_out() {
        let start = Instant::now();
        let result = execute("sleep", &["2".to_string()], Duration::from_millis(100));
        assert!(result.is_err());
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "should return promptly, not wait out the full sleep"
        );
    }
}
