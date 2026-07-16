//! Process management (TCLIS SS4 "Process Management"; Apis SS9 Process
//! Service). `kill_process` shells out to the standard `kill` utility
//! rather than binding the raw syscall, to avoid a `libc`/`nix`
//! dependency for one call. Priority adjustment (TCLIS SS4: "subject to
//! permissions") isn't implemented yet -- not needed until something
//! calls for it, matching this project's incremental approach throughout.

use crate::io_err;
use common::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub state: String,
    pub memory_kb: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// Ask nicely: the process can catch this and shut down cleanly.
    Terminate,
    /// Cannot be caught or ignored -- immediate, ungraceful termination.
    Kill,
    Interrupt,
}

impl Signal {
    fn as_str(self) -> &'static str {
        match self {
            Signal::Terminate => "TERM",
            Signal::Kill => "KILL",
            Signal::Interrupt => "INT",
        }
    }
}

fn read_process_info(pid: u32) -> AppResult<ProcessInfo> {
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .map_err(|e| io_err("linux-services::processes", format!("/proc/{pid}/comm"), e))?
        .trim()
        .to_string();

    let status_raw = std::fs::read_to_string(format!("/proc/{pid}/status")).map_err(|e| {
        io_err(
            "linux-services::processes",
            format!("/proc/{pid}/status"),
            e,
        )
    })?;

    let state = status_raw
        .lines()
        .find(|l| l.starts_with("State:"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let memory_kb = status_raw
        .lines()
        .find(|l| l.starts_with("VmRSS:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|n| n.parse().ok());

    Ok(ProcessInfo {
        pid,
        name: comm,
        state,
        memory_kb,
    })
}

pub fn process_info(pid: u32) -> AppResult<ProcessInfo> {
    read_process_info(pid)
}

/// Lists every process this user can see. Entries that disappear or
/// become unreadable between listing `/proc` and reading their details
/// (normal races -- a process can exit at any moment) are silently
/// skipped rather than failing the whole listing.
pub fn list_processes() -> AppResult<Vec<ProcessInfo>> {
    let read_dir =
        std::fs::read_dir("/proc").map_err(|e| io_err("linux-services::processes", "/proc", e))?;

    let mut processes = Vec::new();
    for entry in read_dir.flatten() {
        let Some(pid_str) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        if let Ok(info) = read_process_info(pid) {
            processes.push(info);
        }
    }
    processes.sort_by_key(|p| p.pid);
    Ok(processes)
}

/// Sends a signal to `pid` via the standard `kill` utility -- Level 3
/// ("Destructive"; TCLIS SS6) in whatever tool wraps this, since it
/// affects a process outside the assistant's own control.
pub fn kill_process(pid: u32, signal: Signal) -> AppResult<()> {
    let status = std::process::Command::new("kill")
        .args(["-s", signal.as_str(), &pid.to_string()])
        .status()
        .map_err(|e| io_err("linux-services::processes", "running kill", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(AppError::Module {
            module: "linux-services::processes",
            message: format!("kill -{} {pid} failed: {status}", signal.as_str()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn lists_at_least_the_current_process() {
        let processes = list_processes().unwrap();
        assert!(!processes.is_empty());
    }

    #[test]
    fn info_for_the_current_process_is_readable() {
        let info = process_info(std::process::id()).unwrap();
        assert_eq!(info.pid, std::process::id());
        assert!(!info.state.is_empty());
    }

    #[test]
    fn info_for_a_nonexistent_pid_is_an_error() {
        // PID 1 always exists (init); a huge, almost-certainly-unused PID
        // number is a reliable stand-in for "doesn't exist".
        assert!(process_info(u32::MAX - 1).is_err());
    }

    #[test]
    fn kill_terminates_a_process_this_test_spawned() {
        // Only ever kill something we deliberately created for this
        // purpose -- never anything else running in the environment.
        let mut child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("failed to spawn test child");
        let pid = child.id();

        kill_process(pid, Signal::Kill).unwrap();

        let status = child.wait().unwrap();
        assert!(
            !status.success(),
            "child should have been killed, not exited cleanly"
        );
    }
}
