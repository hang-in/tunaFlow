//! Compressed conversation memory — Tauri commands + status queries.
//!
//! Topic types, loading, and formatting → `memory_topics`
//! Compression pipeline (prune, LLM call, write) → `memory_compression`

use rusqlite::Connection;

use crate::errors::AppError;

use super::memory_compression::{
    needs_compression, build_transcript, write_topics, parse_topics,
    SUMMARY_PROMPT, COMPRESSION_THRESHOLD,
};
use super::memory_topics::{MemoryTopic, load_compressed_memory_topics};

// ─── Re-exports (public API preserved) ───────────────────────────────────────

pub use super::memory_topics::{load_compressed_memory, format_topics_as_section};
pub use super::memory_compression::{compress_memory_blocking, prune_tool_results};

// ─── MemoryStatus ─────────────────────────────────────────────────────────────

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

// ─── Tauri commands ───────────────────────────────────────────────────────────

/// Tauri command: get memory status for a conversation.
#[tauri::command]
pub fn get_conversation_memory_status(
    conversation_id: String,
    state: tauri::State<crate::db::DbState>,
) -> Result<MemoryStatus, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    Ok(get_memory_status(&conn, &conversation_id))
}

/// Tauri command: list memory topics for a conversation (for Tier 2 Pull).
#[tauri::command]
pub fn list_memory_topics(
    conversation_id: String,
    state: tauri::State<crate::db::DbState>,
) -> Result<Vec<MemoryTopic>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    Ok(load_compressed_memory_topics(&conn, &conversation_id))
}

/// Tauri command: 현재 conversation 의 최근 user/assistant turn 을 **전문** 반환.
/// tool-request:recent_turns 경로에서 사용. memory (요약본) / conversation_chunks (현재
/// conv 제외) 가 커버하지 못하는 "직전 turn 단기 공백" 을 메우는 도구.
///
/// - `N` 은 1..=10 으로 clamp (너무 큰 값은 context bloat)
/// - system 메시지 제외 (tool 결과 등은 잡음)
/// - content 전문 반환. 각 메시지 최대 4000자까지 (그 이상은 tail 자름)
/// - `[role:persona (engine)]` 라벨 포함
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTurn {
    pub role: String,
    pub persona: Option<String>,
    pub engine: Option<String>,
    pub content: String,
    pub timestamp: i64,
}

const RECENT_TURN_MAX_CHARS: usize = 4_000;

#[tauri::command]
pub fn list_recent_turns(
    conversation_id: String,
    n: Option<i64>,
    state: tauri::State<crate::db::DbState>,
) -> Result<Vec<RecentTurn>, AppError> {
    let limit = n.unwrap_or(3).clamp(1, 10);
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let mut stmt = conn.prepare(
        "SELECT role, persona, engine, content, timestamp FROM messages
         WHERE conversation_id = ?1 AND role IN ('user','assistant') AND status = 'done'
         ORDER BY timestamp DESC LIMIT ?2",
    )?;
    let rows: Vec<RecentTurn> = stmt
        .query_map(rusqlite::params![conversation_id, limit], |r| {
            let role: String = r.get(0)?;
            let persona: Option<String> = r.get(1)?;
            let engine: Option<String> = r.get(2)?;
            let raw: String = r.get(3)?;
            let ts: i64 = r.get(4)?;
            let content = if raw.chars().count() > RECENT_TURN_MAX_CHARS {
                let head: String = raw.chars().take(RECENT_TURN_MAX_CHARS).collect();
                format!("{head}\n…(tail truncated)")
            } else { raw };
            Ok(RecentTurn { role, persona, engine, content, timestamp: ts })
        })?
        .filter_map(|r| r.ok())
        .collect();
    // 오래된 것 → 최신 순으로 뒤집어 반환 (대화 읽기 순서)
    let mut out = rows;
    out.reverse();
    Ok(out)
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
            image_paths: Vec::new(),
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
            image_paths: Vec::new(),
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

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::commands::memory_topics::{MemoryTopic, format_topics_as_section};
    use crate::commands::memory_compression::{parse_topics, prune_tool_results};

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

    // ─── format_topics_as_section edge cases ─────────────────────────────

    #[test]
    fn format_empty_topics() {
        let topics: Vec<MemoryTopic> = vec![];
        let out = format_topics_as_section(&topics);
        assert!(out.is_empty());
    }

    #[test]
    fn format_single_topic_with_phase_returns_summary_only() {
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
        let topics = vec![
            MemoryTopic { topic: "Auth".to_string(), phase: Some("review".to_string()), summary: "Reviewed.".to_string() },
            MemoryTopic { topic: "DB".to_string(), phase: None, summary: "Migrated.".to_string() },
        ];
        let out = format_topics_as_section(&topics);
        assert!(out.contains("### Auth (review)"));
        assert!(out.contains("### DB\n"));
    }

    // ─── prune_tool_results ──────────────────────────────────────────────

    #[test]
    fn tool_prune_rawq_section() {
        let input = "Some context.\n## Code context\nfile.rs:10 fn foo() {\nfile.rs:11   bar()\nfile.rs:12 }\n## Next section\nMore text.";
        let result = prune_tool_results(input);
        assert!(result.contains("[rawq results cleared]"));
        assert!(!result.contains("fn foo()"));
        assert!(result.contains("Next section"));
        assert!(result.contains("More text"));
    }

    #[test]
    fn tool_prune_graph_impact() {
        let input = "Before.\n## Impact analysis\nimpacted: a.rs, b.rs\ncallers: 5\n## Other\nAfter.";
        let result = prune_tool_results(input);
        assert!(result.contains("[graph impact cleared]"));
        assert!(!result.contains("impacted"));
        assert!(result.contains("After"));
    }

    #[test]
    fn tool_prune_preserves_normal_content() {
        let input = "Hello world.\nThis is normal text.\n## My heading\nMore text.";
        let result = prune_tool_results(input);
        assert_eq!(result.trim(), input.trim());
    }

    #[test]
    fn tool_prune_long_test_output() {
        let mut input = String::from("Test results:\n```\n");
        for i in 0..30 {
            input.push_str(&format!("  line {}\n", i));
        }
        input.push_str("```\nAfter test.");
        let result = prune_tool_results(&input);
        assert!(result.contains("[test output cleared — 30 lines]"));
        assert!(result.contains("After test"));
        assert!(!result.contains("line 15"));
    }

    #[test]
    fn tool_prune_short_test_output_kept() {
        let input = "Test results:\n```\nPASS 3/3\n```\nDone.";
        let result = prune_tool_results(input);
        assert!(result.contains("[test output — 1 lines]"));
        assert!(result.contains("Done"));
    }
}
