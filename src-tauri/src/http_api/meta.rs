//! Meta notifications HTTP endpoints — thin wrappers over the v38 meta_notifications
//! table. Mirrors the Tauri commands in `crate::commands::meta_notifications` but
//! accessible to mobile clients over REST.
//!
//! Refs: docs/api-inquiry-gamma-delta.md section C

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use rusqlite::params;
use serde::Deserialize;

use super::{ApiState, db_error, lock_conn};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    pub project_key: Option<String>,
    pub limit: Option<i64>,
}

fn map_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "id":           r.get::<_, String>(0)?,
        "projectKey":   r.get::<_, Option<String>>(1)?,
        "kind":         r.get::<_, String>(2)?,
        "title":        r.get::<_, String>(3)?,
        "summary":      r.get::<_, Option<String>>(4)?,
        "routeJson":    r.get::<_, Option<String>>(5)?,
        "createdAt":    r.get::<_, i64>(6)?,
        "readAt":       r.get::<_, Option<i64>>(7)?,
        "dismissedAt":  r.get::<_, Option<i64>>(8)?,
    }))
}

/// GET /api/meta-notifications?projectKey=X&limit=N
/// Filters: dismissed_at IS NULL. If projectKey is given, scopes to that project
/// OR global (project_key IS NULL) rows — matches Tauri command semantics.
pub async fn list_meta_notifications(
    State(state): State<ApiState>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let lim = q.limit.unwrap_or(50).clamp(1, 200);
    let cols = "id, project_key, kind, title, summary, route_json, created_at, read_at, dismissed_at";

    let project_key = q.project_key;
    let sql = if project_key.is_some() {
        format!(
            "SELECT {cols} FROM meta_notifications \
             WHERE dismissed_at IS NULL AND (project_key = ?1 OR project_key IS NULL) \
             ORDER BY created_at DESC LIMIT ?2"
        )
    } else {
        format!(
            "SELECT {cols} FROM meta_notifications \
             WHERE dismissed_at IS NULL \
             ORDER BY created_at DESC LIMIT ?1"
        )
    };
    let mut stmt = match conn.prepare(&sql) { Ok(s) => s, Err(e) => return db_error(e) };
    let rows: Vec<serde_json::Value> = if let Some(pk) = project_key {
        match stmt.query_map(params![pk, lim], map_row) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => return db_error(e),
        }
    } else {
        match stmt.query_map([lim], map_row) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => return db_error(e),
        }
    };
    Json(serde_json::json!(rows)).into_response()
}

/// POST /api/meta-notifications/{id}/read
pub async fn mark_read(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    let now = crate::db::migrations::now_epoch_ms();
    let updated = conn.execute(
        "UPDATE meta_notifications SET read_at = ?1 WHERE id = ?2 AND read_at IS NULL",
        params![now, id],
    ).unwrap_or(0);

    let msg = serde_json::json!({"type": "meta.read", "notificationId": id}).to_string();
    let _ = state.event_tx.send(msg);

    Json(serde_json::json!({"read": updated > 0, "id": id})).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectScopeInput {
    pub project_key: Option<String>,
}

/// POST /api/meta-notifications/mark-all-read  body: { projectKey? }
pub async fn mark_all_read(
    State(state): State<ApiState>,
    Json(input): Json<ProjectScopeInput>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    let now = crate::db::migrations::now_epoch_ms();
    let updated = if let Some(pk) = input.project_key {
        conn.execute(
            "UPDATE meta_notifications SET read_at = ?1 \
             WHERE read_at IS NULL AND dismissed_at IS NULL AND (project_key = ?2 OR project_key IS NULL)",
            params![now, pk],
        )
    } else {
        conn.execute(
            "UPDATE meta_notifications SET read_at = ?1 \
             WHERE read_at IS NULL AND dismissed_at IS NULL",
            [now],
        )
    }.unwrap_or(0);
    Json(serde_json::json!({"markedRead": updated})).into_response()
}

/// POST /api/meta-notifications/{id}/dismiss
pub async fn dismiss(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    let now = crate::db::migrations::now_epoch_ms();
    let updated = conn.execute(
        "UPDATE meta_notifications SET dismissed_at = ?1 WHERE id = ?2",
        params![now, id],
    ).unwrap_or(0);

    let msg = serde_json::json!({"type": "meta.dismissed", "notificationId": id}).to_string();
    let _ = state.event_tx.send(msg);

    if updated > 0 {
        Json(serde_json::json!({"dismissed": true, "id": id})).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "notification not found"}))).into_response()
    }
}

/// POST /api/meta-notifications/clear  body: { projectKey? }
/// Soft-delete (sets dismissed_at on all matching rows). Rows are preserved for
/// potential recovery per the Tauri command's policy.
pub async fn clear(
    State(state): State<ApiState>,
    Json(input): Json<ProjectScopeInput>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    let now = crate::db::migrations::now_epoch_ms();
    let updated = if let Some(pk) = input.project_key {
        conn.execute(
            "UPDATE meta_notifications SET dismissed_at = ?1 \
             WHERE dismissed_at IS NULL AND (project_key = ?2 OR project_key IS NULL)",
            params![now, pk],
        )
    } else {
        conn.execute(
            "UPDATE meta_notifications SET dismissed_at = ?1 WHERE dismissed_at IS NULL",
            [now],
        )
    }.unwrap_or(0);
    Json(serde_json::json!({"cleared": updated})).into_response()
}
