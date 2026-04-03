use rusqlite::params;
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use crate::db::{migrations::now_epoch_ms, models::Message, DbState};
use crate::errors::AppError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserMessageInput {
    pub conversation_id: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendAssistantMessageInput {
    pub conversation_id: String,
    pub content: String,
    pub status: Option<String>,
    pub engine: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMessageStatusInput {
    pub message_id: String,
    pub status: String,
    /// If provided, also update the message content (e.g. finalise streaming content)
    pub content: Option<String>,
}

/// Map a row WITHOUT full progress_content (lightweight, for list_messages).
/// Sets progress_content to a marker "1" if data exists, None otherwise.
/// Frontend uses this to show "has thinking" indicator; actual content loaded via get_progress_content.
fn map_row_light(row: &rusqlite::Row) -> rusqlite::Result<Message> {
    let has_progress: bool = row.get(6)?;
    Ok(Message {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        timestamp: row.get(4)?,
        status: row.get(5)?,
        progress_content: if has_progress { Some("…".into()) } else { None },
        engine: row.get(7)?,
        model: row.get(8)?,
        persona: row.get(9)?,
        duration_ms: row.get(10)?,
        input_tokens: row.get(11)?,
        output_tokens: row.get(12)?,
        cost_usd: row.get(13)?,
    })
}

#[tauri::command]
pub fn list_messages(
    conversation_id: String,
    state: State<DbState>,
) -> Result<Vec<Message>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    // Try JOIN with trace_log.message_id (v23+), fallback to plain query if column missing
    let result = conn.prepare(
        "SELECT m.id, m.conversation_id, m.role, m.content, m.timestamp, m.status,
                (m.progress_content IS NOT NULL) as has_progress,
                m.engine, m.model, m.persona,
                t.duration_ms, t.input_tokens, t.output_tokens, t.cost_usd
         FROM messages m
         LEFT JOIN (
           SELECT message_id, duration_ms, input_tokens, output_tokens, cost_usd
           FROM trace_log WHERE message_id IS NOT NULL
           GROUP BY message_id
         ) t ON t.message_id = m.id
         WHERE m.conversation_id = ?1 ORDER BY m.timestamp ASC",
    );
    match result {
        Ok(mut stmt) => {
            let rows = stmt.query_map([&conversation_id], map_row_light)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
        Err(_) => {
            // Fallback: no trace JOIN (pre-v23 schema)
            let mut stmt = conn.prepare(
                "SELECT id, conversation_id, role, content, timestamp, status,
                        (progress_content IS NOT NULL) as has_progress,
                        engine, model, persona
                 FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC",
            )?;
            let rows = stmt.query_map([&conversation_id], |row| {
                let has_progress: bool = row.get(6)?;
                Ok(Message {
                    id: row.get(0)?, conversation_id: row.get(1)?, role: row.get(2)?,
                    content: row.get(3)?, timestamp: row.get(4)?, status: row.get(5)?,
                    progress_content: if has_progress { Some("…".into()) } else { None },
                    engine: row.get(7)?, model: row.get(8)?, persona: row.get(9)?,
                    duration_ms: None, input_tokens: None, output_tokens: None, cost_usd: None,
                })
            })?.collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        }
    }
}

#[tauri::command]
pub fn create_user_message(
    input: CreateUserMessageInput,
    state: State<DbState>,
) -> Result<Message, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, status)
         VALUES (?1, ?2, 'user', ?3, ?4, 'done')",
        params![id, input.conversation_id, input.content, now],
    )?;
    Ok(Message {
        id,
        conversation_id: input.conversation_id,
        role: "user".into(),
        content: input.content,
        timestamp: now,
        status: "done".into(),
        progress_content: None,
        engine: None,
        model: None,
        persona: None,
        duration_ms: None, input_tokens: None, output_tokens: None, cost_usd: None,
    })
}

#[tauri::command]
pub fn append_assistant_message(
    input: AppendAssistantMessageInput,
    state: State<DbState>,
) -> Result<Message, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    let status = input.status.as_deref().unwrap_or("done").to_string();
    conn.execute(
        "INSERT INTO messages
         (id, conversation_id, role, content, timestamp, status, engine, model)
         VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            input.conversation_id,
            input.content,
            now,
            status,
            input.engine,
            input.model,
        ],
    )?;
    Ok(Message {
        id,
        conversation_id: input.conversation_id,
        role: "assistant".into(),
        content: input.content,
        timestamp: now,
        status,
        progress_content: None,
        engine: input.engine,
        model: input.model,
        persona: None,
        duration_ms: None, input_tokens: None, output_tokens: None, cost_usd: None,
    })
}

#[tauri::command]
pub fn update_message_status(
    input: UpdateMessageStatusInput,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    if let Some(content) = input.content {
        conn.execute(
            "UPDATE messages SET status = ?1, content = ?2 WHERE id = ?3",
            params![input.status, content, input.message_id],
        )?;
    } else {
        conn.execute(
            "UPDATE messages SET status = ?1 WHERE id = ?2",
            params![input.status, input.message_id],
        )?;
    }
    Ok(())
}

/// Lazy-load progress_content for a single message.
/// Called when user expands the ThinkingSummary block.
#[tauri::command]
pub fn get_progress_content(
    message_id: String,
    state: State<DbState>,
) -> Result<Option<String>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let result: Option<String> = conn
        .query_row(
            "SELECT progress_content FROM messages WHERE id = ?1",
            [&message_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    Ok(result)
}

/// Save thinking/tool-use progress content for a message.
/// Called by frontend after streaming completes, to persist progressContent to DB.
/// This data is NOT included in context building — display only.
#[tauri::command]
pub fn save_progress_content(
    message_id: String,
    progress_content: String,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    conn.execute(
        "UPDATE messages SET progress_content = ?1 WHERE id = ?2",
        params![progress_content, message_id],
    )?;
    Ok(())
}

/// Delete a user+assistant message pair.
///
/// Given any message ID:
/// - If user: deletes this + the next assistant message (by timestamp)
/// - If assistant: deletes this + the preceding user message (by timestamp)
/// Returns the number of deleted rows.
#[tauri::command]
pub fn delete_message_pair(
    message_id: String,
    state: State<DbState>,
) -> Result<i32, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;

    // Fetch the target message
    let (role, conv_id, ts): (String, String, i64) = conn
        .query_row(
            "SELECT role, conversation_id, timestamp FROM messages WHERE id = ?1",
            [&message_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|_| AppError::NotFound(format!("Message '{}' not found", message_id)))?;

    // Find the pair message
    let pair_id: Option<String> = if role == "user" {
        // Next assistant message after this user message
        conn.query_row(
            "SELECT id FROM messages
             WHERE conversation_id = ?1 AND role = 'assistant' AND timestamp > ?2
             ORDER BY timestamp ASC LIMIT 1",
            params![conv_id, ts],
            |row| row.get(0),
        ).ok()
    } else {
        // Previous user message before this assistant message
        conn.query_row(
            "SELECT id FROM messages
             WHERE conversation_id = ?1 AND role = 'user' AND timestamp < ?2
             ORDER BY timestamp DESC LIMIT 1",
            params![conv_id, ts],
            |row| row.get(0),
        ).ok()
    };

    // Clear FK references before deleting
    let ids_to_delete: Vec<&str> = std::iter::once(message_id.as_str())
        .chain(pair_id.as_deref())
        .collect();
    for id in &ids_to_delete {
        // branches.checkpoint_id → NULL
        conn.execute("UPDATE branches SET checkpoint_id = NULL WHERE checkpoint_id = ?1", [id])?;
        // memos referencing this message
        conn.execute("DELETE FROM memos WHERE message_id = ?1", [id])?;
    }

    // Delete messages
    let mut deleted = 0i32;
    conn.execute("DELETE FROM messages WHERE id = ?1", [&message_id])?;
    deleted += 1;
    if let Some(pid) = pair_id {
        conn.execute("DELETE FROM messages WHERE id = ?1", [&pid])?;
        deleted += 1;
    }

    Ok(deleted)
}

/// FTS5 search across messages for the current project.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub message_id: String,
    pub conversation_id: String,
    pub conversation_label: String,
    pub role: String,
    pub content_snippet: String,
    pub timestamp: i64,
    pub engine: Option<String>,
    pub persona: Option<String>,
}

#[tauri::command]
pub fn search_messages(
    query: String,
    project_key: String,
    limit: Option<i64>,
    state: State<DbState>,
) -> Result<Vec<SearchResult>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let max = limit.unwrap_or(20);

    // FTS5 match query — join with messages and conversations to get context
    let mut stmt = conn.prepare(
        "SELECT m.id, m.conversation_id, COALESCE(c.custom_label, c.label, ''), m.role,
                snippet(messages_fts, 0, '**', '**', '…', 40), m.timestamp, m.engine, m.persona
         FROM messages_fts fts
         JOIN messages m ON m.rowid = fts.rowid
         JOIN conversations c ON c.id = m.conversation_id
         WHERE messages_fts MATCH ?1
           AND c.project_key = ?2
         ORDER BY rank
         LIMIT ?3"
    )?;

    let results = stmt
        .query_map(params![query, project_key, max], |row| {
            Ok(SearchResult {
                message_id: row.get(0)?,
                conversation_id: row.get(1)?,
                conversation_label: row.get(2)?,
                role: row.get(3)?,
                content_snippet: row.get(4)?,
                timestamp: row.get(5)?,
                engine: row.get(6)?,
                persona: row.get(7)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(results)
}
