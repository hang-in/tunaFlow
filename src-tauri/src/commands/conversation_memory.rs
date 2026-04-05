//! Compressed conversation memory — topic-based structured summaries.
//!
//! When a conversation grows beyond the recent window, older messages are
//! compressed into topic-segmented summaries stored in `conversation_memory`.
//! Each topic becomes a separate row, enabling granular retrieval and
//! better long-term memory for multi-phase conversations.

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::migrations::now_epoch_ms;
use crate::errors::AppError;

/// Minimum messages before compression is triggered.
const COMPRESSION_THRESHOLD: i64 = 12;
/// Number of recent messages to keep as working memory (not compressed).
const RECENT_WINDOW: i64 = 6;

/// Topic-segmented summary prompt. Asks the LLM to produce a JSON array.
const SUMMARY_PROMPT: &str = "\
Analyze the following conversation and produce a JSON array of topic-based summaries.

Each element must have these fields:
- \"topic\": short topic label (2-5 words, e.g. \"DB schema design\", \"FTS5 retrieval\")
- \"phase\": one of \"exploration\", \"implementation\", \"review\", \"debugging\", \"planning\", \"discussion\"
- \"summary\": detailed structured summary for this topic (400-800 characters per topic)

The summary for each topic MUST cover:
- What was discussed/decided (specific decisions, not vague references)
- Key findings, results, or code changes (file paths, function names, values)
- Important context that would be lost (architecture decisions, rejected alternatives, constraints)
- Any unresolved issues or next steps

Rules:
- Output ONLY the JSON array, no markdown fences, no explanation.
- Identify 2-7 distinct topics. More topics = better granularity for long conversations.
- Preserve specifics: file paths, function names, numbers, agent names, error messages.
- Include participant information (agent names/engines) in at least the first topic's summary.
- Each topic summary should be 400-800 characters. Short summaries lose critical context.
- Total output should be under 5000 characters.
- Write in the same language the conversation uses.
- For architecture/design decisions, include the reasoning (WHY, not just WHAT).

Example output:
[{\"topic\":\"DB migration v21\",\"phase\":\"implementation\",\"summary\":\"Added topic/phase/provenance columns to conversation_memory table. Session_links table created with (conversation_id, linked_conv_id, score, method) for FTS5-based auto session discovery. Migration tested with cargo test. Decision: used single v21 migration combining all schema changes rather than separate v21/v22. Reason: atomic rollback if any part fails.\"}]

---

";

/// A single topic from compressed memory.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryTopic {
    pub topic: String,
    pub phase: Option<String>,
    pub summary: String,
}

/// Memory status for a conversation.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStatus {
    /// "not_generated" | "fresh" | "stale" | "below_threshold"
    pub state: String,
    pub source_count: Option<i64>,
    pub message_count: i64,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub new_messages_since: i64,
    pub summary_length: Option<usize>,
    pub topic_count: usize,
    pub provenance: Option<String>,
    pub model_used: Option<String>,
}

/// Check if a conversation needs memory compression.
pub fn needs_compression(conn: &Connection, conversation_id: &str) -> bool {
    let msg_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if msg_count <= COMPRESSION_THRESHOLD {
        return false;
    }

    // Check if we already have a recent enough memory
    let existing: Option<i64> = conn
        .query_row(
            "SELECT MAX(source_count) FROM conversation_memory
             WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    match existing {
        Some(prev_count) => {
            // Re-compress if significantly more messages since last compression
            msg_count - prev_count >= COMPRESSION_THRESHOLD / 2
        }
        None => true,
    }
}

/// Load compressed memory topics for a conversation.
pub fn load_compressed_memory_topics(conn: &Connection, conversation_id: &str) -> Vec<MemoryTopic> {
    let mut stmt = match conn.prepare(
        "SELECT topic, phase, summary FROM conversation_memory
         WHERE conversation_id = ?1
         ORDER BY created_at ASC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map([conversation_id], |row| {
        Ok(MemoryTopic {
            topic: row.get(0)?,
            phase: row.get(1)?,
            summary: row.get(2)?,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// Load compressed memory as a single formatted string (backward-compatible).
pub fn load_compressed_memory(conn: &Connection, conversation_id: &str) -> Option<String> {
    let topics = load_compressed_memory_topics(conn, conversation_id);
    if topics.is_empty() {
        return None;
    }
    Some(format_topics_as_section(&topics))
}

/// Format topic list into a readable section for ContextPack injection.
pub fn format_topics_as_section(topics: &[MemoryTopic]) -> String {
    if topics.len() == 1 {
        // Single topic: just the summary (no subsection headers)
        return topics[0].summary.clone();
    }

    let mut out = String::new();
    for t in topics {
        if let Some(ref phase) = t.phase {
            out.push_str(&format!("### {} ({})\n", t.topic, phase));
        } else {
            out.push_str(&format!("### {}\n", t.topic));
        }
        out.push_str(&t.summary);
        out.push_str("\n\n");
    }
    out.trim_end().to_string()
}

/// Get the compressed memory status for a conversation.
pub fn get_memory_status(conn: &Connection, conversation_id: &str) -> MemoryStatus {
    let msg_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Get the most recent topic row for metadata
    let existing: Option<(i64, i64, i64, String, String, Option<String>)> = conn
        .query_row(
            "SELECT source_count, created_at, updated_at, summary, provenance, model_used
             FROM conversation_memory
             WHERE conversation_id = ?1
             ORDER BY updated_at DESC LIMIT 1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .ok();

    let topic_count: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM conversation_memory WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize;

    match existing {
        Some((src_count, created, updated, summary, provenance, model_used)) => {
            let new_since = msg_count - src_count;
            let state = if new_since >= COMPRESSION_THRESHOLD / 2 {
                "stale"
            } else {
                "fresh"
            };
            MemoryStatus {
                state: state.to_string(),
                source_count: Some(src_count),
                message_count: msg_count,
                created_at: Some(created),
                updated_at: Some(updated),
                new_messages_since: new_since.max(0),
                summary_length: Some(summary.len()),
                topic_count,
                provenance: Some(provenance),
                model_used,
            }
        }
        None => {
            let state = if msg_count > COMPRESSION_THRESHOLD {
                "not_generated"
            } else {
                "below_threshold"
            };
            MemoryStatus {
                state: state.to_string(),
                source_count: None,
                message_count: msg_count,
                created_at: None,
                updated_at: None,
                new_messages_since: 0,
                summary_length: None,
                topic_count: 0,
                provenance: None,
                model_used: None,
            }
        }
    }
}

/// Parse LLM output into topic list. Falls back to single "general" topic.
fn parse_topics(raw: &str) -> Vec<MemoryTopic> {
    // Try to extract JSON array from the response
    let trimmed = raw.trim();

    // Try direct parse
    if let Ok(topics) = serde_json::from_str::<Vec<MemoryTopic>>(trimmed) {
        if !topics.is_empty() {
            return topics;
        }
    }

    // Try extracting from markdown code fence
    if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            let json_str = &trimmed[start..=end];
            if let Ok(topics) = serde_json::from_str::<Vec<MemoryTopic>>(json_str) {
                if !topics.is_empty() {
                    return topics;
                }
            }
        }
    }

    // Fallback: treat entire response as a single general topic
    eprintln!("[memory] topic parse failed, falling back to single topic");
    vec![MemoryTopic {
        topic: "general".to_string(),
        phase: None,
        summary: trimmed.to_string(),
    }]
}

/// Pre-pass pruning for compression: reduce token count before LLM summarization.
///
/// L1: Collapse 3+ consecutive blank lines → 1 blank line
/// L2: Code blocks → keep signature (first 3 lines) + `[... N lines pruned]`
///
/// Does NOT remove inline code or short code snippets (< 5 lines).
fn prune_for_summary(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut in_code_block = false;
    let mut code_lines: Vec<&str> = Vec::new();
    let mut consecutive_blank = 0;

    for line in content.lines() {
        // Code fence detection
        if line.trim_start().starts_with("```") {
            if in_code_block {
                // Closing fence: emit pruned code block
                let total = code_lines.len();
                if total <= 5 {
                    // Short block: keep as-is
                    for cl in &code_lines {
                        result.push_str(cl);
                        result.push('\n');
                    }
                } else {
                    // Keep first 3 lines (signature), prune rest
                    for cl in &code_lines[..3] {
                        result.push_str(cl);
                        result.push('\n');
                    }
                    result.push_str(&format!("[... {} lines pruned]\n", total - 3));
                }
                result.push_str("```\n");
                code_lines.clear();
                in_code_block = false;
                consecutive_blank = 0;
            } else {
                // Opening fence
                in_code_block = true;
                result.push_str(line);
                result.push('\n');
                consecutive_blank = 0;
            }
            continue;
        }

        if in_code_block {
            code_lines.push(line);
            continue;
        }

        // L1: Collapse consecutive blank lines
        if line.trim().is_empty() {
            consecutive_blank += 1;
            if consecutive_blank <= 1 {
                result.push('\n');
            }
            continue;
        }
        consecutive_blank = 0;
        result.push_str(line);
        result.push('\n');
    }

    // Handle unclosed code block
    if in_code_block && !code_lines.is_empty() {
        let total = code_lines.len();
        if total <= 5 {
            for cl in &code_lines {
                result.push_str(cl);
                result.push('\n');
            }
        } else {
            for cl in &code_lines[..3] {
                result.push_str(cl);
                result.push('\n');
            }
            result.push_str(&format!("[... {} lines pruned]\n", total - 3));
        }
    }

    result
}

/// Build transcript from older messages for compression.
fn build_transcript(conn: &Connection, conversation_id: &str) -> Result<(String, i64), AppError> {
    let total: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if total <= RECENT_WINDOW {
        return Ok((String::new(), total));
    }

    let older_count = total - RECENT_WINDOW;
    let mut stmt = conn.prepare(
        "SELECT role, content, engine, persona FROM messages
         WHERE conversation_id = ?1
         ORDER BY timestamp ASC
         LIMIT ?2",
    )?;
    let rows: Vec<(String, String, Option<String>, Option<String>)> = stmt
        .query_map(params![conversation_id, older_count], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    if rows.is_empty() {
        return Ok((String::new(), total));
    }

    let mut transcript = String::new();
    for (role, content, engine, persona) in &rows {
        let author = match (role.as_str(), persona, engine) {
            ("assistant", Some(p), Some(e)) if !p.is_empty() => format!("{}:{} ({})", role, p, e),
            ("assistant", None, Some(e)) if !e.is_empty() => format!("{} ({})", role, e),
            _ => role.clone(),
        };
        // Pre-pass: prune code blocks and collapse blank lines before truncation
        let pruned = prune_for_summary(content);
        let content_preview = if pruned.len() > 1500 {
            format!(
                "{}…",
                &pruned[..pruned
                    .char_indices()
                    .take_while(|&(i, _)| i <= 1500)
                    .last()
                    .map_or(0, |(i, _)| i)]
            )
        } else {
            content.clone()
        };
        transcript.push_str(&format!("[{}] {}\n\n", author, content_preview));
    }

    Ok((transcript, total))
}

/// Write topic rows to DB, replacing existing ones for this conversation.
fn write_topics(
    conn: &Connection,
    conversation_id: &str,
    topics: &[MemoryTopic],
    total: i64,
    provenance: &str,
    model_used: &str,
) -> Result<(), AppError> {
    let now = now_epoch_ms();
    conn.execute(
        "DELETE FROM conversation_memory WHERE conversation_id = ?1",
        [conversation_id],
    )?;
    for t in topics {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO conversation_memory (id, conversation_id, summary, source_count, created_at, updated_at, topic, phase, provenance, model_used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?8, ?9)",
            params![id, conversation_id, t.summary, total, now, t.topic, t.phase, provenance, model_used],
        )?;
    }
    Ok(())
}

/// Tauri command: get memory status for a conversation.
#[tauri::command]
pub fn get_conversation_memory_status(
    conversation_id: String,
    state: tauri::State<crate::db::DbState>,
) -> Result<MemoryStatus, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    Ok(get_memory_status(&conn, &conversation_id))
}

/// Tauri command: trigger memory compression for a conversation.
///
/// Lock strategy: read data with short lock → release → call Claude (slow) → re-lock to write.
/// Runs on a background thread via spawn_blocking to avoid blocking the main thread.
#[tauri::command]
pub async fn compress_conversation_memory(
    conversation_id: String,
    state: tauri::State<'_, crate::db::DbState>,
) -> Result<bool, AppError> {
    let db = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        // Phase 1: check + gather data (short lock)
        let transcript = {
            let conn = db.write.lock().map_err(|_| AppError::Lock)?;
            if !needs_compression(&conn, &conversation_id) {
                return Ok(false);
            }
            let (t, _) = build_transcript(&conn, &conversation_id)?;
            if t.is_empty() {
                return Ok(false);
            }
            t
            // Lock released here
        };

        // Phase 2: call Claude WITHOUT holding any lock
        let prompt = format!("{}{}", SUMMARY_PROMPT, transcript);
        let result = crate::agents::claude::run(crate::agents::claude::RunInput {
            prompt,
            model: None,
            system_prompt: None,
            resume_token: None,
            project_path: None,
        });

        let raw_output = match result {
            Ok(out) if !out.content.trim().is_empty() => out.content.trim().to_string(),
            _ => {
                eprintln!("[memory] compression failed for {}", conversation_id);
                return Ok(false);
            }
        };

        let topics = parse_topics(&raw_output);

        // Phase 3: write result (short lock)
        {
            let conn = db.write.lock().map_err(|_| AppError::Lock)?;
            let total: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
                    [&conversation_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            write_topics(&conn, &conversation_id, &topics, total, "auto", "claude")?;
            eprintln!(
                "[memory] compressed → {} topics ({} chars) for {}",
                topics.len(),
                topics.iter().map(|t| t.summary.len()).sum::<usize>(),
                conversation_id
            );
        }

        Ok(true)
    }).await.map_err(|_| AppError::Lock)?
}

/// Tauri command: force recompress memory (bypasses threshold check).
#[tauri::command]
pub async fn force_recompress_memory(
    conversation_id: String,
    state: tauri::State<'_, crate::db::DbState>,
) -> Result<bool, AppError> {
    let db = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        // Phase 1: gather data (short lock)
        let transcript = {
            let conn = db.write.lock().map_err(|_| AppError::Lock)?;
            let (t, _) = build_transcript(&conn, &conversation_id)?;
            if t.is_empty() {
                return Ok(false);
            }
            t
        };

        // Phase 2: call Claude WITHOUT holding any lock
        let prompt = format!("{}{}", SUMMARY_PROMPT, transcript);
        let result = crate::agents::claude::run(crate::agents::claude::RunInput {
            prompt,
            model: None,
            system_prompt: None,
            resume_token: None,
            project_path: None,
        });

        let raw_output = match result {
            Ok(out) if !out.content.trim().is_empty() => out.content.trim().to_string(),
            _ => {
                eprintln!("[memory] force recompress failed for {}", conversation_id);
                return Ok(false);
            }
        };

        let topics = parse_topics(&raw_output);

        // Phase 3: write result (short lock)
        {
            let conn = db.write.lock().map_err(|_| AppError::Lock)?;
            let total: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
                    [&conversation_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            write_topics(&conn, &conversation_id, &topics, total, "manual", "claude")?;
            eprintln!(
                "[memory] force recompressed → {} topics for {}",
                topics.len(),
                conversation_id
            );
        }

        Ok(true)
    }).await.map_err(|_| AppError::Lock)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_json_array() {
        let raw = r#"[{"topic":"DB design","phase":"implementation","summary":"Created tables."},{"topic":"API layer","phase":"exploration","summary":"Discussed endpoints."}]"#;
        let topics = parse_topics(raw);
        assert_eq!(topics.len(), 2);
        assert_eq!(topics[0].topic, "DB design");
        assert_eq!(topics[0].phase, Some("implementation".to_string()));
        assert_eq!(topics[1].topic, "API layer");
    }

    #[test]
    fn parse_json_with_code_fence() {
        let raw = "```json\n[{\"topic\":\"test\",\"phase\":\"review\",\"summary\":\"All tests pass.\"}]\n```";
        let topics = parse_topics(raw);
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].topic, "test");
    }

    #[test]
    fn parse_invalid_json_fallback() {
        let raw = "This is not JSON at all. Just a plain summary of the conversation.";
        let topics = parse_topics(raw);
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].topic, "general");
        assert_eq!(topics[0].phase, None);
        assert!(topics[0].summary.contains("plain summary"));
    }

    #[test]
    fn parse_empty_array_fallback() {
        let raw = "[]";
        let topics = parse_topics(raw);
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].topic, "general");
    }

    #[test]
    fn format_single_topic() {
        let topics = vec![MemoryTopic {
            topic: "general".to_string(),
            phase: None,
            summary: "A simple summary.".to_string(),
        }];
        let out = format_topics_as_section(&topics);
        assert_eq!(out, "A simple summary.");
    }

    #[test]
    fn prune_collapses_blank_lines() {
        let input = "line1\n\n\n\n\nline2\n\n\nline3";
        let result = prune_for_summary(input);
        assert_eq!(result, "line1\n\nline2\n\nline3\n");
    }

    #[test]
    fn prune_short_code_block_kept() {
        let input = "text\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\nmore";
        let result = prune_for_summary(input);
        assert!(result.contains("fn main()"));
        assert!(result.contains("println!"));
        assert!(!result.contains("pruned"));
    }

    #[test]
    fn prune_long_code_block_truncated() {
        let mut input = String::from("before\n```rust\nfn big() {\n");
        for i in 0..20 {
            input.push_str(&format!("    let x{} = {};\n", i, i));
        }
        input.push_str("}\n```\nafter");
        let result = prune_for_summary(&input);
        // Should keep first 3 lines of code + pruned marker
        assert!(result.contains("fn big()"));
        assert!(result.contains("[... "));
        assert!(result.contains(" lines pruned]"));
        assert!(result.contains("after"));
    }

    #[test]
    fn prune_no_code_passthrough() {
        let input = "just plain text\nwith some lines\nno code blocks";
        let result = prune_for_summary(input);
        assert_eq!(result, "just plain text\nwith some lines\nno code blocks\n");
    }

    #[test]
    fn format_multiple_topics() {
        let topics = vec![
            MemoryTopic {
                topic: "DB design".to_string(),
                phase: Some("implementation".to_string()),
                summary: "Created tables.".to_string(),
            },
            MemoryTopic {
                topic: "API".to_string(),
                phase: None,
                summary: "Discussed endpoints.".to_string(),
            },
        ];
        let out = format_topics_as_section(&topics);
        assert!(out.contains("### DB design (implementation)"));
        assert!(out.contains("### API\n"));
        assert!(out.contains("Created tables."));
        assert!(out.contains("Discussed endpoints."));
    }

    // ─── parse_topics edge cases ─────────────────────────────────────────

    #[test]
    fn parse_topics_with_extra_text_before_json() {
        let raw = "Here are the topics:\n[{\"topic\":\"auth\",\"summary\":\"JWT discussion\"}]";
        let topics = parse_topics(raw);
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].topic, "auth");
    }

    #[test]
    fn parse_topics_optional_phase() {
        let raw = r#"[{"topic":"test","summary":"All tests pass."}]"#;
        let topics = parse_topics(raw);
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].phase, None);
    }

    #[test]
    fn parse_topics_whitespace_padded() {
        let raw = "  \n  [{\"topic\":\"x\",\"summary\":\"y\"}]  \n  ";
        let topics = parse_topics(raw);
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].topic, "x");
    }

    #[test]
    fn parse_topics_malformed_json_object() {
        // JSON object instead of array → fallback
        let raw = r#"{"topic":"single","summary":"oops"}"#;
        let topics = parse_topics(raw);
        assert_eq!(topics.len(), 1);
        assert_eq!(topics[0].topic, "general");
    }

    // ─── prune_for_summary edge cases ────────────────────────────────────

    #[test]
    fn prune_unclosed_code_block_short() {
        let input = "text\n```rust\nfn x() {}";
        let result = prune_for_summary(input);
        assert!(result.contains("fn x()"));
        assert!(!result.contains("pruned"));
    }

    #[test]
    fn prune_unclosed_code_block_long() {
        let mut input = String::from("```\nline1\n");
        for i in 0..10 {
            input.push_str(&format!("line{}\n", i + 2));
        }
        // No closing ``` — unclosed block
        let result = prune_for_summary(&input);
        assert!(result.contains("[... "));
        assert!(result.contains("lines pruned]"));
    }

    #[test]
    fn prune_exactly_five_line_code_block_kept() {
        let input = "```\n1\n2\n3\n4\n5\n```\nafter";
        let result = prune_for_summary(input);
        assert!(!result.contains("pruned"));
        assert!(result.contains("5"));
    }

    #[test]
    fn prune_six_line_code_block_truncated() {
        let input = "```\n1\n2\n3\n4\n5\n6\n```\nafter";
        let result = prune_for_summary(input);
        assert!(result.contains("[... 3 lines pruned]")); // 6 lines - 3 kept = 3
    }

    #[test]
    fn prune_empty_input() {
        let result = prune_for_summary("");
        assert!(result.is_empty());
    }

    // ─── format_topics_as_section edge cases ─────────────────────────────

    #[test]
    fn format_empty_topics() {
        let topics: Vec<MemoryTopic> = vec![];
        let out = format_topics_as_section(&topics);
        assert!(out.is_empty());
    }

    #[test]
    fn format_single_topic_with_phase_returns_summary_only() {
        // Single topic: format_topics_as_section returns only the summary (no headers)
        let topics = vec![MemoryTopic {
            topic: "Auth".to_string(),
            phase: Some("review".to_string()),
            summary: "Reviewed auth flow.".to_string(),
        }];
        let out = format_topics_as_section(&topics);
        assert_eq!(out, "Reviewed auth flow.");
    }

    #[test]
    fn format_multiple_topics_includes_phase_headers() {
        // Multiple topics: each gets a ### header with phase
        let topics = vec![
            MemoryTopic { topic: "Auth".to_string(), phase: Some("review".to_string()), summary: "Reviewed.".to_string() },
            MemoryTopic { topic: "DB".to_string(), phase: None, summary: "Migrated.".to_string() },
        ];
        let out = format_topics_as_section(&topics);
        assert!(out.contains("### Auth (review)"));
        assert!(out.contains("### DB\n"));
    }
}
