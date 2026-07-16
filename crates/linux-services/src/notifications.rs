//! Desktop notifications (TCLIS SS4 "Notification"; Apis SS9
//! Notification Service). Wraps `notify-send`, which talks to a
//! `org.freedesktop.Notifications` D-Bus service -- there's no session
//! bus or notification daemon running in this sandbox, so this fails
//! gracefully here and succeeds on a real desktop session, the same
//! honest split as [`crate::clipboard`].

use common::{AppError, AppResult};

pub fn notify(title: &str, body: &str) -> AppResult<()> {
    let output = std::process::Command::new("notify-send")
        .args([title, body])
        .output()
        .map_err(|e| AppError::Module {
            module: "linux-services::notifications",
            message: format!("notify-send is not available: {e}"),
        })?;

    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::Module {
            module: "linux-services::notifications",
            message: format!(
                "notify-send failed (no notification daemon? this sandbox has none): {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_fails_clearly_without_a_notification_daemon() {
        assert!(notify("test", "this should not succeed here").is_err());
    }
}
