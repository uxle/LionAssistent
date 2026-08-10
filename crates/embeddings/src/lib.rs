//! `embeddings` -- turns text into a fixed-size vector for semantic
//! search (Archd SS6 "Semantic Memory").
//!
//! [`HashingEmbedder`] is a real, working, dependency-free implementation
//! (the "hashing trick": character trigrams hashed into a fixed-size
//! vector, in the spirit of feature hashing / SimHash). It captures
//! lexical overlap -- similar wording scores higher -- not deep semantic
//! meaning the way a trained neural embedding model would. It needs no
//! model download and runs instantly, so it's genuinely useful today
//! rather than a placeholder, and it's honest about what it isn't.
//!
//! A neural embedder can implement the same [`Embedder`] trait later
//! (Archd SS3 "Replaceable AI backend") -- it would face the same
//! toolchain/network constraints already documented for `llm-runtime` in
//! Phase 4, which is exactly why this crate doesn't depend on Candle to
//! deliver *something* real in the meantime.
//!
//! [`caching::CachingEmbedder`] (Development_plan Phase 18 "Caching")
//! wraps any `Embedder` in a memoizing cache -- useful even for the
//! dependency-free [`HashingEmbedder`] below, since real callers
//! (`memory::MemoryStore::search_memory` embeds its query on every
//! search) can and do re-embed the same text more than once.

mod caching;

pub use caching::CachingEmbedder;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub trait Embedder: Send + Sync {
    fn dimension(&self) -> usize;
    fn embed(&self, text: &str) -> Vec<f32>;
}

/// Feature-hashing embedder over character trigrams.
#[derive(Debug, Clone, Copy)]
pub struct HashingEmbedder {
    dimension: usize,
}

impl HashingEmbedder {
    pub fn new(dimension: usize) -> Self {
        assert!(dimension > 0, "embedding dimension must be positive");
        Self { dimension }
    }
}

impl Default for HashingEmbedder {
    /// 256 dimensions is plenty for lexical-overlap matching at the scale
    /// of a personal memory store, while keeping each stored vector small.
    fn default() -> Self {
        Self::new(256)
    }
}

impl Embedder for HashingEmbedder {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0f32; self.dimension];
        for trigram in char_trigrams(text) {
            let hash = hash_str(&trigram);
            let index = (hash as usize) % self.dimension;
            let sign = if hash & 1 == 0 { 1.0 } else { -1.0 };
            vector[index] += sign;
        }
        normalize(&mut vector);
        vector
    }
}

fn char_trigrams(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.to_lowercase().chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }
    if chars.len() < 3 {
        return vec![chars.into_iter().collect()];
    }
    chars.windows(3).map(|w| w.iter().collect()).collect()
}

fn hash_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in vector.iter_mut() {
            *x /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vector_db::cosine_similarity;

    #[test]
    fn same_text_produces_identical_vectors() {
        let e = HashingEmbedder::default();
        assert_eq!(e.embed("hello world"), e.embed("hello world"));
    }

    #[test]
    fn output_has_the_configured_dimension() {
        let e = HashingEmbedder::new(64);
        assert_eq!(e.embed("anything").len(), 64);
    }

    #[test]
    fn empty_text_yields_a_zero_vector() {
        let e = HashingEmbedder::default();
        assert!(e.embed("").iter().all(|&x| x == 0.0));
    }

    #[test]
    fn similar_text_scores_higher_than_unrelated_text() {
        let e = HashingEmbedder::default();
        let base = e.embed("the quick brown fox jumps over the lazy dog");
        let similar = e.embed("the quick brown fox jumps over a lazy dog");
        let unrelated = e.embed("quantum entanglement in superconducting circuits");

        let sim_similar = cosine_similarity(&base, &similar);
        let sim_unrelated = cosine_similarity(&base, &unrelated);
        assert!(
            sim_similar > sim_unrelated,
            "expected similar text to score higher: {sim_similar} vs {sim_unrelated}"
        );
    }

    #[test]
    fn nonempty_output_is_normalized() {
        let e = HashingEmbedder::default();
        let v = e.embed("some real text with several trigrams");
        let magnitude: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 1e-4);
    }
}
