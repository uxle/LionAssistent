//! Deterministic, dependency-free relevance scoring shared by the file
//! and command search providers (Development_plan Phase 13 "Global
//! Search" needs one consistent scale to merge results from otherwise
//! hard-to-compare sources). A small, clearly-defined cascade of rules
//! covers what this workspace's searches actually need without an
//! external fuzzy-matching crate -- DIG SS11 "avoid introducing
//! unrelated dependencies" -- and is simple enough to unit-test
//! exhaustively, unlike a real edit-distance or BM25 implementation.

/// Scores how well `haystack` matches `query`, `0.0..=1.0`. `0.0` means
/// no match at all; callers generally exclude those rather than display
/// a zero-relevance result. Case-insensitive throughout.
///
/// Rules, in order (first match wins, so the scale is strictly ordered
/// by how strong a signal each is):
///
/// 1. Exact match (whole string, case-insensitive) -- `1.0`.
/// 2. `haystack` starts with `query` -- `0.9`.
/// 3. `haystack` contains `query` as a substring -- `0.75`.
/// 4. Multi-word `query`: some (not all, or that would already have hit
///    rule 3) of its words appear in `haystack` -- `0.3..=0.7`, scaled
///    by the fraction matched.
/// 5. `query`'s characters appear in `haystack` in order, not
///    necessarily contiguous (a loose fuzzy fallback, e.g. `"gcm"`
///    against `"git commit message"`) -- `0.2`.
/// 6. Otherwise -- `0.0`.
pub fn score_text(query: &str, haystack: &str) -> f32 {
    let query = query.trim();
    if query.is_empty() {
        return 0.0;
    }
    let q = query.to_lowercase();
    let h = haystack.to_lowercase();

    if h == q {
        return 1.0;
    }
    if h.starts_with(&q) {
        return 0.9;
    }
    if h.contains(&q) {
        return 0.75;
    }

    let words: Vec<&str> = q.split_whitespace().collect();
    if words.len() > 1 {
        let matched = words.iter().filter(|w| h.contains(*w)).count();
        if matched > 0 {
            return 0.3 + 0.4 * (matched as f32 / words.len() as f32);
        }
    }

    if is_subsequence(&q, &h) {
        return 0.2;
    }

    0.0
}

/// Whether every character of `needle` appears in `haystack` in the same
/// relative order (not necessarily contiguously).
fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut hay = haystack.chars();
    needle.chars().all(|c| hay.any(|h| h == c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_scores_highest() {
        assert_eq!(score_text("report", "report"), 1.0);
        assert_eq!(score_text("Report", "report"), 1.0, "case-insensitive");
    }

    #[test]
    fn prefix_match_scores_above_substring_match() {
        let prefix = score_text("rep", "report.pdf");
        let substring = score_text("port", "report.pdf");
        assert!(prefix > substring);
        assert_eq!(prefix, 0.9);
        assert_eq!(substring, 0.75);
    }

    #[test]
    fn multi_word_partial_match_scores_between_subsequence_and_substring() {
        // "monthly" isn't in the haystack at all, "report" is -- 1 of 2
        // words, so partial credit, but strictly less than a full
        // substring match of the whole phrase.
        let score = score_text("monthly report", "quarterly report draft");
        assert!(score > 0.2);
        assert!(score < 0.75);
    }

    #[test]
    fn full_multi_word_match_out_of_order_beats_partial_but_not_contiguous() {
        let full_scattered = score_text("report monthly", "monthly team report");
        let contiguous = score_text("report", "monthly team report");
        assert!(full_scattered < contiguous.max(0.75)); // never beats a real substring hit
        assert!(full_scattered > 0.3);
    }

    #[test]
    fn subsequence_fallback_catches_loose_fuzzy_matches() {
        assert_eq!(score_text("gcm", "git commit message"), 0.2);
    }

    #[test]
    fn no_match_scores_zero() {
        assert_eq!(score_text("xyz", "abc def"), 0.0);
    }

    #[test]
    fn empty_query_scores_zero() {
        assert_eq!(score_text("", "anything"), 0.0);
        assert_eq!(score_text("   ", "anything"), 0.0);
    }

    #[test]
    fn is_subsequence_respects_character_order() {
        assert!(is_subsequence("abc", "a1b2c3"));
        assert!(!is_subsequence("cba", "a1b2c3"), "order matters");
        assert!(!is_subsequence("xyz", "a1b2c3"));
    }
}
