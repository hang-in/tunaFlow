//! Unified search module — Phase A (query expansion), Phase B (hybrid RRF),
//! Phase C (Korean tokenizer) from `docs/plans/searchPipelineFromSecallPlan.md`.

pub mod hybrid;
pub mod query_expand;
pub mod unified;

#[allow(unused_imports)]
pub use query_expand::{expand_query, normalize_query, query_expansion_enabled};
#[allow(unused_imports)]
pub use unified::{search_unified, UnifiedResult};
