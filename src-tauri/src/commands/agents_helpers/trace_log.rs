use rusqlite::params;
use uuid::Uuid;

/// OTel-style span metadata for trace_log insertion.
pub struct SpanInfo<'a> {
    pub trace_id: &'a str,
    pub span_id: String,
    pub parent_span_id: Option<&'a str>,
    pub operation: &'a str,
    pub engine: &'a str,
    pub duration_ms: i64,
    pub status: &'a str,
}

/// ContextPack metadata — lightweight record of what went into the system prompt.
/// Full prompt body is NOT stored; only mode/sections/length/hash for traceability.
#[derive(Debug, Clone, Default)]
pub struct ContextPackMeta {
    pub mode: String,
    pub sections: Vec<String>,
    pub length: usize,
    pub hash: String,
    pub truncated: bool,
    /// Cache read tokens (prompt cache hits — discounted rate)
    pub cache_read_tokens: i64,
    /// Cache creation tokens (new cache entries — premium rate)
    pub cache_creation_tokens: i64,
}

impl ContextPackMeta {
    /// Build metadata from assembled prompt parts.
    #[allow(dead_code)]
    pub fn from_parts(
        mode: &str,
        section_flags: &[(&str, bool)],
        prompt: &Option<String>,
        was_truncated: bool,
    ) -> Self {
        let sections: Vec<String> = section_flags.iter()
            .filter(|(_, present)| *present)
            .map(|(name, _)| name.to_string())
            .collect();
        let length = prompt.as_ref().map_or(0, |s| s.len());
        let hash = prompt.as_ref().map_or_else(String::new, |s| {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut h = DefaultHasher::new();
            s.hash(&mut h);
            format!("{:016x}", h.finish())
        });
        Self { mode: mode.to_string(), sections, length, hash, truncated: was_truncated, cache_read_tokens: 0, cache_creation_tokens: 0 }
    }

    /// JSON-encoded sections list for DB storage.
    pub fn sections_json(&self) -> String {
        serde_json::to_string(&self.sections).unwrap_or_else(|_| "[]".to_string())
    }
}

/// Generate a new random span id (UUID v4 hex, no dashes, 32 chars).
pub fn new_span_id() -> String {
    Uuid::new_v4().simple().to_string()
}

/// Generate a new trace id (same format as span_id).
pub fn new_trace_id() -> String {
    Uuid::new_v4().simple().to_string()
}

/// Insert a trace_log record with full OTel-style metadata.
/// Errors are silently swallowed so a logging failure never breaks the caller.
/// Determine usage_status based on engine and token values.
pub fn resolve_usage_status(engine: &str, input_tokens: i64, output_tokens: i64) -> &'static str {
    match engine {
        "opencode" => "unavailable",
        "gemini" if input_tokens == 0 && output_tokens == 0 => "unavailable",
        _ if input_tokens == 0 && output_tokens == 0 => "unknown",
        _ => "exact",
    }
}

pub fn insert_trace_log(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    input_tokens: i64,
    output_tokens: i64,
    cost_usd: f64,
    recorded_at: i64,
    span: &SpanInfo,
) {
    let usage_status = resolve_usage_status(span.engine, input_tokens, output_tokens);
    let _ = conn.execute(
        "INSERT INTO trace_log
         (conversation_id, input_tokens, output_tokens, cost_usd, recorded_at,
          trace_id, span_id, parent_span_id, operation, engine, duration_ms, status, usage_status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            conversation_id,
            input_tokens,
            output_tokens,
            cost_usd,
            recorded_at,
            span.trace_id,
            span.span_id,
            span.parent_span_id,
            span.operation,
            span.engine,
            span.duration_ms,
            span.status,
            usage_status,
        ],
    );
}

/// Insert trace_log with ContextPack metadata + optional message_id linkage.
pub fn insert_trace_log_with_context(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    input_tokens: i64,
    output_tokens: i64,
    cost_usd: f64,
    recorded_at: i64,
    span: &SpanInfo,
    ctx: &ContextPackMeta,
    message_id: Option<&str>,
) {
    let usage_status = resolve_usage_status(span.engine, input_tokens, output_tokens);
    let _ = conn.execute(
        "INSERT INTO trace_log
         (conversation_id, input_tokens, output_tokens, cost_usd, recorded_at,
          trace_id, span_id, parent_span_id, operation, engine, duration_ms, status,
          context_mode, context_sections, context_length, context_hash, context_truncated,
          usage_status, message_id, cache_read_tokens, cache_creation_tokens)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
        params![
            conversation_id, input_tokens, output_tokens, cost_usd, recorded_at,
            span.trace_id, span.span_id, span.parent_span_id,
            span.operation, span.engine, span.duration_ms, span.status,
            ctx.mode, ctx.sections_json(), ctx.length as i64, ctx.hash,
            if ctx.truncated { 1 } else { 0 },
            usage_status, message_id,
            ctx.cache_read_tokens, ctx.cache_creation_tokens,
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_id_format() {
        let id = new_trace_id();
        assert_eq!(id.len(), 32); // UUID simple format = 32 hex chars
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn span_id_format() {
        let id = new_span_id();
        assert_eq!(id.len(), 32);
    }

    #[test]
    fn trace_and_span_are_unique() {
        let a = new_trace_id();
        let b = new_trace_id();
        assert_ne!(a, b);
    }
}
