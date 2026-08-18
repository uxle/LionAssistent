//! Document chunking (Development_plan Phase 26 "Document Indexing").
//! Splits on whitespace into a word-count sliding window with overlap,
//! deliberately not attempting real sentence/paragraph segmentation:
//! that needs real language understanding to do honestly (abbreviations,
//! decimal numbers, quoted speech all break a naive `.` -based
//! splitter), the same reasoning `planner`'s doc comment gives for why
//! picking a specific tool from a broad intent needs a working
//! `llm-runtime` rather than a heuristic pretending to be one. A
//! word-count window is simple, deterministic, fully testable, and
//! honest about being exactly that -- not a step toward sentence-aware
//! chunking that was never finished.

use common::{AppError, AppResult};

/// Reasonable defaults: 200 words is roughly 260-300 English tokens --
/// large enough to keep real context together, small enough that a
/// `HashingEmbedder`'s trigram features (or the input window of a real
/// future embedding model) don't wash out any one chunk's specific
/// content. 40 words (20%) of overlap follows common RAG practice of
/// not losing content that happens to fall on a chunk boundary --
/// reasoned defaults, not measured against this project's own embedder
/// on a real retrieval benchmark, which doesn't exist yet.
pub const DEFAULT_MAX_WORDS: usize = 200;
pub const DEFAULT_OVERLAP_WORDS: usize = 40;

fn module_err(context: impl std::fmt::Display) -> AppError {
    AppError::Module {
        module: "rag",
        message: context.to_string(),
    }
}

/// Splits `text` into chunks of at most `max_words` words, each
/// consecutive pair overlapping by `overlap_words` words. Chunks are
/// rejoined with single spaces -- original whitespace (newlines, runs
/// of spaces) isn't preserved, since the point is a chunk's semantic
/// content for embedding, not a byte-exact excerpt.
pub fn chunk_text(text: &str, max_words: usize, overlap_words: usize) -> AppResult<Vec<String>> {
    if max_words == 0 {
        return Err(module_err("max_words must be at least 1"));
    }
    if overlap_words >= max_words {
        return Err(module_err(format!(
            "overlap_words ({overlap_words}) must be less than max_words ({max_words}), or \
             every chunk would advance by zero real words"
        )));
    }

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Ok(Vec::new());
    }

    let step = max_words - overlap_words;
    let mut chunks = Vec::new();
    let mut start = 0;
    loop {
        let end = (start + max_words).min(words.len());
        chunks.push(words[start..end].join(" "));
        if end == words.len() {
            break;
        }
        start += step;
    }
    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_is_a_single_chunk() {
        let chunks = chunk_text("just a few words here", 200, 40).unwrap();
        assert_eq!(chunks, vec!["just a few words here"]);
    }

    #[test]
    fn empty_text_produces_no_chunks() {
        assert_eq!(chunk_text("", 200, 40).unwrap(), Vec::<String>::new());
        assert_eq!(
            chunk_text("   \n  ", 200, 40).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn long_text_splits_into_multiple_overlapping_chunks() {
        let words: Vec<String> = (0..250).map(|i| format!("w{i}")).collect();
        let text = words.join(" ");

        let chunks = chunk_text(&text, 100, 20).unwrap();
        // step = 80: starts at 0, 80, 160 -> chunks [0,100), [80,180),
        // [160,250) -- the third start (160+100=260) clamps to the real
        // length (250) and that's the final chunk, so 3, not 4. Traced
        // by hand and double-checked against a small script before
        // being written down as this test's expected value, not assumed.
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].split_whitespace().count(), 100);
        assert_eq!(chunks[1].split_whitespace().count(), 100);

        // Consecutive chunks genuinely share words, not just adjacent
        // ones -- real overlap, not an off-by-one illusion of one.
        let first_words: Vec<&str> = chunks[0].split_whitespace().collect();
        let second_words: Vec<&str> = chunks[1].split_whitespace().collect();
        let overlap: Vec<&&str> = first_words
            .iter()
            .filter(|w| second_words.contains(w))
            .collect();
        assert_eq!(overlap.len(), 20);
    }

    #[test]
    fn the_last_chunk_is_never_dropped_even_if_shorter_than_max_words() {
        let words: Vec<String> = (0..90).map(|i| format!("w{i}")).collect();
        let text = words.join(" ");
        let chunks = chunk_text(&text, 50, 10).unwrap();

        let total_unique_words_covered: std::collections::HashSet<&str> =
            chunks.iter().flat_map(|c| c.split_whitespace()).collect();
        assert_eq!(
            total_unique_words_covered.len(),
            90,
            "every word must appear in some chunk"
        );
        assert!(
            chunks.last().unwrap().contains("w89"),
            "the final word must not be dropped"
        );
    }

    #[test]
    fn zero_max_words_is_a_clean_error() {
        assert!(chunk_text("some text", 0, 0).is_err());
    }

    #[test]
    fn overlap_at_or_above_max_words_is_a_clean_error() {
        assert!(chunk_text("some text here", 10, 10).is_err());
        assert!(chunk_text("some text here", 10, 15).is_err());
    }

    #[test]
    fn multiple_whitespace_and_newlines_are_normalized_to_single_spaces() {
        let chunks = chunk_text("word1\n\nword2   word3\tword4", 200, 40).unwrap();
        assert_eq!(chunks, vec!["word1 word2 word3 word4"]);
    }
}
