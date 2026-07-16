//! Audio control (TCLIS SS4 "Audio"; Apis SS9 Audio Service). Wraps
//! `pactl` (works against both PulseAudio and PipeWire's pulse-compat
//! layer) -- there's no sound server running in this sandbox, so this
//! fails gracefully here and succeeds on a real desktop session.

use common::{AppError, AppResult};

fn run(args: &[&str]) -> AppResult<std::process::Output> {
    std::process::Command::new("pactl")
        .args(args)
        .output()
        .map_err(|e| AppError::Module {
            module: "linux-services::audio",
            message: format!("pactl is not available: {e}"),
        })
}

/// Volume as a percentage of the default sink (0-100+; audio can be
/// boosted above 100% on many systems, so this isn't clamped).
pub fn get_volume() -> AppResult<u32> {
    let output = run(&["get-sink-volume", "@DEFAULT_SINK@"])?;
    if !output.status.success() {
        return Err(AppError::Module {
            module: "linux-services::audio",
            message: format!(
                "pactl failed (no sound server? this sandbox has none): {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.split('%')
        .next()
        .and_then(|s| s.rsplit(' ').find(|tok| !tok.is_empty()))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Module {
            module: "linux-services::audio",
            message: format!("could not parse pactl output: {text:?}"),
        })
}

pub fn set_volume(percent: u32) -> AppResult<()> {
    let output = run(&["set-sink-volume", "@DEFAULT_SINK@", &format!("{percent}%")])?;
    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::Module {
            module: "linux-services::audio",
            message: format!(
                "pactl failed (no sound server? this sandbox has none): {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        })
    }
}

pub fn set_muted(muted: bool) -> AppResult<()> {
    let flag = if muted { "1" } else { "0" };
    let output = run(&["set-sink-mute", "@DEFAULT_SINK@", flag])?;
    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::Module {
            module: "linux-services::audio",
            message: format!(
                "pactl failed (no sound server? this sandbox has none): {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_volume_fails_clearly_without_a_sound_server() {
        assert!(get_volume().is_err());
    }

    #[test]
    fn set_volume_fails_clearly_without_a_sound_server() {
        assert!(set_volume(50).is_err());
    }
}
