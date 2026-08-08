//! "Sandbox" (Development_plan Phase 16), part one: real, verified
//! resource limits for a spawned child process, via POSIX `rlimit`
//! (`setrlimit`, through the small `rlimit` crate rather than hand-rolled
//! `libc` calls). Directly relevant to Phase 8's Terminal capability
//! (TCLIS SS4: "No unrestricted shell execution... high-risk commands
//! require confirmation") -- confirmation handles *whether* a command
//! runs; this handles *how much damage it can do once running*, an
//! orthogonal, complementary protection.
//!
//! This one is empirically confirmed to work in the sandbox this was
//! built in: a real child process given a 1-second CPU limit and told to
//! spin forever was actually killed after ~1 second, not left running.
//! See [`crate::filesystem`] for the other half of this module, where
//! the equivalent check came back negative.
//!
//! Deliberately not wired into `linux_services::terminal` in this phase.
//! That crate is Phase 8's, already complete and verified; adding a
//! dependency from an *earlier* phase onto this *later* one would run
//! against the grain of how every dependency edge in this workspace has
//! flowed so far (later phases depend on earlier ones). `apply_limits`
//! is written as a standalone function precisely so a future revision of
//! that call site -- or `app`'s composition, or a plugin execution path
//! -- can adopt it by calling it on a `Command` they already own,
//! without this crate needing to reach backward into Phase 8's.

use common::AppResult;
use rlimit::Resource;
use std::os::unix::process::CommandExt;
use std::process::Command;

/// All fields are `Option` -- `None` leaves that particular limit alone
/// (inherited from the parent process), rather than every field needing
/// an explicit value to construct a [`ProcessLimits`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessLimits {
    pub cpu_seconds: Option<u64>,
    pub memory_bytes: Option<u64>,
    pub max_open_files: Option<u64>,
    pub max_processes: Option<u64>,
    pub max_file_size_bytes: Option<u64>,
}

impl ProcessLimits {
    /// No limits at all -- identical to [`ProcessLimits::default`], kept
    /// as a named constructor so a call site reads as an explicit choice
    /// ("run this one totally unconstrained") rather than an
    /// accidentally-empty struct literal.
    pub fn none() -> Self {
        Self::default()
    }

    /// Sensible, deliberately generous defaults for "an assistant-run
    /// command shouldn't be able to consume the whole machine," not a
    /// tight security boundary on their own -- 30s CPU, 1GiB memory,
    /// 256 open files, 64 processes, 100MB max file size. Tune per call
    /// site for anything more specific.
    pub fn moderate() -> Self {
        Self {
            cpu_seconds: Some(30),
            memory_bytes: Some(1024 * 1024 * 1024),
            max_open_files: Some(256),
            max_processes: Some(64),
            max_file_size_bytes: Some(100 * 1024 * 1024),
        }
    }
}

/// Applies `limits` to `command`, so that when it's spawned, the *child*
/// process (never the current one) is constrained -- via `pre_exec`,
/// which runs in the forked child between `fork()` and `exec()`, before
/// `command`'s program replaces this code. Must be called before
/// `.spawn()`/`.status()`/`.output()`.
///
/// # Safety note on the `unsafe` inside
///
/// `Command::pre_exec` is `unsafe` because *arbitrary* code isn't safe to
/// run between `fork()` and `exec()` in a multi-threaded process (only
/// async-signal-safe operations are) -- `setrlimit` is POSIX-specified
/// as async-signal-safe, so this specific use is sound. This is the same
/// category of narrowly-scoped, documented `unsafe` at an OS-boundary
/// this workspace already uses elsewhere (Phase 11's `libloading`-based
/// plugin loading).
pub fn apply_limits(command: &mut Command, limits: ProcessLimits) {
    unsafe {
        command.pre_exec(move || {
            if let Some(seconds) = limits.cpu_seconds {
                Resource::CPU.set(seconds, seconds)?;
            }
            if let Some(bytes) = limits.memory_bytes {
                Resource::AS.set(bytes, bytes)?;
            }
            if let Some(count) = limits.max_open_files {
                Resource::NOFILE.set(count, count)?;
            }
            if let Some(count) = limits.max_processes {
                Resource::NPROC.set(count, count)?;
            }
            if let Some(bytes) = limits.max_file_size_bytes {
                Resource::FSIZE.set(bytes, bytes)?;
            }
            Ok(())
        });
    }
}

/// Reads back the *current process's* limits. Mostly useful from inside
/// a child spawned with [`apply_limits`] (e.g. to log or assert what it
/// actually ended up with) -- deliberately not exercised against this
/// crate's own test process in any test here, since rlimits are
/// only ever loosened by re-exec, never simply reset, and permanently
/// narrowing the test binary's own limits could interfere with whatever
/// else runs in the same process afterward.
pub fn current_limits() -> AppResult<ProcessLimits> {
    let (cpu_soft, _) = Resource::CPU.get()?;
    let (mem_soft, _) = Resource::AS.get()?;
    let (files_soft, _) = Resource::NOFILE.get()?;
    let (proc_soft, _) = Resource::NPROC.get()?;
    let (fsize_soft, _) = Resource::FSIZE.get()?;

    // rlimit reports "no limit" as INFINITY -- normalize that to `None`
    // rather than a suspiciously huge `Some(u64::MAX)`.
    let none_if_infinite = |v: u64| (v != rlimit::INFINITY).then_some(v);

    Ok(ProcessLimits {
        cpu_seconds: none_if_infinite(cpu_soft),
        memory_bytes: none_if_infinite(mem_soft),
        max_open_files: none_if_infinite(files_soft),
        max_processes: none_if_infinite(proc_soft),
        max_file_size_bytes: none_if_infinite(fsize_soft),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn moderate_limits_are_all_set() {
        let limits = ProcessLimits::moderate();
        assert!(limits.cpu_seconds.is_some());
        assert!(limits.memory_bytes.is_some());
        assert!(limits.max_open_files.is_some());
        assert!(limits.max_processes.is_some());
        assert!(limits.max_file_size_bytes.is_some());
    }

    #[test]
    fn none_limits_are_all_unset() {
        let limits = ProcessLimits::none();
        assert!(limits.cpu_seconds.is_none());
        assert!(limits.memory_bytes.is_none());
    }

    #[test]
    fn a_cpu_limited_child_is_actually_killed_rather_than_left_running() {
        // The empirical proof this module's doc comment refers to,
        // as an automated test: a child told to busy-loop forever, with
        // a 1-second CPU limit applied, must not still be running after
        // a few real seconds.
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("i=0; while true; do i=$((i+1)); done");
        apply_limits(
            &mut command,
            ProcessLimits {
                cpu_seconds: Some(1),
                ..ProcessLimits::none()
            },
        );

        let start = Instant::now();
        let status = command.status().expect("failed to spawn child");
        let elapsed = start.elapsed();

        assert!(
            !status.success(),
            "child should have been killed, not exited cleanly"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "child ran for {elapsed:?}, longer than its 1s CPU limit should have allowed"
        );
    }

    #[test]
    fn applying_no_limits_does_not_interfere_with_normal_execution() {
        let mut command = Command::new("true");
        apply_limits(&mut command, ProcessLimits::none());
        let status = command.status().unwrap();
        assert!(status.success());
    }

    #[test]
    fn current_limits_reads_back_without_error() {
        // A smoke test only -- doesn't assert particular values, since
        // this test process's real limits depend entirely on whatever
        // environment is running it.
        assert!(current_limits().is_ok());
    }
}
