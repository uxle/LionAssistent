//! "Sandbox" (Development_plan Phase 16), part two: filesystem access
//! restriction via Landlock, an unprivileged Linux security module
//! (kernel 5.13+, no special capabilities needed -- unlike namespaces or
//! seccomp, which typically need either privilege or a container
//! runtime's cooperation).
//!
//! **Read this before trusting a call to [`FilesystemSandbox::restrict_current_process`]:**
//! Landlock is a defense-in-depth mechanism that the kernel is allowed
//! to silently *not* enforce -- on an old kernel, one built without the
//! Landlock config option, or one running inside another layer of
//! sandboxing that blocks the relevant syscalls. The `landlock` crate
//! reflects this faithfully: `restrict_self()` returns a status, not
//! just success or failure, and this module forces every caller to look
//! at it via [`EnforcementStatus`] rather than assuming `Ok` means
//! "protected."
//!
//! That distinction isn't hypothetical here. This exact code was tested
//! against a real read from an explicitly-not-allowed directory in the
//! sandbox this workspace was built in: it compiled, ran, returned
//! `Ok`, and reported [`EnforcementStatus::NotEnforced`] -- and the
//! blocked read genuinely succeeded anyway, exactly as that status says
//! it would. The kernel here (6.18, far newer than Landlock's 5.13
//! floor) supports it in principle; something about running inside
//! Claude's own sandboxing evidently blocks the underlying syscalls in
//! practice. On an unrestricted modern Linux desktop -- the actual
//! target for this application -- real enforcement is expected. See
//! `examples/filesystem_sandbox_demo.rs` for a standalone, manually-run
//! demonstration (not part of the automated suite; see that file for
//! why: this call is process-wide and irreversible, unsafe to invoke
//! from a shared test binary).

use common::{AppError, AppResult};
use landlock::{
    Access, AccessFs, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus,
    ABI,
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnforcementStatus {
    /// The kernel confirmed it will actually enforce these rules.
    Enforced,
    /// The restriction call succeeded but the kernel will *not* enforce
    /// it -- callers must not treat this as "protected." Log it, and
    /// decide per call site whether to proceed anyway (degraded, but
    /// still functional) or refuse.
    NotEnforced,
}

impl From<RulesetStatus> for EnforcementStatus {
    fn from(status: RulesetStatus) -> Self {
        match status {
            RulesetStatus::FullyEnforced => EnforcementStatus::Enforced,
            RulesetStatus::PartiallyEnforced | RulesetStatus::NotEnforced => {
                EnforcementStatus::NotEnforced
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FilesystemSandbox {
    read_paths: Vec<PathBuf>,
    write_paths: Vec<PathBuf>,
}

impl FilesystemSandbox {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allow_read(mut self, path: impl Into<PathBuf>) -> Self {
        self.read_paths.push(path.into());
        self
    }

    pub fn allow_write(mut self, path: impl Into<PathBuf>) -> Self {
        self.write_paths.push(path.into());
        self
    }

    /// Restricts the *current* process -- and everything it `exec`s
    /// afterward -- to only the paths configured via
    /// [`FilesystemSandbox::allow_read`]/[`FilesystemSandbox::allow_write`],
    /// for the rest of this process's lifetime. Landlock rules are
    /// additive and irrevocable: there is no "undo" once called, which
    /// is exactly why this is meant for a process (or a worker thread
    /// about to load genuinely untrusted code, e.g. a plugin `.so`) to
    /// apply to *itself* early, not something to toggle on and off.
    ///
    /// **Always check the returned status** -- see this module's doc
    /// comment.
    pub fn restrict_current_process(self) -> AppResult<EnforcementStatus> {
        let abi = ABI::V1;
        let access = AccessFs::from_all(abi);

        let mut ruleset = Ruleset::default()
            .handle_access(access)
            .map_err(landlock_err)?
            .create()
            .map_err(landlock_err)?;

        for path in &self.read_paths {
            ruleset = ruleset
                .add_rule(PathBeneath::new(
                    PathFd::new(path).map_err(landlock_err)?,
                    AccessFs::from_read(abi),
                ))
                .map_err(landlock_err)?;
        }
        for path in &self.write_paths {
            ruleset = ruleset
                .add_rule(PathBeneath::new(
                    PathFd::new(path).map_err(landlock_err)?,
                    access, // read+write for explicitly writable paths
                ))
                .map_err(landlock_err)?;
        }

        let status = ruleset.restrict_self().map_err(landlock_err)?;
        Ok(status.ruleset.into())
    }
}

fn landlock_err(e: impl std::fmt::Display) -> AppError {
    AppError::Module {
        module: "sandbox",
        message: format!("landlock: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_accumulates_multiple_paths() {
        let sandbox = FilesystemSandbox::new()
            .allow_read("/etc")
            .allow_read("/usr/share")
            .allow_write("/tmp/scratch");
        assert_eq!(sandbox.read_paths.len(), 2);
        assert_eq!(sandbox.write_paths.len(), 1);
    }

    #[test]
    fn new_sandbox_has_no_paths() {
        let sandbox = FilesystemSandbox::new();
        assert!(sandbox.read_paths.is_empty());
        assert!(sandbox.write_paths.is_empty());
    }

    #[test]
    fn enforcement_status_conversion_treats_partial_as_not_enforced() {
        // A partially-enforced ruleset is not a safety guarantee --
        // this workspace's rule is "fully enforced or treat it as
        // unenforced," never "enforced enough."
        assert_eq!(
            EnforcementStatus::from(RulesetStatus::PartiallyEnforced),
            EnforcementStatus::NotEnforced
        );
        assert_eq!(
            EnforcementStatus::from(RulesetStatus::FullyEnforced),
            EnforcementStatus::Enforced
        );
        assert_eq!(
            EnforcementStatus::from(RulesetStatus::NotEnforced),
            EnforcementStatus::NotEnforced
        );
    }

    // Deliberately no test here calls `restrict_current_process` --
    // see this module's doc comment and `examples/filesystem_sandbox_demo.rs`.
}
