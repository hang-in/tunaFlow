//! Unified search module — Phase A (query expansion), Phase B (hybrid RRF),
//! Phase C (Korean tokenizer) from `docs/plans/searchPipelineFromSecallPlan.md`.
//!
//! Phase A 현재: query expansion + 7-day DB cache. Default OFF (opt-in via
//! `TUNAFLOW_QUERY_EXPANSION`). Future phases chain on this module.

pub mod query_expand;

#[allow(unused_imports)]
pub use query_expand::{expand_query, normalize_query, query_expansion_enabled};
