//! Reciprocal Rank Fusion (RRF) — secall `search/hybrid.rs` 이식.
//!
//! Merges independent ranked lists (FTS and vector) into a single score-based
//! ranking without needing comparable absolute scores. Each list contributes
//! `1 / (k + rank)` per result; sums are normalized to [0, 1].
//!
//! RRF is robust to score-scale mismatches (FTS rank vs vector cosine), which
//! is exactly our situation here.

use std::collections::HashMap;

/// Standard RRF constant from the original paper — secall uses 60, same here.
pub const RRF_K: f64 = 60.0;

/// A candidate result identified by a stable key. Different sources (FTS /
/// vector / future rawq) use different key spaces (`msg:<id>`, `doc:<path>`)
/// so collisions are not expected — but if they DO collide, the scores sum,
/// which naturally boosts results that appear in multiple sources.
pub trait RankedCandidate: Clone {
    fn key(&self) -> String;
}

/// Fuse two (or more) ranked lists via reciprocal rank fusion. The returned
/// `Vec<(T, f64)>` is sorted by score descending and the max score is 1.0
/// (normalization). Lower rank in either input list = better.
///
/// The first occurrence of each key wins for the returned candidate payload —
/// callers should pass the more-informative list first if payloads differ.
pub fn reciprocal_rank_fusion<T: RankedCandidate>(
    lists: &[&[T]],
    k: f64,
) -> Vec<(T, f64)> {
    let mut score_map: HashMap<String, f64> = HashMap::new();
    let mut candidate_map: HashMap<String, T> = HashMap::new();

    for list in lists {
        for (rank, item) in list.iter().enumerate() {
            let key = item.key();
            let rrf_score = 1.0 / (k + rank as f64 + 1.0);
            *score_map.entry(key.clone()).or_insert(0.0) += rrf_score;
            candidate_map.entry(key).or_insert_with(|| item.clone());
        }
    }

    let mut merged: Vec<(T, f64)> = candidate_map
        .into_iter()
        .map(|(k, v)| (v, score_map[&k]))
        .collect();

    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Normalize so top result = 1.0
    if let Some(&(_, max)) = merged.first() {
        if max > 0.0 {
            for entry in merged.iter_mut() {
                entry.1 /= max;
            }
        }
    }

    merged
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Item {
        id: String,
        payload: String,
    }

    impl RankedCandidate for Item {
        fn key(&self) -> String {
            self.id.clone()
        }
    }

    fn item(id: &str) -> Item {
        Item { id: id.into(), payload: format!("payload-{id}") }
    }

    #[test]
    fn empty_lists_produce_empty_result() {
        let out = reciprocal_rank_fusion::<Item>(&[], RRF_K);
        assert!(out.is_empty());
    }

    #[test]
    fn single_list_preserves_order() {
        let list = vec![item("a"), item("b"), item("c")];
        let out = reciprocal_rank_fusion(&[list.as_slice()], RRF_K);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].0.id, "a");
        assert_eq!(out[1].0.id, "b");
        assert_eq!(out[2].0.id, "c");
    }

    #[test]
    fn top_result_normalized_to_one() {
        let list = vec![item("a"), item("b")];
        let out = reciprocal_rank_fusion(&[list.as_slice()], RRF_K);
        assert!((out[0].1 - 1.0).abs() < 1e-9);
        assert!(out[1].1 < 1.0);
        assert!(out[1].1 > 0.0);
    }

    #[test]
    fn disjoint_lists_are_merged() {
        let a = vec![item("a"), item("b")];
        let b = vec![item("c"), item("d")];
        let out = reciprocal_rank_fusion(&[a.as_slice(), b.as_slice()], RRF_K);
        let ids: Vec<&str> = out.iter().map(|(i, _)| i.id.as_str()).collect();
        assert_eq!(ids.len(), 4);
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
        assert!(ids.contains(&"c"));
        assert!(ids.contains(&"d"));
    }

    #[test]
    fn overlapping_key_sums_contributions() {
        // Same key in both lists → score is SUM of rrf contributions.
        // This should rank higher than keys that appear in only one list.
        let a = vec![item("shared"), item("a-only")];
        let b = vec![item("shared"), item("b-only")];
        let out = reciprocal_rank_fusion(&[a.as_slice(), b.as_slice()], RRF_K);
        assert_eq!(out[0].0.id, "shared", "overlapping key must rank first");
    }

    #[test]
    fn first_list_wins_payload_on_overlap() {
        // When the same key appears in multiple lists, the FIRST occurrence
        // wins — callers should pass the more-informative list first.
        let a = vec![Item { id: "x".into(), payload: "from_a".into() }];
        let b = vec![Item { id: "x".into(), payload: "from_b".into() }];
        let out = reciprocal_rank_fusion(&[a.as_slice(), b.as_slice()], RRF_K);
        assert_eq!(out[0].0.payload, "from_a");
    }

    #[test]
    fn sort_is_stable_under_ties() {
        // Construct a case where two keys get exactly the same RRF score by
        // appearing only in one list at the same position of different lists.
        let a = vec![item("x")];
        let b = vec![item("y")];
        let out = reciprocal_rank_fusion(&[a.as_slice(), b.as_slice()], RRF_K);
        // Both "x" and "y" are at rank 0 of their list — same contribution.
        // After normalization, both end up at 1.0. Result includes both.
        assert_eq!(out.len(), 2);
        assert!((out[0].1 - 1.0).abs() < 1e-9);
        // The second one should be at 1.0 as well since scores were equal
        // before normalization, then divided by the same max.
        assert!((out[1].1 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn k_value_affects_score_magnitude() {
        let list = vec![item("a"), item("b")];
        let out_small = reciprocal_rank_fusion(&[list.as_slice()], 10.0);
        let out_large = reciprocal_rank_fusion(&[list.as_slice()], 1000.0);
        // Larger k compresses score differences; after normalization the first
        // entry is still 1.0 but the ratio differs.
        // Small k: 1/11 vs 1/12 → ratio ~0.917
        // Large k: 1/1001 vs 1/1002 → ratio ~0.999
        assert!(out_large[1].1 > out_small[1].1);
    }
}
