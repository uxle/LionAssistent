//! Package management (TCLIS SS4 "Package Management"; Apis SS9 Package
//! Service). Wraps `dpkg`/`apt` rather than reimplementing package
//! management -- TCLIS SS4 explicitly wants this abstracted so a
//! different package manager (`dnf`, `pacman`, ...) could stand in later
//! without changing callers.
//!
//! `list_installed` and `search` are read-only and safe to run in any
//! test suite. `install`/`remove` are real, but this crate's tests never
//! actually invoke them -- running `apt-get install` as a side effect of
//! `cargo test` would be slow, network-dependent, and could leave the
//! environment in a different state than it started in. That's not a
//! sandbox limitation; it's just not something a test suite should do.

use common::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
}

fn run(command: &str, args: &[&str]) -> AppResult<std::process::Output> {
    std::process::Command::new(command)
        .args(args)
        .output()
        .map_err(|e| AppError::Module {
            module: "linux-services::packages",
            message: format!("running {command}: {e}"),
        })
}

/// `dpkg -l` lists every installed package on Debian/Ubuntu systems.
pub fn list_installed() -> AppResult<Vec<PackageInfo>> {
    let output = run("dpkg-query", &["-W", "-f=${Package}\t${Version}\n"])?;
    if !output.status.success() {
        return Err(AppError::Module {
            module: "linux-services::packages",
            message: format!("dpkg-query exited with {}", output.status),
        });
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let packages = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            Some(PackageInfo {
                name: parts.next()?.to_string(),
                version: parts.next()?.to_string(),
            })
        })
        .collect();
    Ok(packages)
}

/// Searches the local package index (already-downloaded metadata, no
/// network access needed for the search itself).
pub fn search(query: &str) -> AppResult<Vec<String>> {
    let output = run("apt-cache", &["search", query])?;
    if !output.status.success() {
        return Err(AppError::Module {
            module: "linux-services::packages",
            message: format!("apt-cache search exited with {}", output.status),
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text.lines().map(|l| l.to_string()).collect())
}

/// Requires root and network access; TCLIS SS6 Level 3 (destructive,
/// confirmation required) in whatever tool wraps this.
pub fn install(package: &str) -> AppResult<()> {
    let output = run("apt-get", &["install", "-y", package])?;
    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::Module {
            module: "linux-services::packages",
            message: format!(
                "installing {package} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        })
    }
}

/// Same permission tier as `install`.
pub fn remove(package: &str) -> AppResult<()> {
    let output = run("apt-get", &["remove", "-y", package])?;
    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::Module {
            module: "linux-services::packages",
            message: format!(
                "removing {package} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_at_least_one_installed_package() {
        // This very sandbox has cargo/rustc installed via apt (see
        // docs/ROADMAP.md's Phase 1 account), so dpkg's own database is
        // never empty here.
        let packages = list_installed().unwrap();
        assert!(!packages.is_empty());
    }

    #[test]
    fn search_finds_a_real_package_by_name() {
        let results = search("coreutils").unwrap();
        assert!(!results.is_empty());
    }
}
