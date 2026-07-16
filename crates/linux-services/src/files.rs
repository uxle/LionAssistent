//! File operations (TCLIS SS4 "File Management"; Apis SS9 File Service).
//! `delete_file` requires an explicit `confirm: true` -- PRD SS6.6:
//! "Dangerous operations must require confirmation" -- there is no way
//! to delete a file through this module without it.

use crate::io_err;
use common::{AppError, AppResult};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub size: u64,
    pub is_dir: bool,
    pub is_file: bool,
    pub readonly: bool,
    pub modified: Option<SystemTime>,
}

fn require_exists(path: &Path) -> AppResult<()> {
    if !path.exists() {
        return Err(AppError::Module {
            module: "linux-services::files",
            message: format!("no such file or directory: {}", path.display()),
        });
    }
    Ok(())
}

pub fn list_directory(dir: &Path) -> AppResult<Vec<FileEntry>> {
    require_exists(dir)?;
    let read_dir =
        std::fs::read_dir(dir).map_err(|e| io_err("linux-services::files", dir.display(), e))?;

    let mut entries = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|e| io_err("linux-services::files", dir.display(), e))?;
        let metadata = entry
            .metadata()
            .map_err(|e| io_err("linux-services::files", entry.path().display(), e))?;
        entries.push(FileEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: entry.path(),
            is_dir: metadata.is_dir(),
            size: metadata.len(),
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// Reads a file as UTF-8 text. Binary files aren't this function's job
/// (Apis SS9 separates "read" from richer document handling elsewhere).
pub fn read_file(path: &Path) -> AppResult<String> {
    require_exists(path)?;
    std::fs::read_to_string(path).map_err(|e| io_err("linux-services::files", path.display(), e))
}

/// Creates or overwrites a file with `content` (TCLIS "Write files").
pub fn write_file(path: &Path, content: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| io_err("linux-services::files", parent.display(), e))?;
        }
    }
    std::fs::write(path, content).map_err(|e| io_err("linux-services::files", path.display(), e))
}

/// Deletes a file. Requires `confirm: true` -- PRD SS6.6.
pub fn delete_file(path: &Path, confirm: bool) -> AppResult<()> {
    if !confirm {
        return Err(AppError::Module {
            module: "linux-services::files",
            message: format!(
                "refusing to delete {} without explicit confirmation",
                path.display()
            ),
        });
    }
    require_exists(path)?;
    std::fs::remove_file(path).map_err(|e| io_err("linux-services::files", path.display(), e))
}

pub fn rename(from: &Path, to: &Path) -> AppResult<()> {
    require_exists(from)?;
    std::fs::rename(from, to).map_err(|e| {
        io_err(
            "linux-services::files",
            format!("{} -> {}", from.display(), to.display()),
            e,
        )
    })
}

/// Moves a file, falling back to copy-then-delete if `rename` fails
/// (e.g. across filesystems, where a plain rename can't work).
pub fn move_file(from: &Path, to: &Path) -> AppResult<()> {
    require_exists(from)?;
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to).map_err(|e| {
        io_err(
            "linux-services::files",
            format!("{} -> {}", from.display(), to.display()),
            e,
        )
    })?;
    std::fs::remove_file(from).map_err(|e| io_err("linux-services::files", from.display(), e))
}

pub fn copy_file(from: &Path, to: &Path) -> AppResult<()> {
    require_exists(from)?;
    std::fs::copy(from, to).map(|_| ()).map_err(|e| {
        io_err(
            "linux-services::files",
            format!("{} -> {}", from.display(), to.display()),
            e,
        )
    })
}

pub fn metadata(path: &Path) -> AppResult<FileMetadata> {
    require_exists(path)?;
    let m =
        std::fs::metadata(path).map_err(|e| io_err("linux-services::files", path.display(), e))?;
    Ok(FileMetadata {
        size: m.len(),
        is_dir: m.is_dir(),
        is_file: m.is_file(),
        readonly: m.permissions().readonly(),
        modified: m.modified().ok(),
    })
}

/// Recursively searches `dir` for entries whose name contains `pattern`
/// (case-insensitive substring match, not a full glob engine). Depth is
/// capped to avoid runaway recursion into symlink cycles or enormous
/// trees.
pub fn search(dir: &Path, pattern: &str, max_depth: usize) -> AppResult<Vec<PathBuf>> {
    require_exists(dir)?;
    let pattern = pattern.to_lowercase();
    let mut results = Vec::new();
    search_inner(dir, &pattern, max_depth, &mut results)?;
    Ok(results)
}

fn search_inner(
    dir: &Path,
    pattern: &str,
    depth_remaining: usize,
    results: &mut Vec<PathBuf>,
) -> AppResult<()> {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(()), // unreadable subdirectory (permissions, etc.) -- skip, don't fail the whole search
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.contains(pattern) {
            results.push(path.clone());
        }
        if depth_remaining > 0 && path.is_dir() {
            search_inner(&path, pattern, depth_remaining - 1, results)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("linux-services-files-test-{label}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_and_reads_a_file() {
        let dir = temp_dir("write-read");
        let path = dir.join("hello.txt");
        write_file(&path, "hello world").unwrap();
        assert_eq!(read_file(&path).unwrap(), "hello world");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reading_a_missing_file_is_a_clear_error_not_a_panic() {
        let result = read_file(Path::new("/definitely/does/not/exist/anywhere"));
        assert!(result.is_err());
    }

    #[test]
    fn delete_without_confirmation_is_refused() {
        let dir = temp_dir("delete-refuse");
        let path = dir.join("keep-me.txt");
        write_file(&path, "data").unwrap();

        assert!(delete_file(&path, false).is_err());
        assert!(path.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_with_confirmation_removes_the_file() {
        let dir = temp_dir("delete-confirm");
        let path = dir.join("remove-me.txt");
        write_file(&path, "data").unwrap();

        delete_file(&path, true).unwrap();
        assert!(!path.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn lists_directory_contents_sorted() {
        let dir = temp_dir("list");
        write_file(&dir.join("b.txt"), "").unwrap();
        write_file(&dir.join("a.txt"), "").unwrap();

        let entries = list_directory(&dir).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["a.txt", "b.txt"]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn copies_a_file_leaving_the_original() {
        let dir = temp_dir("copy");
        let src = dir.join("src.txt");
        let dst = dir.join("dst.txt");
        write_file(&src, "content").unwrap();

        copy_file(&src, &dst).unwrap();
        assert_eq!(read_file(&dst).unwrap(), "content");
        assert!(src.exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn moves_a_file_removing_the_original() {
        let dir = temp_dir("move");
        let src = dir.join("src.txt");
        let dst = dir.join("dst.txt");
        write_file(&src, "content").unwrap();

        move_file(&src, &dst).unwrap();
        assert!(!src.exists());
        assert_eq!(read_file(&dst).unwrap(), "content");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn metadata_reports_size_and_kind() {
        let dir = temp_dir("metadata");
        let path = dir.join("f.txt");
        write_file(&path, "12345").unwrap();

        let m = metadata(&path).unwrap();
        assert_eq!(m.size, 5);
        assert!(m.is_file);
        assert!(!m.is_dir);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_finds_matching_names_recursively() {
        let dir = temp_dir("search");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        write_file(&dir.join("report.pdf"), "").unwrap();
        write_file(&dir.join("sub").join("summary.pdf"), "").unwrap();
        write_file(&dir.join("notes.txt"), "").unwrap();

        let results = search(&dir, "pdf", 5).unwrap();
        assert_eq!(results.len(), 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_respects_max_depth() {
        let dir = temp_dir("search-depth");
        std::fs::create_dir_all(dir.join("a").join("b")).unwrap();
        write_file(&dir.join("a").join("b").join("deep.pdf"), "").unwrap();

        let results = search(&dir, "pdf", 0).unwrap();
        assert!(
            results.is_empty(),
            "depth 0 should not descend into subdirectories"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
