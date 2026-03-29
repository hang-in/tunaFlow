use rusqlite::params;
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use crate::db::{
    migrations::{now_epoch, now_epoch_ms},
    models::{Branch, Message},
    DbState,
};
use crate::errors::AppError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchInput {
    pub conversation_id: String,
    /// Auto-generated as b1, b1.1, etc. if not provided
    pub label: Option<String>,
    pub checkpoint_id: Option<String>,
    pub parent_branch_id: Option<String>,
    /// "chat" (default) | "roundtable"
    pub mode: Option<String>,
    /// Plan subtask this branch implements (developer lane linkage)
    pub subtask_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptBranchInput {
    pub branch_id: String,
    pub conversation_id: String,
}

#[tauri::command]
pub fn list_branches(
    conversation_id: String,
    state: State<DbState>,
) -> Result<Vec<Branch>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, label, custom_label, status,
                checkpoint_id, parent_branch_id, session_id, git_branch, mode, subtask_id, created_at
         FROM branches WHERE conversation_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map([&conversation_id], |row| {
            Ok(Branch {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                label: row.get(2)?,
                custom_label: row.get(3)?,
                status: row.get(4)?,
                checkpoint_id: row.get(5)?,
                parent_branch_id: row.get(6)?,
                session_id: row.get(7)?,
                git_branch: row.get(8)?,
                mode: row.get(9)?,
                subtask_id: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn create_branch(
    input: CreateBranchInput,
    state: State<DbState>,
) -> Result<Branch, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let id = Uuid::new_v4().to_string();
    let now = now_epoch();

    // Auto-generate label: b1, b1.1, b2, etc.
    let label = match input.label {
        Some(l) => l,
        None => {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM branches WHERE conversation_id = ?1",
                [&input.conversation_id],
                |row| row.get(0),
            )?;
            match &input.parent_branch_id {
                None => format!("b{}", count + 1),
                Some(parent_id) => {
                    // Nested: derive from parent label
                    let parent_label: String = conn
                        .query_row(
                            "SELECT label FROM branches WHERE id = ?1",
                            [parent_id],
                            |row| row.get(0),
                        )
                        .unwrap_or_else(|_| format!("b{}", count));
                    let nested_count: i64 = conn.query_row(
                        "SELECT COUNT(*) FROM branches WHERE parent_branch_id = ?1",
                        [parent_id],
                        |row| row.get(0),
                    )?;
                    format!("{}.{}", parent_label, nested_count + 1)
                }
            }
        }
    };

    let branch_mode = input.mode.as_deref().unwrap_or("chat");
    conn.execute(
        "INSERT INTO branches
         (id, conversation_id, label, status, checkpoint_id, parent_branch_id, mode, subtask_id, created_at)
         VALUES (?1, ?2, ?3, 'active', ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            input.conversation_id,
            label,
            input.checkpoint_id,
            input.parent_branch_id,
            branch_mode,
            input.subtask_id,
            now,
        ],
    )?;

    Ok(Branch {
        id,
        conversation_id: input.conversation_id,
        label,
        custom_label: None,
        status: "active".into(),
        checkpoint_id: input.checkpoint_id,
        parent_branch_id: input.parent_branch_id,
        session_id: None,
        git_branch: None,
        mode: Some(branch_mode.to_string()),
        subtask_id: input.subtask_id,
        created_at: now,
    })
}

/// Open (or ensure) a branch-dedicated conversation stream (DATA_MODEL §1.4, §1.5).
///
/// Branch messages are stored with `conversation_id = "branch:{branch_id}"`.
/// Because the `messages` table has FK → `conversations`, this command
/// creates a shadow `conversations` row with that id on first call (idempotent).
/// Returns the branch conversation id (`"branch:{branch_id}"`).
#[tauri::command]
pub fn open_branch_stream(
    branch_id: String,
    state: State<DbState>,
) -> Result<String, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let branch_conv_id = format!("branch:{}", branch_id);

    // Idempotent: skip creation if shadow row already exists
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations WHERE id = ?1",
            [&branch_conv_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if !exists {
        // Resolve branch → parent conversation → project_key + branch mode
        let (parent_conv_id, branch_label, branch_mode): (String, String, String) = conn
            .query_row(
                "SELECT conversation_id, label, COALESCE(mode, 'chat') FROM branches WHERE id = ?1",
                [&branch_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|_| AppError::NotFound(format!("Branch '{}' not found", branch_id)))?;

        let project_key: String = conn
            .query_row(
                "SELECT project_key FROM conversations WHERE id = ?1",
                [&parent_conv_id],
                |row| row.get(0),
            )
            .map_err(|_| {
                AppError::NotFound(format!("Conversation '{}' not found", parent_conv_id))
            })?;

        let now = now_epoch();
        conn.execute(
            "INSERT INTO conversations
             (id, project_key, label, type, mode, parent_id, source,
              created_at, updated_at, total_input_tokens, total_output_tokens, total_cost_usd)
             VALUES (?1, ?2, ?3, 'branch', ?4, ?5, 'tunadish', ?6, ?6, 0, 0, 0.0)",
            params![
                branch_conv_id,
                project_key,
                format!("Branch {}", branch_label),
                branch_mode,
                parent_conv_id,
                now,
            ],
        )?;
    }

    Ok(branch_conv_id)
}

/// Set or clear the user-facing display label for a branch.
/// Empty string → NULL (fallback to auto-generated label).
#[tauri::command]
pub fn rename_branch(id: String, custom_label: String, state: State<DbState>) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let value: Option<&str> = if custom_label.trim().is_empty() { None } else { Some(custom_label.trim()) };
    conn.execute(
        "UPDATE branches SET custom_label = ?1 WHERE id = ?2",
        params![value, id],
    )?;
    Ok(())
}

/// Delete a branch and its shadow conversation + messages.
#[tauri::command]
pub fn delete_branch(
    id: String,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;

    let branch_conv_id = format!("branch:{}", id);

    // Delete shadow conversation messages (FK cascade would handle this,
    // but the shadow conv itself needs explicit cleanup)
    conn.execute(
        "DELETE FROM messages WHERE conversation_id = ?1",
        [&branch_conv_id],
    )?;
    conn.execute(
        "DELETE FROM memos WHERE conversation_id = ?1",
        [&branch_conv_id],
    )?;
    conn.execute(
        "DELETE FROM artifacts WHERE conversation_id = ?1",
        [&branch_conv_id],
    )?;
    // Delete shadow conversation row
    conn.execute(
        "DELETE FROM conversations WHERE id = ?1",
        [&branch_conv_id],
    )?;
    // Delete the branch itself
    conn.execute("DELETE FROM branches WHERE id = ?1", [&id])?;

    Ok(())
}

/// DATA_MODEL §3.3 Adopt flow (simplified):
/// 1. Branch.status → 'adopted'
/// 2. Insert placeholder adopt-summary message in parent Conversation
#[tauri::command]
pub fn adopt_branch(
    input: AdoptBranchInput,
    state: State<DbState>,
) -> Result<Message, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;

    // Validate branch exists and is active
    let (branch_label, branch_mode): (String, String) = conn
        .query_row(
            "SELECT label, COALESCE(mode, 'chat') FROM branches WHERE id = ?1 AND status = 'active'",
            [&input.branch_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| AppError::NotFound(format!("Active branch '{}' not found", input.branch_id)))?;

    // 1. Check content BEFORE changing status — so empty_branch error doesn't leave status as 'adopted'
    let shadow_id = format!("branch:{}", input.branch_id);
    let is_rt = branch_mode == "roundtable";

    // Try to get RT brief first
    let brief: Option<String> = conn
        .query_row(
            "SELECT content FROM memos
             WHERE conversation_id = ?1 AND type = 'roundtable_brief'
             ORDER BY created_at DESC LIMIT 1",
            [&shadow_id],
            |row| row.get(0),
        )
        .ok();

    let summary_body = if let Some(ref brief_content) = brief {
        // Extract Key Positions section from brief
        let lines: Vec<&str> = brief_content.lines().collect();
        let pos_start = lines.iter().position(|l| l.contains("Key Positions"));
        let key_points = if let Some(idx) = pos_start {
            lines[idx + 1..].iter()
                .filter(|l| l.starts_with("- "))
                .take(4)
                .copied()
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            // Fallback: first meaningful lines from brief
            lines.iter()
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .take(3)
                .copied()
                .collect::<Vec<_>>()
                .join("\n")
        };
        format!("### Key Points\n\n{}", key_points)
    } else {
        // Fallback for non-RT or RT without brief: last assistant message from branch
        let last_msg: Option<String> = conn
            .query_row(
                "SELECT content FROM messages
                 WHERE conversation_id = ?1 AND role = 'assistant' AND status = 'done'
                 ORDER BY timestamp DESC LIMIT 1",
                [&shadow_id],
                |row| row.get(0),
            )
            .ok();
        match last_msg {
            Some(msg) => {
                let preview = if msg.len() > 300 {
                    let end = msg.char_indices().map(|(i,_)|i).take_while(|&i| i <= 300).last().unwrap_or(0);
                    format!("{}...", &msg[..end])
                } else {
                    msg
                };
                format!("### Last Response\n\n{}", preview)
            }
            None => String::new(), // Empty branch — frontend will handle
        }
    };

    // If branch has no content, return error BEFORE changing status
    if summary_body.is_empty() {
        return Err(AppError::Agent("empty_branch".into()));
    }

    // Now safe to mark as adopted — content exists
    conn.execute(
        "UPDATE branches SET status = 'adopted' WHERE id = ?1",
        [&input.branch_id],
    )?;

    // Archive all descendant branches recursively
    conn.execute_batch(&format!(
        "WITH RECURSIVE descendants AS (
           SELECT id FROM branches WHERE parent_branch_id = '{bid}'
           UNION ALL
           SELECT b.id FROM branches b JOIN descendants d ON b.parent_branch_id = d.id
         )
         UPDATE branches SET status = 'archived'
         WHERE id IN (SELECT id FROM descendants) AND status = 'active';",
        bid = input.branch_id,
    ))?;

    // Get engine/model from the last assistant message in the branch
    let (last_engine, last_model): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT engine, model FROM messages
             WHERE conversation_id = ?1 AND role = 'assistant' AND status = 'done'
             ORDER BY timestamp DESC LIMIT 1",
            [&shadow_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((None, None));

    let msg_id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    let type_label = if is_rt { "Roundtable" } else { "Thread" };
    let content = format!(
        "## {} Adopted: {}\n\n{}\n",
        type_label, branch_label, summary_body
    );
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, status, engine, model)
         VALUES (?1, ?2, 'assistant', ?3, ?4, 'done', ?5, ?6)",
        params![msg_id, input.conversation_id, content, now, last_engine, last_model],
    )?;

    Ok(Message {
        id: msg_id,
        conversation_id: input.conversation_id,
        role: "assistant".into(),
        content,
        timestamp: now,
        status: "done".into(),
        progress_content: None,
        engine: last_engine,
        model: last_model,
        persona: None,
    })
}
