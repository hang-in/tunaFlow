//! Conversation, message, branch, memory, and search endpoints.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;

use super::{ApiState, db_error, lock_conn};

// ─── Conversation endpoints ─────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationQuery {
    pub project_key: Option<String>,
}

pub async fn list_conversations(
    State(state): State<ApiState>,
    Query(q): Query<ConversationQuery>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let map_conv = |r: &rusqlite::Row| -> rusqlite::Result<serde_json::Value> {
        Ok(serde_json::json!({
            "id":         r.get::<_, String>(0)?,
            "projectKey": r.get::<_, String>(1)?,
            "label":      r.get::<_, Option<String>>(2)?,
            "mode":       r.get::<_, String>(3)?,
            "type":       r.get::<_, String>(4)?,
            "updatedAt":  r.get::<_, i64>(5)?,
        }))
    };
    // Only return top-level conversations (type='main' or 'rt'), ordered by most recently updated.
    // Branch shadow conversations (id starts with 'branch:') are excluded.
    let rows: Vec<serde_json::Value> = if let Some(ref pk) = q.project_key {
        let mut stmt = match conn.prepare(
            "SELECT id, project_key, label, mode, type, updated_at FROM conversations \
             WHERE project_key = ?1 AND type IN ('main','rt') AND id NOT LIKE 'branch:%' \
             ORDER BY updated_at DESC"
        ) { Ok(s) => s, Err(e) => return db_error(e) };
        let result: Vec<serde_json::Value> = match stmt.query_map([pk], map_conv) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => return db_error(e),
        };
        result
    } else {
        let mut stmt = match conn.prepare(
            "SELECT id, project_key, label, mode, type, updated_at FROM conversations \
             WHERE type IN ('main','rt') AND id NOT LIKE 'branch:%' \
             ORDER BY updated_at DESC"
        ) { Ok(s) => s, Err(e) => return db_error(e) };
        let result: Vec<serde_json::Value> = match stmt.query_map([], map_conv) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => return db_error(e),
        };
        result
    };
    Json(serde_json::json!(rows)).into_response()
}

pub async fn list_messages(
    State(state): State<ApiState>,
    Path(conv_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let mut stmt = match conn.prepare(
        "SELECT id, role, content, engine, model, status, timestamp FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC"
    ) { Ok(s) => s, Err(e) => return db_error(e) };
    let rows: Vec<serde_json::Value> = match stmt.query_map([&conv_id], |r| Ok(serde_json::json!({
        "id": r.get::<_, String>(0)?, "role": r.get::<_, String>(1)?,
        "content": r.get::<_, String>(2)?, "engine": r.get::<_, Option<String>>(3)?,
        "model": r.get::<_, Option<String>>(4)?, "status": r.get::<_, String>(5)?,
        "timestamp": r.get::<_, i64>(6)?,
    }))) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => return db_error(e),
    };
    Json(serde_json::json!(rows)).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConversationInput {
    pub project_key: String,
    pub label: Option<String>,
}

pub async fn create_conversation(
    State(state): State<ApiState>,
    Json(input): Json<CreateConversationInput>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    let id = uuid::Uuid::new_v4().to_string();
    let label = input.label.unwrap_or_else(|| "API conversation".into());
    let now = crate::db::migrations::now_epoch_ms();
    if let Err(e) = conn.execute(
        "INSERT INTO conversations (id, project_key, label, mode, usage_status, source, created_at, updated_at) VALUES (?1, ?2, ?3, 'chat', 'active', 'api', ?4, ?4)",
        rusqlite::params![id, input.project_key, label, now],
    ) {
        return db_error(e);
    }
    (StatusCode::CREATED, Json(serde_json::json!({"id": id, "label": label, "createdAt": now}))).into_response()
}

pub async fn delete_conversation(
    State(state): State<ApiState>,
    Path(conv_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    conn.execute("DELETE FROM messages WHERE conversation_id = ?1", [&conv_id]).ok();
    conn.execute("DELETE FROM memos WHERE conversation_id = ?1", [&conv_id]).ok();
    let deleted = conn.execute("DELETE FROM conversations WHERE id = ?1", [&conv_id]).unwrap_or(0);
    if deleted > 0 {
        Json(serde_json::json!({"deleted": true, "conversationId": conv_id})).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "conversation not found"}))).into_response()
    }
}

// ─── Branch endpoints ───────────────────────────────────────────────────

pub async fn list_branches(
    State(state): State<ApiState>,
    Path(conv_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let mut stmt = match conn.prepare(
        "SELECT id, label, custom_label, status, checkpoint_id, mode, parent_branch_id, created_at FROM branches WHERE conversation_id = ?1 ORDER BY created_at ASC"
    ) { Ok(s) => s, Err(e) => return db_error(e) };
    let rows: Vec<serde_json::Value> = match stmt.query_map([&conv_id], |r| Ok(serde_json::json!({
        "id": r.get::<_, String>(0)?, "label": r.get::<_, String>(1)?,
        "customLabel": r.get::<_, Option<String>>(2)?, "status": r.get::<_, String>(3)?,
        "checkpointId": r.get::<_, Option<String>>(4)?, "mode": r.get::<_, String>(5)?,
        "parentBranchId": r.get::<_, Option<String>>(6)?, "createdAt": r.get::<_, i64>(7)?,
    }))) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => return db_error(e),
    };
    Json(serde_json::json!(rows)).into_response()
}

/// Phase 2 Finding 2-5: active plan pointer for the conversation.
/// Returns `{ planId, phase, title }` for the most recent non-done /
/// non-abandoned plan on this conversation, or `null` when none.
/// Mirrors `commands::plans::get_active_plan_phase` but includes the
/// plan id and title so mobile can route directly to the plan view.
pub async fn get_active_plan(
    State(state): State<ApiState>,
    Path(conv_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let row: Option<(String, String, String)> = conn
        .query_row(
            "SELECT id, title, phase FROM plans
             WHERE conversation_id = ?1 AND status != 'done' AND status != 'abandoned'
             ORDER BY updated_at DESC LIMIT 1",
            [&conv_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    match row {
        Some((id, title, phase)) => Json(serde_json::json!({
            "planId": id, "title": title, "phase": phase,
        })).into_response(),
        None => Json(serde_json::json!(null)).into_response(),
    }
}

/// Phase 2 Finding 2-2: branch detail endpoint for mobile δ-Branch.
/// Consolidates the fields a single-branch detail view needs in one
/// call: labels / status / mode / parent, the rt_config (participants)
/// if this branch is a roundtable, and the `adopted_message_id` for
/// display of the adoption summary.
pub async fn get_branch_detail(
    State(state): State<ApiState>,
    Path(branch_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    type Row = (
        String, String, Option<String>, String, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<String>, Option<String>, i64,
    );
    let row: Result<Row, _> = conn.query_row(
        "SELECT id, label, custom_label, status, checkpoint_id, mode,
                parent_branch_id, subtask_id, adopted_message_id, conversation_id,
                created_at
         FROM branches WHERE id = ?1",
        [&branch_id],
        |r| Ok((
            r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?,
            r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?,
        )),
    );
    let (
        id, label, custom_label, status, checkpoint_id, mode,
        parent_branch_id, subtask_id, adopted_message_id, parent_conv_id, created_at,
    ) = match row {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "branch not found"})),
            )
                .into_response()
        }
    };
    // rt_config lives on the shadow conversation row — present only for
    // roundtable-mode branches. Absent → null.
    let shadow_id = format!("branch:{}", id);
    let rt_config: Option<String> = conn
        .query_row(
            "SELECT rt_config FROM conversations WHERE id = ?1",
            [&shadow_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let participants: Option<serde_json::Value> = rt_config
        .as_deref()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| v.get("participants").cloned());
    Json(serde_json::json!({
        "id": id,
        "label": label,
        "customLabel": custom_label,
        "status": status,
        "mode": mode,
        "checkpointId": checkpoint_id,
        "parentBranchId": parent_branch_id,
        "subtaskId": subtask_id,
        "adoptedMessageId": adopted_message_id,
        "parentConversationId": parent_conv_id,
        "shadowConversationId": shadow_id,
        "participants": participants,
        "createdAt": created_at,
    })).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchInput {
    pub conversation_id: String,
    pub label: Option<String>,
    pub mode: Option<String>,
    pub checkpoint_id: Option<String>,
}

pub async fn create_branch(
    State(state): State<ApiState>,
    Json(input): Json<CreateBranchInput>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    let id = uuid::Uuid::new_v4().to_string();
    let label = input.label.unwrap_or_else(|| format!("b{}", id.chars().take(4).collect::<String>()));
    let mode = input.mode.unwrap_or_else(|| "chat".into());
    let now = crate::db::migrations::now_epoch_ms();

    if let Err(e) = conn.execute(
        "INSERT INTO branches (id, conversation_id, label, status, mode, checkpoint_id, created_at) VALUES (?1, ?2, ?3, 'active', ?4, ?5, ?6)",
        rusqlite::params![id, input.conversation_id, label, mode, input.checkpoint_id, now],
    ) {
        return db_error(e);
    }

    let shadow_id = format!("branch:{}", id);
    if let Err(e) = conn.execute(
        "INSERT INTO conversations (id, project_key, label, mode, usage_status, source, created_at, updated_at) \
         SELECT ?1, project_key, ?2, ?3, 'active', 'api', ?4, ?4 FROM conversations WHERE id = ?5",
        rusqlite::params![shadow_id, format!("Branch {}", label), mode, now, input.conversation_id],
    ) {
        return db_error(e);
    }
    drop(conn);

    let _ = state.event_tx.send(serde_json::json!({
        "type": "branch:created",
        "branchId": id, "conversationId": input.conversation_id, "mode": mode
    }).to_string());

    (StatusCode::CREATED, Json(serde_json::json!({
        "id": id, "label": label, "mode": mode, "shadowConversationId": shadow_id
    }))).into_response()
}

pub async fn delete_branch(
    State(state): State<ApiState>,
    Path(branch_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    let shadow_id = format!("branch:{}", branch_id);
    conn.execute("DELETE FROM messages WHERE conversation_id = ?1", [&shadow_id]).ok();
    conn.execute("DELETE FROM conversations WHERE id = ?1", [&shadow_id]).ok();
    let deleted = conn.execute("DELETE FROM branches WHERE id = ?1", [&branch_id]).unwrap_or(0);
    if deleted > 0 {
        Json(serde_json::json!({"deleted": true, "branchId": branch_id})).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "branch not found"}))).into_response()
    }
}

pub async fn archive_branch(
    State(state): State<ApiState>,
    Path(branch_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    let updated = conn.execute("UPDATE branches SET status = 'archived' WHERE id = ?1", [&branch_id]).unwrap_or(0);
    if updated > 0 {
        drop(conn);
        let _ = state.event_tx.send(serde_json::json!({
            "type": "branch:archived", "branchId": branch_id
        }).to_string());
        Json(serde_json::json!({"archived": true, "branchId": branch_id})).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "branch not found"}))).into_response()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptInput {
    pub conversation_id: String,
}

pub async fn adopt_branch(
    State(state): State<ApiState>,
    Path(branch_id): Path<String>,
    Json(input): Json<AdoptInput>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    let shadow_id = format!("branch:{}", branch_id);

    let summary = {
        let mut stmt = match conn.prepare(
            "SELECT content, persona, engine FROM messages WHERE conversation_id = ?1 AND role = 'assistant' ORDER BY timestamp ASC"
        ) { Ok(s) => s, Err(e) => return db_error(e) };
        let parts: Vec<String> = match stmt.query_map([&shadow_id], |r| {
            let content: String = r.get(0)?;
            let persona: Option<String> = r.get(1)?;
            let engine: Option<String> = r.get(2)?;
            let label = persona.or(engine).unwrap_or_default();
            let truncated = if content.len() > 300 { format!("{}...", &content[..300]) } else { content };
            Ok(if label.is_empty() { truncated } else { format!("**[{}]** {}", label, truncated) })
        }) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => return db_error(e),
        };
        if parts.is_empty() { "(no summary available)".to_string() } else { parts.join("\n\n") }
    };

    let updated = conn.execute("UPDATE branches SET status = 'adopted' WHERE id = ?1", [&branch_id]).unwrap_or(0);
    if updated == 0 {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "branch not found"}))).into_response();
    }

    let msg_id = uuid::Uuid::new_v4().to_string();
    let now = crate::db::migrations::now_epoch_ms();
    let capped = if summary.len() > 2000 { format!("{}...", &summary[..2000]) } else { summary };
    let adopt_content = format!("[Branch adopted]\n\n{}", capped);
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES (?1, ?2, 'system', ?3, ?4, 'done')",
        rusqlite::params![msg_id, input.conversation_id, adopt_content, now],
    ).ok();
    drop(conn);

    let _ = state.event_tx.send(serde_json::json!({
        "type": "branch:adopted",
        "branchId": branch_id, "summaryMessageId": msg_id,
        "conversationId": input.conversation_id
    }).to_string());

    Json(serde_json::json!({"adopted": true, "branchId": branch_id, "summaryMessageId": msg_id})).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameInput {
    pub label: String,
}

pub async fn rename_branch(
    State(state): State<ApiState>,
    Path(branch_id): Path<String>,
    Json(input): Json<RenameInput>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    let updated = conn.execute(
        "UPDATE branches SET custom_label = ?1 WHERE id = ?2",
        rusqlite::params![input.label, branch_id],
    ).unwrap_or(0);
    if updated > 0 {
        Json(serde_json::json!({"renamed": true, "branchId": branch_id, "label": input.label})).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "branch not found"}))).into_response()
    }
}

// ─── Memory & search endpoints ──────────────────────────────────────────

pub async fn memory_status(
    State(state): State<ApiState>,
    Path(conv_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let status = crate::commands::conversation_memory::get_memory_status(&conn, &conv_id);
    Json(serde_json::json!(status)).into_response()
}

pub async fn compress_memory(
    State(state): State<ApiState>,
    Path(conv_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let cid = conv_id.clone();
    match tokio::task::spawn_blocking(move || {
        crate::commands::conversation_memory::compress_memory_blocking(&db, &cid)
    }).await {
        Ok(Ok(compressed)) => Json(serde_json::json!({"compressed": compressed, "conversationId": conv_id})).into_response(),
        Ok(Err(e)) => db_error(e),
        Err(e) => db_error(format!("task: {}", e)),
    }
}

pub async fn list_session_links(
    State(state): State<ApiState>,
    Path(conv_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let mut stmt = match conn.prepare(
        "SELECT id, linked_conv_id, score, method, created_at FROM session_links WHERE conversation_id = ?1 ORDER BY score DESC"
    ) { Ok(s) => s, Err(e) => return db_error(e) };
    let rows: Vec<serde_json::Value> = match stmt.query_map([&conv_id], |r| Ok(serde_json::json!({
        "id": r.get::<_, String>(0)?, "linkedConvId": r.get::<_, String>(1)?,
        "score": r.get::<_, f64>(2)?, "method": r.get::<_, String>(3)?,
        "createdAt": r.get::<_, i64>(4)?,
    }))) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => return db_error(e),
    };
    Json(serde_json::json!(rows)).into_response()
}

pub async fn refresh_session_links(
    State(state): State<ApiState>,
    Path(conv_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let project_key: String = match conn.query_row(
        "SELECT project_key FROM conversations WHERE id = ?1", [&conv_id], |r| r.get(0),
    ) {
        Ok(pk) => pk,
        Err(_) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "conversation not found"}))).into_response(),
    };
    let links = crate::commands::session_discovery::discover_related_sessions(&conn, &conv_id, &project_key, 5);
    drop(conn);
    let write_conn = lock_conn(&state.db.write);
    let now = crate::db::migrations::now_epoch_ms();
    for (linked_id, score) in &links {
        let link_id = uuid::Uuid::new_v4().to_string();
        write_conn.execute(
            "INSERT OR REPLACE INTO session_links (id, conversation_id, linked_conv_id, score, method, created_at) VALUES (?1, ?2, ?3, ?4, 'fts5', ?5)",
            rusqlite::params![link_id, conv_id, linked_id, score, now],
        ).ok();
    }
    Json(serde_json::json!({"refreshed": links.len(), "links": links.iter().map(|(id, score)| serde_json::json!({"conversationId": id, "score": score})).collect::<Vec<_>>()})).into_response()
}

pub async fn index_chunks(
    State(state): State<ApiState>,
    Path(conv_id): Path<String>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let cid = conv_id.clone();
    match tokio::task::spawn_blocking(move || {
        crate::commands::vector_search::index_chunks_blocking(&db, &cid)
    }).await {
        Ok(Ok(count)) => Json(serde_json::json!({"indexed": count, "conversationId": conv_id})).into_response(),
        Ok(Err(e)) => db_error(e),
        Err(e) => db_error(format!("task: {}", e)),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchChunksInput {
    pub query: String,
    pub limit: Option<usize>,
}

pub async fn search_chunks(
    State(state): State<ApiState>,
    Path(conv_id): Path<String>,
    Json(input): Json<SearchChunksInput>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let query = input.query;
    let limit = input.limit.unwrap_or(5);
    match tokio::task::spawn_blocking(move || {
        crate::commands::vector_search::search_chunks_blocking(&db, &conv_id, &query, limit)
    }).await {
        Ok(Ok(results)) => Json(serde_json::json!(results)).into_response(),
        Ok(Err(e)) => db_error(e),
        Err(e) => db_error(format!("task: {}", e)),
    }
}

pub async fn list_conv_traces(
    State(state): State<ApiState>,
    Path(conv_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let mut stmt = match conn.prepare(
        "SELECT id, trace_id, span_id, engine, context_mode, context_length, input_tokens, output_tokens, cost_usd, created_at FROM trace_log WHERE conversation_id = ?1 ORDER BY created_at DESC LIMIT 20"
    ) { Ok(s) => s, Err(e) => return db_error(e) };
    let rows: Vec<serde_json::Value> = match stmt.query_map([&conv_id], |r| Ok(serde_json::json!({
        "id": r.get::<_, String>(0)?, "traceId": r.get::<_, Option<String>>(1)?,
        "engine": r.get::<_, Option<String>>(3)?, "contextMode": r.get::<_, Option<String>>(4)?,
        "contextLength": r.get::<_, i64>(5)?, "inputTokens": r.get::<_, i64>(6)?,
        "outputTokens": r.get::<_, i64>(7)?, "costUsd": r.get::<_, f64>(8)?,
        "createdAt": r.get::<_, i64>(9)?,
    }))) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => return db_error(e),
    };
    Json(serde_json::json!(rows)).into_response()
}
