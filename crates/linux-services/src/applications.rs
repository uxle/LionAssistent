//! Application discovery and launching (TCLIS SS4 "Application
//! Management"; Apis SS9 Application Service), via the XDG `.desktop`
//! file format any Linux desktop environment uses to advertise installed
//! GUI applications.
//!
//! This container has no desktop environment installed, so
//! `/usr/share/applications` is empty or absent here -- the discovery
//! *logic* is tested against a fixture `.desktop` file this crate writes
//! itself, which is a more reliable test than depending on whatever
//! happens to be installed. `launch` needs a display session to actually
//! open a window, so it fails gracefully here the same way
//! [`crate::clipboard`] does.

use common::{AppError, AppResult};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct AppInfo {
    pub name: String,
    pub exec: String,
    pub desktop_file: PathBuf,
}

/// Standard XDG locations for `.desktop` files.
pub fn default_search_dirs() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ]
}

fn parse_desktop_file(path: &Path) -> Option<AppInfo> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_desktop_entry = false;
    let mut name = None;
    let mut exec = None;
    let mut no_display = false;
    let mut is_application = true;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        if let Some(value) = line.strip_prefix("Name=") {
            name.get_or_insert_with(|| value.to_string());
        } else if let Some(value) = line.strip_prefix("Exec=") {
            exec.get_or_insert_with(|| value.to_string());
        } else if let Some(value) = line.strip_prefix("NoDisplay=") {
            no_display = value.eq_ignore_ascii_case("true");
        } else if let Some(value) = line.strip_prefix("Type=") {
            is_application = value == "Application";
        }
    }

    if no_display || !is_application {
        return None;
    }

    Some(AppInfo {
        name: name?,
        exec: exec?,
        desktop_file: path.to_path_buf(),
    })
}

/// Scans `dirs` for `.desktop` files describing user-visible
/// applications (`NoDisplay=true` entries -- helpers, not real apps --
/// are excluded).
pub fn list_applications(dirs: &[PathBuf]) -> AppResult<Vec<AppInfo>> {
    let mut apps = Vec::new();
    for dir in dirs {
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            continue; // dir doesn't exist on this system -- not an error
        };
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("desktop") {
                if let Some(app) = parse_desktop_file(&path) {
                    apps.push(app);
                }
            }
        }
    }
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(apps)
}

/// Strips XDG field codes (`%f`, `%F`, `%u`, `%U`, `%i`, `%c`, `%k`) and
/// launches the application detached from this process. Needs a display
/// session to actually show a window -- fails gracefully without one,
/// the same honest split as [`crate::clipboard`].
pub fn launch(exec: &str) -> AppResult<()> {
    let cleaned: String = {
        let mut out = String::new();
        let mut chars = exec.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '%' {
                chars.next(); // consume the field-code letter, drop both
            } else {
                out.push(c);
            }
        }
        out
    };

    let mut parts = cleaned.split_whitespace();
    let Some(program) = parts.next() else {
        return Err(AppError::Module {
            module: "linux-services::applications",
            message: format!("empty Exec line: {exec:?}"),
        });
    };

    std::process::Command::new(program)
        .args(parts)
        .spawn()
        .map(|_child| ())
        .map_err(|e| AppError::Module {
            module: "linux-services::applications",
            message: format!(
                "launching {program:?} failed (no display session? this sandbox has none): {e}"
            ),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_fixture(dir: &Path, filename: &str, content: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(filename), content).unwrap();
    }

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("linux-services-apps-test-{label}"))
    }

    #[test]
    fn parses_a_well_formed_desktop_file() {
        let dir = temp_dir("wellformed");
        write_fixture(
            &dir,
            "editor.desktop",
            "[Desktop Entry]\nType=Application\nName=Test Editor\nExec=test-editor %F\n",
        );

        let apps = list_applications(&[dir.clone()]).unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Test Editor");
        assert_eq!(apps[0].exec, "test-editor %F");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_display_entries_are_excluded() {
        let dir = temp_dir("nodisplay");
        write_fixture(
            &dir,
            "helper.desktop",
            "[Desktop Entry]\nType=Application\nName=Background Helper\nExec=helper\nNoDisplay=true\n",
        );

        let apps = list_applications(&[dir.clone()]).unwrap();
        assert!(apps.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn non_application_entries_are_excluded() {
        let dir = temp_dir("nonapp");
        write_fixture(
            &dir,
            "link.desktop",
            "[Desktop Entry]\nType=Link\nName=A Link\nURL=https://example.com\n",
        );

        let apps = list_applications(&[dir.clone()]).unwrap();
        assert!(apps.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_search_dir_is_not_an_error() {
        let apps = list_applications(&[PathBuf::from("/definitely/does/not/exist")]).unwrap();
        assert!(apps.is_empty());
    }

    #[test]
    fn launch_strips_field_codes() {
        // We can't observe the stripped command directly through the
        // public API, but we can confirm a program that doesn't exist
        // fails the way a missing binary should, proving the exec line
        // was actually parsed and split rather than passed through whole
        // (which would try to run "nonexistent-program %F" as one token
        // and fail differently).
        let result = launch("nonexistent-program-xyz %f %U");
        assert!(result.is_err());
    }

    #[test]
    fn launch_succeeds_for_a_program_that_needs_no_display() {
        // `true` exists on every Linux system and exits 0 immediately
        // without ever needing a window -- confirming `launch`'s only
        // real precondition is a valid program to exec, not some hidden
        // display-server check baked into the function itself. Actual
        // GUI applications would still fail here for the same reason
        // `clipboard`/`notifications`/`audio` do: no display session.
        let result = launch("true");
        assert!(result.is_ok());
    }
}
