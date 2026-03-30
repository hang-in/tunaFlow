//! Compressed conversation memory — structured summaries of older messages.
//!
//! When a conversation grows beyond the recent window, older messages are
//! compressed into a structured summary stored in `conversation_memory`.
//! This summary is injected into ContextPack as a separate section,
//! providing continuity without expanding the recent window.

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::migrations::now_epoch_ms;
use crate::errors::AppError;

/// Minimum messages before compression is triggered.
const COMPRESSION_THRESHOLD: i64 = 12;
/// Number of recent messages to keep as working memory (not compressed).
const RECENT_WINDOW: i64 = 6;

/// Structured summary format for compressed memory.
const SUMMARY_PROMPT: &str = "\
Summarize the following conversation into a structured memory document.
Use these exact sections:

## Participants
Which agents (by profile name and engine) participated? List each with a one-line summary of their contribution.

## Task Overview
What the user is working on and the main goal.

## Current State
Where things stand right now.

## Important Discoveries
Key findings, results, or learnings from the conversation.

## Decisions
Choices that were made and their rationale.

## Open Questions
Unresolved issues or pending items.

## Context to Preserve
Any specific details, names, values, or constraints that must not be lost.

Rules:
- Be concise but preserve specifics (names, numbers, file paths, agent names).
- Each section should be 1-3 bullet points max.
- Total summary should be under 2000 characters.
- The Participants section is mandatory — never omit agent names from the summary.
- Write in the same language the conversation uses.

---

";

/// Check if a conversation needs memory compression.
/// Returns true if total messages exceed threshold and no recent memory exists.
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
    let existing: Option<(i64, i64)> = conn
        .query_row(
            "SELECT source_count, updated_at FROM conversation_memory
             WHERE conversation_id = ?1
             ORDER BY updated_at DESC LIMIT 1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    match existing {
        Some((prev_count, _)) => {
            // Re-compress if significantly more messages since last compression
            msg_count - prev_count >= COMPRESSION_THRESHOLD / 2
        }
        None => true,
    }
}

/// Load the most recent compressed memory for a conversation.
pub fn load_compressed_memory(conn: &Connection, conversation_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT summary FROM conversation_memory
         WHERE conversation_id = ?1
         ORDER BY updated_at DESC LIMIT 1",
        [conversation_id],
        |row| row.get(0),
    )
    .ok()
}

/// Memory status for a conversation.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStatus {
    /// "not_generated" | "fresh" | "stale" | "failed"
    pub state: String,
    pub source_count: Option<i64>,
    pub message_count: i64,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub new_messages_since: i64,
    pub summary_length: Option<usize>,
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

    let existing: Option<(i64, i64, i64, String)> = conn
        .query_row(
            "SELECT source_count, created_at, updated_at, summary FROM conversation_memory
             WHERE conversation_id = ?1
             ORDER BY updated_at DESC LIMIT 1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .ok();

    match existing {
        Some((src_count, created, updated, summary)) => {
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
            }
        }
        None => {
            let state = if msg_count > COMPRESSION_THRESHOLD {
                "not_generated"   // threshold exceeded but never compressed
            } else {
                "below_threshold" // not enough messages yet
            };
            MemoryStatus {
                state: state.to_string(),
                source_count: None,
                message_count: msg_count,
                created_at: None,
                updated_at: None,
                new_messages_since: 0,
                summary_length: None,
            }
        }
    }
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
/// This prevents blocking the entire app during the Claude API call.
#[tauri::command]
pub fn compress_conversation_memory(
    conversation_id: String,
    state: tauri::State<crate::db::DbState>,
) -> Result<bool, AppError> {
    // Phase 1: check + gather data (short lock)
    let (transcript, total) = {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;
        if !needs_compression(&conn, &conversation_id) {
            return Ok(false);
        }

        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
                [&conversation_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if total <= RECENT_WINDOW {
            return Ok(false);
        }

        let older_count = total - RECENT_WINDOW;
        let mut stmt = conn.prepare(
            "SELECT role, content, engine, persona FROM messages
             WHERE conversation_id = ?1
             ORDER BY timestamp ASC
             LIMIT ?2",
        )?;
        let rows: Vec<(String, String, Option<String>, Option<String>)> = stmt
            .query_map(params![&conversation_id, older_count], |row| {
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
            return Ok(false);
        }

        let mut transcript = String::new();
        for (role, content, engine, persona) in &rows {
            let author = match (role.as_str(), persona, engine) {
                ("assistant", Some(p), Some(e)) if !p.is_empty() => format!("{}:{} ({})", role, p, e),
                ("assistant", None, Some(e)) if !e.is_empty() => format!("{} ({})", role, e),
                _ => role.clone(),
            };
            let content_preview = if content.len() > 1500 {
                format!("{}…", &content[..content.char_indices().take_while(|&(i, _)| i <= 1500).last().map_or(0, |(i, _)| i)])
            } else {
                content.clone()
            };
            transcript.push_str(&format!("[{}] {}\n\n", author, content_preview));
        }
        (transcript, total)
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

    let summary = match result {
        Ok(out) if !out.content.trim().is_empty() => out.content.trim().to_string(),
        _ => {
            eprintln!("[memory] compression failed for {}", conversation_id);
            return Ok(false);
        }
    };

    // Phase 3: write result (short lock)
    {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;
        let id = Uuid::new_v4().to_string();
        let now = now_epoch_ms();
        conn.execute(
            "DELETE FROM conversation_memory WHERE conversation_id = ?1",
            [&conversation_id],
        )?;
        conn.execute(
            "INSERT INTO conversation_memory (id, conversation_id, summary, source_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, conversation_id, summary, total, now],
        )?;
        eprintln!(
            "[memory] compressed → {} chars for {}",
            summary.len(),
            conversation_id
        );
    }

    Ok(true)
}
