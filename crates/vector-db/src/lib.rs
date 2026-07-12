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
    /// highest first. Vectors of a different dimension than `query` are
    /// skipped rather than causing an error, since callers may hold
    /// embeddings from more than one embedder over time.
    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<(Id, f32)> {
        let mut scored: Vec<(Id, f32)> = self
            .entries
            .iter()
            .filter(|e| e.vector.len() == query.len())
            .map(|e| (e.id.clone(), cosine_similarity(query, &e.vector)))
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored.truncate(top_k);
        scored
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
}
