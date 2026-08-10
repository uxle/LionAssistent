//! `vector-db` -- semantic vector storage and similarity search (Archd
//! SS6 "Semantic Memory"; DBDMS SS7 "Vector Database").
//!
//! Brute-force cosine similarity, deliberately: at the scale this project
//! targets (a single local user's memories -- hundreds to low thousands
//! of items, not millions), a linear scan is fast enough and needs no
//! approximate-nearest-neighbor index or its tuning knobs. Revisit if
//! `search` ever shows up in a profile.

/// One stored vector, keyed by an arbitrary id (the caller decides what
/// `Id` means -- `memory`'s crate uses `common::Id`).
#[derive(Debug, Clone)]
pub struct VectorEntry<Id> {
    pub id: Id,
    pub vector: Vec<f32>,
}

#[derive(Debug, Default)]
pub struct VectorIndex<Id> {
    entries: Vec<VectorEntry<Id>>,
}

impl<Id> VectorIndex<Id> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<Id: PartialEq + Clone> VectorIndex<Id> {
    /// Inserts a vector, replacing any existing entry with the same id.
    pub fn insert(&mut self, id: Id, vector: Vec<f32>) {
        self.entries.retain(|e| e.id != id);
        self.entries.push(VectorEntry { id, vector });
    }

    pub fn remove(&mut self, id: &Id) {
        self.entries.retain(|e| &e.id != id);
    }

    /// Returns up to `top_k` ids ranked by cosine similarity to `query`,
    /// highest first. Ties (equal similarity) break in the entries'
    /// original relative order, oldest first. Vectors of a different
    /// dimension than `query` are skipped rather than causing an error,
    /// since callers may hold embeddings from more than one embedder
    /// over time.
    ///
    /// Development_plan Phase 18 "Memory Optimization": scores every
    /// entry (still an unavoidable, honest O(n) scan -- see the crate
    /// doc comment), but no longer fully sorts the whole result set just
    /// to keep the top few. `select_nth_unstable_by` partitions the
    /// requested `top_k` to the front in O(n) average time, so only that
    /// slice -- not all of `self.entries` -- ever pays for a comparison
    /// sort. `select_nth_unstable_by` is, as the name says, *unstable*:
    /// left to compare scores alone it could shuffle tied entries
    /// differently than the plain `sort_by` this replaced. Breaking ties
    /// on each entry's original position (tracked via `enumerate`, not
    /// exposed in the return value) makes the result byte-for-byte
    /// identical to the old full-stable-sort behavior for every input,
    /// ties included -- not just usually the same.
    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<(Id, f32)> {
        let mut scored: Vec<(usize, Id, f32)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.vector.len() == query.len())
            .map(|(i, e)| (i, e.id.clone(), cosine_similarity(query, &e.vector)))
            .collect();

        if top_k == 0 {
            return Vec::new();
        }

        let rank = |a: &(usize, Id, f32), b: &(usize, Id, f32)| {
            b.2.total_cmp(&a.2).then_with(|| a.0.cmp(&b.0))
        };
        if top_k < scored.len() {
            scored.select_nth_unstable_by(top_k - 1, rank);
            scored.truncate(top_k);
        }
        scored.sort_by(rank);
        scored
            .into_iter()
            .map(|(_, id, score)| (id, score))
            .collect()
    }
}

/// Cosine similarity in `[-1.0, 1.0]`; `0.0` if either vector has zero
/// magnitude (rather than dividing by zero / returning NaN).
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_vectors_have_similarity_one() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn orthogonal_vectors_have_similarity_zero() {
        assert!((cosine_similarity(&[1.0, 0.0], &[0.0, 1.0])).abs() < 1e-6);
    }

    #[test]
    fn opposite_vectors_have_similarity_negative_one() {
        assert!((cosine_similarity(&[1.0, 0.0], &[-1.0, 0.0]) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn zero_vector_has_similarity_zero_not_nan() {
        let score = cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn search_ranks_by_similarity_descending() {
        let mut index = VectorIndex::new();
        index.insert("far", vec![0.0, 1.0]);
        index.insert("near", vec![0.99, 0.1]);
        index.insert("exact", vec![1.0, 0.0]);

        let results = index.search(&[1.0, 0.0], 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "exact");
        assert_eq!(results[1].0, "near");
    }

    #[test]
    fn insert_replaces_existing_id() {
        let mut index = VectorIndex::new();
        index.insert("a", vec![1.0, 0.0]);
        index.insert("a", vec![0.0, 1.0]);
        assert_eq!(index.len(), 1);
        assert_eq!(index.search(&[0.0, 1.0], 1)[0].1, 1.0);
    }

    #[test]
    fn remove_drops_the_entry() {
        let mut index = VectorIndex::new();
        index.insert("a", vec![1.0, 0.0]);
        index.remove(&"a");
        assert!(index.is_empty());
    }

    #[test]
    fn mismatched_dimensions_are_skipped_not_errors() {
        let mut index = VectorIndex::new();
        index.insert("wrong-dim", vec![1.0, 0.0, 0.0]);
        index.insert("right-dim", vec![1.0, 0.0]);
        let results = index.search(&[1.0, 0.0], 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "right-dim");
    }

    #[test]
    fn top_k_of_zero_returns_nothing() {
        let mut index = VectorIndex::new();
        index.insert("a", vec![1.0, 0.0]);
        assert!(index.search(&[1.0, 0.0], 0).is_empty());
    }

    #[test]
    fn top_k_at_or_beyond_the_entry_count_returns_everything_sorted() {
        let mut index = VectorIndex::new();
        index.insert("far", vec![0.0, 1.0]);
        index.insert("near", vec![0.99, 0.1]);
        index.insert("exact", vec![1.0, 0.0]);

        for requested in [3, 4, 100] {
            let results = index.search(&[1.0, 0.0], requested);
            assert_eq!(results.len(), 3, "requested top_k={requested}");
            assert_eq!(results[0].0, "exact");
            assert_eq!(results[1].0, "near");
            assert_eq!(results[2].0, "far");
        }
    }

    /// Phase 18's `select_nth_unstable_by` partial-selection rewrite of
    /// `search` must return exactly what a naive "score everything, sort
    /// everything, truncate" reference implementation would -- same ids,
    /// same order -- for every `top_k` from `0` to well past `n`, not
    /// just the handful of entries the other hand-written tests above
    /// happen to cover. 128 entries with a deterministic pseudo-random
    /// (not measurement-sensitive) spread of similarities gives the
    /// partial-selection path (`top_k < n`) real work to partition,
    /// including ties.
    #[test]
    fn search_matches_a_naive_full_sort_reference_across_many_top_k_values() {
        fn naive_search(
            entries: &[(u32, Vec<f32>)],
            query: &[f32],
            top_k: usize,
        ) -> Vec<(u32, f32)> {
            let mut scored: Vec<(u32, f32)> = entries
                .iter()
                .filter(|(_, v)| v.len() == query.len())
                .map(|(id, v)| (*id, cosine_similarity(query, v)))
                .collect();
            scored.sort_by(|a, b| b.1.total_cmp(&a.1));
            scored.truncate(top_k);
            scored
        }

        // A small deterministic LCG stands in for randomness without
        // pulling in a `rand` dependency for one test -- this crate is
        // zero-dependency by design (see the crate doc comment).
        let mut state: u64 = 0x2545F4914F6CDD1D;
        let mut next = move || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((state >> 33) as u32) as f32 / u32::MAX as f32
        };

        let mut index = VectorIndex::new();
        let mut raw: Vec<(u32, Vec<f32>)> = Vec::new();
        for id in 0..128u32 {
            // Every 10th vector is an exact duplicate of another one, so
            // the reference and the optimized path both have real ties
            // to resolve consistently, not just distinct scores.
            let vector = if id % 10 == 0 && id > 0 {
                raw[(id - 1) as usize].1.clone()
            } else {
                vec![next(), next(), next(), next()]
            };
            index.insert(id, vector.clone());
            raw.push((id, vector));
        }

        let query = vec![0.5, 0.5, 0.5, 0.5];
        for top_k in [0, 1, 2, 5, 17, 63, 128, 200] {
            let optimized = index.search(&query, top_k);
            let reference = naive_search(&raw, &query, top_k);
            assert_eq!(
                optimized.len(),
                reference.len(),
                "top_k={top_k}: result count differs"
            );
            for (i, ((opt_id, opt_score), (ref_id, ref_score))) in
                optimized.iter().zip(reference.iter()).enumerate()
            {
                assert_eq!(opt_id, ref_id, "top_k={top_k}: id mismatch at position {i}");
                assert!(
                    (opt_score - ref_score).abs() < 1e-6,
                    "top_k={top_k}: score mismatch at position {i}: {opt_score} vs {ref_score}"
                );
            }
        }
    }
}
