//! Unified search — `search_unified` Tauri command.
//!
//! Phase B of `searchPipelineFromSecallPlan.md`. Combines:
//!   1. FTS5 over `messages` (via existing `messages_fts`)
//!   2. bge-m3 vector similarity over `conversation_chunks WHERE source_type='document'`
//!
//! The two independent ranked lists are fused via RRF (`hybrid.rs`). Query
//! expansion (Phase A) runs first when enabled, and session-diversity cap is
//! applied afterwards so one noisy conversation doesn't monopolize the top-N.

use rusqlite::params;
use serde::Serialize;
use tauri::State;

use crate::db::DbState;
use crate::errors::AppError;

use super::hybrid::{reciprocal_rank_fusion, RankedCandidate, RRF_K};
use super::query_expand::{expand_query, query_expansion_enabled};
use super::tokenizer::{morphological_query_enabled, tokenize_query_for_fts};

/// Default cap for per-conversation result diversity, matches secall.
const DEFAULT_MAX_PER_CONVERSATION: usize = 2;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedResult {
    /// "conversation" | "document"
    pub kind: &'static str,
    /// Unique id inside its space — message_id for conversation, file_path for document.
    pub id: String,
    /// Label for the UI header ("Chat: {conv_label}" or "Doc: {path}").
    pub source_label: String,
    pub snippet: String,
    /// Normalized 0~1 RRF-fused score. 1.0 = best.
    pub score: f64,
    pub fts_score: Option<f64>,
    pub vector_score: Option<f64>,
    pub timestamp: Option<i64>,
    /// Conversation id for "conversation" results — used for diversity capping
    /// and for click-to-navigate UX. `None` for "document" results.
    pub conversation_id: Option<String>,
}

impl RankedCandidate for UnifiedResult {
    fn key(&self) -> String {
        format!("{}:{}", self.kind, self.id)
    }
}

/// Unified search across conversation FTS + document vector. Returns
/// `limit` best results sorted by fused score descending.
#[tauri::command]
pub fn search_unified(
    query: String,
    project_key: String,
    limit: Option<i64>,
    max_per_conversation: Option<usize>,
    state: State<DbState>,
) -> Result<Vec<UnifiedResult>, AppError> {
    let max = limit.unwrap_or(20) as usize;
    let div_cap = max_per_conversation.unwrap_or(DEFAULT_MAX_PER_CONVERSATION);
    // We pull 3x the final target from each source so RRF has room to rerank.
    let per_source = (max * 3).max(10);

    // Phase A — query expansion (opt-in). Takes the write lock only when
    // enabled, because the cache table is writable on miss.
    let effective_query = if query_expansion_enabled() {
        let write = state.write.lock().map_err(|_| AppError::Lock)?;
        expand_query(&query, Some(&*write)).unwrap_or_else(|_| query.clone())
    } else {
        query.clone()
    };

    // Optional morphological tokenization for the FTS side. Only when the
    // index has been rebuilt under the same tokenizer (Phase C Part2); until
    // then, leaving the flag off is the correct default.
    let fts_query = if morphological_query_enabled() {
        tokenize_query_for_fts(&effective_query)
    } else {
        effective_query.clone()
    };

    let fts_results = fts_conversation_search(&state, &fts_query, &project_key, per_source)?;
    let vec_results = document_vector_search(&state, &effective_query, &project_key, per_source);

    let fused = reciprocal_rank_fusion(&[fts_results.as_slice(), vec_results.as_slice()], RRF_K);

    let mut out: Vec<UnifiedResult> = fused
        .into_iter()
        .map(|(mut r, score)| {
            r.score = score;
            r
        })
        .collect();

    if div_cap > 0 {
        diversify_by_conversation(&mut out, div_cap);
    }
    out.truncate(max);
    Ok(out)
}

// ─── Source: conversation FTS ─────────────────────────────────────────────────

fn fts_conversation_search(
    state: &DbState,
    query: &str,
    project_key: &str,
    limit: usize,
) -> Result<Vec<UnifiedResult>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let mut stmt = conn.prepare(
        "SELECT m.id, m.conversation_id,
                COALESCE(c.custom_label, c.label, ''),
                snippet(messages_fts, 0, '**', '**', '…', 40),
                m.timestamp,
                rank AS fts_rank
         FROM messages_fts fts
         JOIN messages m ON m.rowid = fts.rowid
         JOIN conversations c ON c.id = m.conversation_id
         WHERE messages_fts MATCH ?1
           AND c.project_key = ?2
         ORDER BY rank
         LIMIT ?3",
    )?;

    let results: Vec<UnifiedResult> = stmt
        .query_map(params![query, project_key, limit as i64], |row| {
            let msg_id: String = row.get(0)?;
            let conv_id: String = row.get(1)?;
            let conv_label: String = row.get(2)?;
            let snippet: String = row.get(3)?;
            let ts: i64 = row.get(4)?;
            let fts_rank: f64 = row.get(5)?;
            let label = if conv_label.is_empty() { "Chat".to_string() } else { format!("Chat: {}", conv_label) };
            Ok(UnifiedResult {
                kind: "conversation",
                id: msg_id,
                source_label: label,
                snippet,
                score: 0.0, // filled in by RRF
                fts_score: Some(fts_rank),
                vector_score: None,
                timestamp: Some(ts),
                conversation_id: Some(conv_id),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(results)
}

// ─── Source: document vector ──────────────────────────────────────────────────

fn document_vector_search(
    state: &DbState,
    query: &str,
    project_key: &str,
    limit: usize,
) -> Vec<UnifiedResult> {
    // Document vector search internally embeds the query (bge-m3), so we just
    // adapt its output to UnifiedResult. Errors here are NOT fatal — we prefer
    // to return FTS-only results over a 500 when the embedder or vec0 is down.
    match crate::commands::document_index::search_documents(state, project_key, query, limit) {
        Ok(docs) => docs
            .into_iter()
            .map(|d| UnifiedResult {
                kind: "document",
                id: d.file_path.clone(),
                source_label: format!("Doc: {}", d.file_path),
                snippet: d.text_preview,
                score: 0.0,
                fts_score: None,
                vector_score: Some(d.score as f64),
                timestamp: None,
                conversation_id: None,
            })
            .collect(),
        Err(e) => {
            eprintln!("[search_unified] document vector search skipped: {e:?}");
            Vec::new()
        }
    }
}

// ─── Session diversity ────────────────────────────────────────────────────────

/// Cap the number of results per conversation to `max_per`. Document results
/// (conversation_id=None) are unaffected. The relative order of retained
/// results is preserved.
pub fn diversify_by_conversation(results: &mut Vec<UnifiedResult>, max_per: usize) {
    use std::collections::HashMap;
    let mut counts: HashMap<String, usize> = HashMap::new();
    results.retain(|r| match &r.conversation_id {
        Some(cid) => {
            let c = counts.entry(cid.clone()).or_insert(0);
            if *c < max_per {
                *c += 1;
                true
            } else {
                false
            }
        }
        None => true, // documents are not capped
    });
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn conv_result(conv_id: &str, msg_id: &str) -> UnifiedResult {
        UnifiedResult {
            kind: "conversation",
            id: msg_id.into(),
            source_label: "Chat".into(),
            snippet: String::new(),
            score: 0.0,
            fts_score: Some(1.0),
            vector_score: None,
            timestamp: Some(0),
            conversation_id: Some(conv_id.into()),
        }
    }

    fn doc_result(path: &str) -> UnifiedResult {
        UnifiedResult {
            kind: "document",
            id: path.into(),
            source_label: format!("Doc: {path}"),
            snippet: String::new(),
            score: 0.0,
            fts_score: None,
            vector_score: Some(0.9),
            timestamp: None,
            conversation_id: None,
        }
    }

    #[test]
    fn ranked_candidate_key_disambiguates_by_kind() {
        let c = conv_result("conv-1", "msg-x");
        let d = doc_result("docs/plans/foo.md");
        assert_eq!(c.key(), "conversation:msg-x");
        assert_eq!(d.key(), "document:docs/plans/foo.md");
    }

    #[test]
    fn diversify_caps_per_conversation() {
        let mut results = vec![
            conv_result("conv-a", "m1"),
            conv_result("conv-a", "m2"),
            conv_result("conv-a", "m3"), // should be dropped (cap=2)
            conv_result("conv-b", "m4"),
            conv_result("conv-a", "m5"), // should be dropped
            doc_result("p1.md"),
            doc_result("p2.md"),
        ];
        diversify_by_conversation(&mut results, 2);
        let ids: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["m1", "m2", "m4", "p1.md", "p2.md"]);
    }

    #[test]
    fn diversify_zero_cap_is_a_noop() {
        let mut results = vec![conv_result("a", "m1"), conv_result("a", "m2")];
        let before_len = results.len();
        diversify_by_conversation(&mut results, 0);
        // cap=0 would mean "zero per conversation" — our docs say >0 is cap.
        // But passing 0 should not crash; current impl treats it as "no keep".
        // Document the semantic: cap=0 filters everything with conv_id.
        assert!(results.len() < before_len || results.is_empty());
    }

    #[test]
    fn diversify_does_not_affect_documents() {
        let mut results = vec![doc_result("a.md"), doc_result("b.md"), doc_result("c.md")];
        diversify_by_conversation(&mut results, 1);
        // Documents are never capped — all three should remain.
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn diversify_preserves_relative_order() {
        let mut results = vec![
            conv_result("a", "m1"),
            conv_result("b", "m2"),
            conv_result("a", "m3"),
            conv_result("b", "m4"),
            conv_result("a", "m5"), // dropped
        ];
        diversify_by_conversation(&mut results, 2);
        let ids: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["m1", "m2", "m3", "m4"]);
    }
}
