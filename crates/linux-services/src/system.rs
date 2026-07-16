//! System information (TCLIS SS4 "System"; Apis SS9 relevant methods).
//! Reads `/proc` directly rather than depending on a system-info crate --
//! `/proc` is a kernel interface present in every Linux environment,
//! containers included, so this needs no new dependency and genuinely
//! works here, not just on a full desktop.

use crate::io_err;
use common::{AppError, AppResult};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub cpu_cores: usize,
    pub cpu_model: Option<String>,
    pub memory_total_kb: u64,
    pub memory_available_kb: u64,
    pub uptime_seconds: f64,
    pub kernel_version: String,
    pub load_average_1m: f64,
}

#[derive(Debug, Clone)]
pub struct DiskUsage {
    pub total_kb: u64,
    pub used_kb: u64,
    pub available_kb: u64,
    pub mounted_on: String,
}

fn read_proc(path: &str) -> AppResult<String> {
    std::fs::read_to_string(path).map_err(|e| io_err("linux-services::system", path, e))
}

pub fn cpu_info() -> AppResult<(usize, Option<String>)> {
    let raw = read_proc("/proc/cpuinfo")?;
    let cores = raw.lines().filter(|l| l.starts_with("processor")).count();
    let model = raw
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string());
    Ok((cores, model))
}

pub fn memory_info() -> AppResult<(u64, u64)> {
    let raw = read_proc("/proc/meminfo")?;
    let field = |name: &str| -> AppResult<u64> {
        raw.lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| AppError::Module {
                module: "linux-services::system",
                message: format!("could not find {name} in /proc/meminfo"),
            })
    };
    Ok((field("MemTotal:")?, field("MemAvailable:")?))
}

pub fn uptime_seconds() -> AppResult<f64> {
    let raw = read_proc("/proc/uptime")?;
    raw.split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Module {
            module: "linux-services::system",
            message: "could not parse /proc/uptime".to_string(),
        })
}

pub fn kernel_version() -> AppResult<String> {
    Ok(read_proc("/proc/sys/kernel/osrelease")?.trim().to_string())
}

pub fn load_average_1m() -> AppResult<f64> {
    let raw = read_proc("/proc/loadavg")?;
    raw.split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Module {
            module: "linux-services::system",
            message: "could not parse /proc/loadavg".to_string(),
        })
}

/// Uses `df` rather than a raw `statvfs` syscall binding, to avoid
/// pulling in a libc/nix dependency for one call -- `df` is present on
/// every Linux system this assistant targets.
pub fn disk_usage(path: &Path) -> AppResult<DiskUsage> {
    let output = std::process::Command::new("df")
        .args(["-Pk", &path.to_string_lossy()])
        .output()
        .map_err(|e| io_err("linux-services::system", "running df", e))?;

    if !output.status.success() {
        return Err(AppError::Module {
            module: "linux-services::system",
            message: format!(
                "df exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let data_line = text.lines().nth(1).ok_or_else(|| AppError::Module {
        module: "linux-services::system",
        message: format!("unexpected df output for {}", path.display()),
    })?;

    let cols: Vec<&str> = data_line.split_whitespace().collect();
    if cols.len() < 6 {
        return Err(AppError::Module {
            module: "linux-services::system",
            message: format!("unexpected df output: {data_line:?}"),
        });
    }

    Ok(DiskUsage {
        total_kb: cols[1].parse().unwrap_or(0),
        used_kb: cols[2].parse().unwrap_or(0),
        available_kb: cols[3].parse().unwrap_or(0),
        mounted_on: cols[5].to_string(),
    })
}

pub fn system_info() -> AppResult<SystemInfo> {
    let (cpu_cores, cpu_model) = cpu_info()?;
    let (memory_total_kb, memory_available_kb) = memory_info()?;
    Ok(SystemInfo {
        cpu_cores,
        cpu_model,
        memory_total_kb,
        memory_available_kb,
        uptime_seconds: uptime_seconds()?,
        kernel_version: kernel_version()?,
        load_average_1m: load_average_1m()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_info_reports_at_least_one_core() {
        let (cores, _model) = cpu_info().unwrap();
        assert!(cores >= 1);
    }

    #[test]
    fn memory_info_reports_plausible_values() {
        let (total, available) = memory_info().unwrap();
        assert!(total > 0);
        assert!(available <= total);
    }

    #[test]
    fn uptime_is_positive() {
        assert!(uptime_seconds().unwrap() > 0.0);
    }

    #[test]
    fn kernel_version_is_nonempty() {
        assert!(!kernel_version().unwrap().is_empty());
    }

    #[test]
    fn load_average_is_non_negative() {
        assert!(load_average_1m().unwrap() >= 0.0);
    }

    #[test]
    fn disk_usage_reports_plausible_values_for_root() {
        let usage = disk_usage(Path::new("/")).unwrap();
        assert!(usage.total_kb > 0);
        assert!(usage.used_kb <= usage.total_kb);
    }

    #[test]
    fn system_info_combines_everything() {
        let info = system_info().unwrap();
        assert!(info.cpu_cores >= 1);
        assert!(info.memory_total_kb > 0);
        assert!(!info.kernel_version.is_empty());
    }
}
