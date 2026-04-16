//! Trace export commands — read trace_log spans and produce OTel-compatible JSON.
//!
//! The trace_log table stores per-invocation span records. After migration v6,
//! each row can carry OTel-style fields: trace_id, span_id, parent_span_id,
//! operation, engine, duration_ms, status.
//!
//! `export_traces_otel` returns a JSON array of span objects in a format
//! compatible with OTLP JSON (simplified — no full resource/scope nesting).

use rusqlite::params;
use serde::Serialize;
use tauri::State;

use crate::db::DbState;
use crate::errors::AppError;

/// Simplified OTel-compatible span representation.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceSpan {
    pub id: i64,
    pub conversation_id: String,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub parent_span_id: Option<String>,
    pub operation: Option<String>,
    pub engine: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
    pub duration_ms: Option<i64>,
    pub status: Option<String>,
    pub recorded_at: i64,
    // ContextPack traceability (v11)
    pub context_mode: Option<String>,
    pub context_sections: Option<String>,
    pub context_length: Option<i64>,
    pub context_hash: Option<String>,
    pub context_truncated: Option<i64>,
    // Message linkage (v23)
    pub message_id: Option<String>,
    // Cache token classification (v35)
    pub cache_read_tokens: Option<i64>,
    pub cache_creation_tokens: Option<i64>,
}

/// List trace spans for a conversation, ordered by recorded_at descending.
/// Optionally filtered by trace_id.
#[tauri::command]
pub fn list_traces(
    conversation_id: String,
    trace_id: Option<String>,
    state: State<DbState>,
) -> Result<Vec<TraceSpan>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;

    let (sql, arg_trace) = if let Some(ref tid) = trace_id {
        (
            "SELECT id, conversation_id, trace_id, span_id, parent_span_id, operation, engine,
                    input_tokens, output_tokens, cost_usd, duration_ms, status, recorded_at,
                    context_mode, context_sections, context_length, context_hash, context_truncated,
                    message_id, cache_read_tokens, cache_creation_tokens
             FROM trace_log
             WHERE conversation_id = ?1 AND trace_id = ?2
             ORDER BY recorded_at DESC",
            Some(tid.clone()),
        )
    } else {
        (
            "SELECT id, conversation_id, trace_id, span_id, parent_span_id, operation, engine,
                    input_tokens, output_tokens, cost_usd, duration_ms, status, recorded_at,
                    context_mode, context_sections, context_length, context_hash, context_truncated,
                    message_id, cache_read_tokens, cache_creation_tokens
             FROM trace_log
             WHERE conversation_id = ?1
             ORDER BY recorded_at DESC",
            None,
        )
    };

    let mut stmt = conn.prepare(sql)?;
    let rows: Vec<TraceSpan> = if let Some(tid) = arg_trace {
        stmt.query_map(params![conversation_id, tid], map_span)?
            .filter_map(|r| r.ok())
            .collect()
    } else {
        stmt.query_map(params![conversation_id], map_span)?
            .filter_map(|r| r.ok())
            .collect()
    };

    Ok(rows)
}

/// Export traces for a conversation as OTel-compatible JSON string.
/// Returns a JSON array of span objects.
#[tauri::command]
pub fn export_traces_otel(
    conversation_id: String,
    state: State<DbState>,
) -> Result<String, AppError> {
    let spans = list_traces(conversation_id, None, state)?;
    let json = serde_json::to_string_pretty(&spans)
        .map_err(|e| AppError::Agent(format!("JSON serialization failed: {}", e)))?;
    Ok(json)
}

fn map_span(row: &rusqlite::Row) -> rusqlite::Result<TraceSpan> {
    Ok(TraceSpan {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        trace_id: row.get(2)?,
        span_id: row.get(3)?,
        parent_span_id: row.get(4)?,
        operation: row.get(5)?,
        engine: row.get(6)?,
        input_tokens: row.get(7)?,
        output_tokens: row.get(8)?,
        cost_usd: row.get(9)?,
        duration_ms: row.get(10)?,
        status: row.get(11)?,
        recorded_at: row.get(12)?,
        context_mode: row.get(13)?,
        context_sections: row.get(14)?,
        context_length: row.get(15)?,
        context_hash: row.get(16)?,
        context_truncated: row.get(17)?,
        message_id: row.get(18)?,
        cache_read_tokens: row.get(19).ok(),
        cache_creation_tokens: row.get(20).ok(),
    })
}
