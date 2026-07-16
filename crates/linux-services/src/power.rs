//! Power actions (TCLIS SS4 "Power"; Apis SS9-adjacent). Lock is Level 1
//! (safe); suspend/hibernate/restart/shutdown are Level 4
//! ("Administrator" -- TCLIS SS6) in whatever tool wraps these, since
//! they affect the whole machine, not just the assistant.
//!
//! **This module's own test suite never actually invokes any of these,
//! not even `lock`.** Automatically restarting or shutting down whatever
//! machine happens to be running `cargo test` -- including, in this
//! project's case, the sandbox this was built in -- is not a defensible
//! thing for a test to do. Tests here only confirm the right command
//! gets *constructed*; running it for real is exactly the kind of
//! action Phase 9's Permission Manager should gate behind an explicit,
//! interactive confirmation, not something a test harness should trigger
//! on its own.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Lock,
    Suspend,
    Hibernate,
    Restart,
    Shutdown,
}

impl PowerAction {
    /// The `systemctl`/`loginctl` invocation this action maps to. Exposed
    /// so callers (and tests) can inspect exactly what would run without
    /// running it.
    pub fn command(&self) -> (&'static str, &'static [&'static str]) {
        match self {
            PowerAction::Lock => ("loginctl", &["lock-session"]),
            PowerAction::Suspend => ("systemctl", &["suspend"]),
            PowerAction::Hibernate => ("systemctl", &["hibernate"]),
            PowerAction::Restart => ("systemctl", &["reboot"]),
            PowerAction::Shutdown => ("systemctl", &["poweroff"]),
        }
    }

    pub fn permission_level(&self) -> common::PermissionLevel {
        match self {
            PowerAction::Lock => common::PermissionLevel::Safe,
            _ => common::PermissionLevel::Administrator,
        }
    }
}

/// Actually runs the action's command. Deliberately given a name that
/// can't be called by accident from a test or from anywhere in this
/// crate without very clearly meaning to.
///
/// # Safety note
/// This is real: calling it on a running system really does lock,
/// suspend, hibernate, restart, or shut down. Callers are expected to
/// have already gone through the real Permission Manager (Phase 9) for
/// anything above `Lock` before ever reaching this function.
pub fn execute_power_action_for_real(action: PowerAction) -> common::AppResult<()> {
    let (program, args) = action.command();
    let status = std::process::Command::new(program)
        .args(args)
        .status()
        .map_err(|e| common::AppError::Module {
            module: "linux-services::power",
            message: format!("running {program}: {e}"),
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(common::AppError::Module {
            module: "linux-services::power",
            message: format!("{program} {args:?} exited with {status}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These check command *construction* only. None of them call
    // `execute_power_action_for_real` -- see the module-level doc
    // comment for why that's a deliberate choice, not an oversight.

    #[test]
    fn lock_maps_to_loginctl_lock_session() {
        assert_eq!(
            PowerAction::Lock.command(),
            ("loginctl", &["lock-session"][..])
        );
    }

    #[test]
    fn shutdown_maps_to_systemctl_poweroff() {
        assert_eq!(
            PowerAction::Shutdown.command(),
            ("systemctl", &["poweroff"][..])
        );
    }

    #[test]
    fn only_lock_is_below_administrator_permission() {
        assert_eq!(
            PowerAction::Lock.permission_level(),
            common::PermissionLevel::Safe
        );
        for action in [
            PowerAction::Suspend,
            PowerAction::Hibernate,
            PowerAction::Restart,
            PowerAction::Shutdown,
        ] {
            assert_eq!(
                action.permission_level(),
                common::PermissionLevel::Administrator
            );
        }
    }
}
