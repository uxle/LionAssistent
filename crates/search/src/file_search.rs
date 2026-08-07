//! "File Search" (Development_plan Phase 13), wrapping Phase 8's
//! `linux_services::files::search` (TCLIS SS4 "File Management" "Search"
//! capability). That function already does the real work -- recursive,
//! depth-capped, case-insensitive substring matching over the real
//! filesystem -- so this module's job is narrow: search across more than
//! one root directory and rank the combined results the same way
//! `crate::command_search` ranks its own, so `SearchEngine::search_all`
//! has something comparable to merge.

use crate::provider::SearchProvider;
use crate::ranking::score_text;
use crate::types::{SearchHit, SearchKind};
use common::AppResult;
use std::path::PathBuf;

/// `$HOME` if set, otherwise no default roots at all -- searching zero
/// directories is not an error (see [`FileSearchProvider::search`]), so
/// there's no need to fail construction just because the environment is
/// unusual (a stripped-down container, say).
pub fn default_roots() -> Vec<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .into_iter()
        .collect()
}

#[derive(Debug, Clone)]
pub struct FileSearchProvider {
    roots: Vec<PathBuf>,
    max_depth: usize,
}

impl FileSearchProvider {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            max_depth: 6,
        }
    }

    /// Overrides the default recursion depth (6), matching
    /// `linux_services::files::search`'s own depth cap -- kept low
    /// enough to avoid runaway recursion into huge or cyclic (symlink)
    /// trees, per that function's own documentation.
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }
}

impl SearchProvider for FileSearchProvider {
    /// Missing or unreadable roots are skipped rather than failing the
    /// whole search -- the same "one bad subdirectory shouldn't sink the
    /// query" philosophy `linux_services::files::search`'s own recursion
    /// already uses for unreadable subdirectories. A genuinely
    /// misconfigured root (nothing under `$HOME`, say) should read as
    /// "no results," not "search is broken."
    fn search(&self, query: &str, limit: usize) -> AppResult<Vec<SearchHit>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut hits = Vec::new();
        for root in &self.roots {
            let Ok(paths) = linux_services::files::search(root, query, self.max_depth) else {
                continue;
            };
            for path in paths {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                // `linux_services::files::search` already confirmed
                // `query` is a case-insensitive substring of `name`, so
                // `score_text` reliably lands in its "contains" tier or
                // above here -- this is a ranking pass over
                // already-relevant matches, not a second filter.
                let score = score_text(query, &name);
                let path_str = path.display().to_string();
                hits.push(SearchHit {
                    kind: SearchKind::File,
                    title: name,
                    snippet: path_str.clone(),
                    score,
                    source_id: path_str,
                });
            }
        }

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.title.cmp(&b.title))
        });
        hits.truncate(limit);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("search-file-search-test-{label}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_and_ranks_matching_files() {
        let dir = temp_dir("basic");
        fs::write(dir.join("quarterly-report.pdf"), "").unwrap();
        fs::write(dir.join("report-cover.txt"), "").unwrap();
        fs::write(dir.join("unrelated.txt"), "").unwrap();

        let provider = FileSearchProvider::new(vec![dir.clone()]);
        let hits = provider.search("report", 10).unwrap();

        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.kind == SearchKind::File));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn respects_the_limit() {
        let dir = temp_dir("limit");
        for i in 0..5 {
            fs::write(dir.join(format!("note-{i}.txt")), "").unwrap();
        }

        let provider = FileSearchProvider::new(vec![dir.clone()]);
        let hits = provider.search("note", 2).unwrap();
        assert_eq!(hits.len(), 2);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn aggregates_across_multiple_roots() {
        let dir_a = temp_dir("multi-a");
        let dir_b = temp_dir("multi-b");
        fs::write(dir_a.join("alpha-notes.txt"), "").unwrap();
        fs::write(dir_b.join("beta-notes.txt"), "").unwrap();

        let provider = FileSearchProvider::new(vec![dir_a.clone(), dir_b.clone()]);
        let hits = provider.search("notes", 10).unwrap();
        assert_eq!(hits.len(), 2);

        fs::remove_dir_all(&dir_a).ok();
        fs::remove_dir_all(&dir_b).ok();
    }

    #[test]
    fn a_missing_root_is_skipped_not_an_error() {
        let provider = FileSearchProvider::new(vec![PathBuf::from(
            "/definitely/not/a/real/directory/anywhere",
        )]);
        let hits = provider.search("anything", 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn empty_query_returns_no_results_rather_than_everything() {
        let dir = temp_dir("empty-query");
        fs::write(dir.join("something.txt"), "").unwrap();

        let provider = FileSearchProvider::new(vec![dir.clone()]);
        assert!(provider.search("", 10).unwrap().is_empty());
        assert!(provider.search("   ", 10).unwrap().is_empty());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_roots_configured_returns_no_results() {
        let provider = FileSearchProvider::new(vec![]);
        assert!(provider.search("anything", 10).unwrap().is_empty());
    }
}
