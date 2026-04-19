use rusqlite::params;
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use crate::db::{migrations::now_epoch_ms, models::Conversation, DbState};
use crate::errors::AppError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConversationInput {
    pub project_key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub conv_type: Option<String>,
    pub mode: Option<String>,
    pub source: Option<String>,
    pub engine: Option<String>,
    pub model: Option<String>,
}

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Conversation> {
    Ok(Conversation {
        id: row.get(0)?,
        project_key: row.get(1)?,
        label: row.get(2)?,
        custom_label: row.get(3)?,
        conv_type: row.get(4)?,
        mode: row.get(5)?,
        parent_id: row.get(6)?,
        source: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        engine: row.get(10)?,
        model: row.get(11)?,
        persona: row.get(12)?,
        trigger_mode: row.get(13)?,
        total_input_tokens: row.get(14)?,
        total_output_tokens: row.get(15)?,
        total_cost_usd: row.get(16)?,
        resume_token: row.get(17)?,
    })
}

const SELECT_COLS: &str =
    "id, project_key, label, custom_label, type, mode, parent_id, source,
     created_at, updated_at, engine, model, persona, trigger_mode,
     total_input_tokens, total_output_tokens, total_cost_usd, resume_token";

#[tauri::command]
pub fn list_conversations(
    project_key: String,
    state: State<DbState>,
) -> Result<Vec<Conversation>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let sql = format!(
        "SELECT {} FROM conversations WHERE project_key = ?1 ORDER BY updated_at DESC",
        SELECT_COLS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([&project_key], map_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn create_conversation(
    input: CreateConversationInput,
    state: State<DbState>,
) -> Result<Conversation, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let id = Uuid::new_v4().to_string();
    // 컨벤션: ms. messages.timestamp/plans.created_at 과 단위 일치.
    let now = now_epoch_ms();
    let conv_type = input.conv_type.as_deref().unwrap_or("main").to_string();
    let mode = input.mode.as_deref().unwrap_or("chat").to_string();
    let source = input.source.as_deref().unwrap_or("tunadish").to_string();

    conn.execute(
        "INSERT INTO conversations
         (id, project_key, label, type, mode, source, created_at, updated_at,
          engine, model, total_input_tokens, total_output_tokens, total_cost_usd)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, 0, 0.0)",
        params![
            id,
            input.project_key,
            input.label,
            conv_type,
            mode,
            source,
            now,
            now,
            input.engine,
            input.model,
        ],
    )?;

    Ok(Conversation {
        id,
        project_key: input.project_key,
        label: input.label,
        custom_label: None,
        conv_type,
        mode,
        parent_id: None,
        source,
        created_at: now,
        updated_at: now,
        engine: input.engine,
        model: input.model,
        persona: None,
        trigger_mode: None,
        total_input_tokens: 0,
        total_output_tokens: 0,
        total_cost_usd: 0.0,
        resume_token: None,
    })
}

/// Delete a conversation and all associated data.
///
/// CASCADE handles: messages, branches (via FK ON DELETE CASCADE).
/// Manual cleanup: memos, artifacts, trace_log (no FK cascade).
/// Also deletes shadow branch conversations (parent_id = this conversation).
#[tauri::command]
pub fn delete_conversation(id: String, state: State<DbState>) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;

    // Manual cleanup for tables without FK CASCADE
    conn.execute("DELETE FROM conversation_memory WHERE conversation_id = ?1", [&id])?;
    conn.execute("DELETE FROM memos WHERE conversation_id = ?1", [&id])?;
    conn.execute("DELETE FROM artifacts WHERE conversation_id = ?1", [&id])?;
    conn.execute("DELETE FROM trace_log WHERE conversation_id = ?1", [&id])?;

    // Delete shadow branch conversations (branch:{branchId} rows whose parent_id = this conv)
    // Their messages/branches are also cascade-deleted.
    let shadow_ids: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM conversations WHERE parent_id = ?1",
        )?;
        let ids: Vec<String> = stmt.query_map([&id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        ids
    };
    for shadow_id in &shadow_ids {
        conn.execute("DELETE FROM conversation_memory WHERE conversation_id = ?1", [shadow_id])?;
        conn.execute("DELETE FROM memos WHERE conversation_id = ?1", [shadow_id])?;
        conn.execute("DELETE FROM artifacts WHERE conversation_id = ?1", [shadow_id])?;
        conn.execute("DELETE FROM trace_log WHERE conversation_id = ?1", [shadow_id])?;
        conn.execute("DELETE FROM conversations WHERE id = ?1", [shadow_id])?;
    }

    // Delete the conversation itself (messages + branches cascade via FK)
    conn.execute("DELETE FROM conversations WHERE id = ?1", [&id])?;

    Ok(())
}

/// Set or clear the user-facing display label for a conversation.
/// Empty string → NULL (fallback to auto-generated label).
#[tauri::command]
pub fn rename_conversation(id: String, custom_label: String, state: State<DbState>) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let value: Option<&str> = if custom_label.trim().is_empty() { None } else { Some(custom_label.trim()) };
    conn.execute(
        "UPDATE conversations SET custom_label = ?1, updated_at = ?2 WHERE id = ?3",
        params![value, now_epoch_ms(), id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_conversation(id: String, state: State<DbState>) -> Result<Conversation, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let sql = format!(
        "SELECT {} FROM conversations WHERE id = ?1",
        SELECT_COLS
    );
    conn.query_row(&sql, [&id], map_row)
        .map_err(|_| AppError::NotFound(format!("Conversation '{}' not found", id)))
}

/// Update the resume_token (Claude session ID) for a conversation.
/// Used by PTY mode to persist session identity for --resume on next start.
#[tauri::command]
pub fn update_resume_token(
    conversation_id: String,
    resume_token: Option<String>,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    conn.execute(
        "UPDATE conversations SET resume_token = ?1 WHERE id = ?2",
        params![resume_token, conversation_id],
    )?;
    Ok(())
}

/// Save RT config (participants + mode) as JSON in conversations.rt_config.
/// Persists across app restarts (unlike sessionStorage).
#[tauri::command]
pub fn save_rt_config(conversation_id: String, config_json: String, state: State<DbState>) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    conn.execute(
        "UPDATE conversations SET rt_config = ?1 WHERE id = ?2",
        params![config_json, conversation_id],
    )?;
    Ok(())
}

/// Load RT config for a conversation. Returns null if none saved.
#[tauri::command]
pub fn get_rt_config(conversation_id: String, state: State<DbState>) -> Result<Option<String>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let result: Option<String> = conn
        .query_row(
            "SELECT rt_config FROM conversations WHERE id = ?1",
            [&conversation_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    Ok(result)
}
