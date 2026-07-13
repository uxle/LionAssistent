//! Display control (TCLIS SS4 "Display"; Apis SS9 Display Service).
//! Brightness reads/writes `/sys/class/backlight/*/brightness` -- a
//! kernel sysfs interface, not dependent on X11/Wayland at all. It fails
//! gracefully here for a different, more fundamental reason than
//! [`crate::clipboard`]/[`crate::audio`]/[`crate::notifications`]: this
//! container simply has no backlight device, because it has no physical
//! display. A real laptop has a `/sys/class/backlight` entry and this
//! same code works without any display session running.

use common::{AppError, AppResult};
use std::path::PathBuf;

fn backlight_device() -> AppResult<PathBuf> {
    std::fs::read_dir("/sys/class/backlight")
        .ok()
        .and_then(|mut rd| rd.next())
        .and_then(|e| e.ok())
        .map(|e| e.path())
        .ok_or_else(|| AppError::Module {
            module: "linux-services::display",
            message: "no backlight device found (no physical display attached)".to_string(),
        })
}

/// Brightness as a percentage of the device's maximum.
pub fn get_brightness() -> AppResult<u32> {
    let device = backlight_device()?;
    let read = |name: &str| -> AppResult<u64> {
        std::fs::read_to_string(device.join(name))
            .map_err(|e| AppError::Module {
                module: "linux-services::display",
                message: format!("reading {}: {e}", device.join(name).display()),
            })?
            .trim()
            .parse()
            .map_err(|e| AppError::Module {
                module: "linux-services::display",
                message: format!("parsing {name}: {e}"),
            })
    };
    let current = read("brightness")?;
    let max = read("max_brightness")?;
    if max == 0 {
        return Ok(0);
    }
    Ok(((current * 100) / max) as u32)
}

pub fn set_brightness(percent: u32) -> AppResult<()> {
    let device = backlight_device()?;
    let max: u64 = std::fs::read_to_string(device.join("max_brightness"))
        .map_err(|e| AppError::Module {
            module: "linux-services::display",
            message: e.to_string(),
        })?
        .trim()
        .parse()
        .map_err(|e: std::num::ParseIntError| AppError::Module {
            module: "linux-services::display",
            message: e.to_string(),
        })?;
    let target = (max * percent.min(100) as u64) / 100;
    std::fs::write(device.join("brightness"), target.to_string()).map_err(|e| AppError::Module {
        module: "linux-services::display",
        message: format!(
            "writing brightness (needs permission to {}): {e}",
            device.display()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_fails_clearly_with_no_backlight_device() {
        // Honest for this container: no physical display, no backlight
        // sysfs entry. A laptop running this same code would succeed.
        assert!(get_brightness().is_err());
    }

    #[test]
    fn set_brightness_fails_clearly_with_no_backlight_device() {
        assert!(set_brightness(50).is_err());
    }
}
