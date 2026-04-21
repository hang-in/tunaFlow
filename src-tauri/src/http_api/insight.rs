//! Insight HTTP endpoints (Phase 5 E2E).
//!
//! Read-only + status-update surface over `insight_sessions` / `insight_findings`.
//! Mirrors selected Tauri commands in `crate::commands::insight` so mobile clients
//! and the E2E automation script can exercise scenario 6 over REST.
//!
//! Deliberately excludes the heavy analysis trigger (`run_insight_analysis`) —
//! that path spawns an LLM subprocess and belongs behind the queued job API.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use rusqlite::params;
use serde::Deserialize;

use super::{ApiState, db_error, lock_conn};

const SESSION_COLS: &str =
    "id, project_key, status, categories, test_output, summary, created_at, completed_at";
const FINDING_COLS: &str =
    "id, session_id, project_key, category, severity, fix_difficulty, \
     title, description, file_path, line_number, snippet, \
     status, resolution, plan_id, estimated_files, created_at";

fn map_session(r: &rusqlite::Row<'_>) -> rusqlite::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "id":           r.get::<_, String>(0)?,
        "projectKey":   r.get::<_, String>(1)?,
        "status":       r.get::<_, String>(2)?,
        "categories":   r.get::<_, Option<String>>(3)?,
        "testOutput":   r.get::<_, Option<String>>(4)?,
        "summary":      r.get::<_, Option<String>>(5)?,
        "createdAt":    r.get::<_, i64>(6)?,
        "completedAt":  r.get::<_, Option<i64>>(7)?,
    }))
}

fn map_finding(r: &rusqlite::Row<'_>) -> rusqlite::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "id":             r.get::<_, String>(0)?,
        "sessionId":      r.get::<_, String>(1)?,
        "projectKey":     r.get::<_, String>(2)?,
        "category":       r.get::<_, String>(3)?,
        "severity":       r.get::<_, String>(4)?,
        "fixDifficulty":  r.get::<_, String>(5)?,
        "title":          r.get::<_, String>(6)?,
        "description":    r.get::<_, String>(7)?,
        "filePath":       r.get::<_, Option<String>>(8)?,
        "lineNumber":     r.get::<_, Option<i64>>(9)?,
        "snippet":        r.get::<_, Option<String>>(10)?,
        "status":         r.get::<_, String>(11)?,
        "resolution":     r.get::<_, Option<String>>(12)?,
        "planId":         r.get::<_, Option<String>>(13)?,
        "estimatedFiles": r.get::<_, Option<i64>>(14)?,
        "createdAt":      r.get::<_, i64>(15)?,
    }))
}

/// GET /api/v1/projects/{key}/insight/sessions
pub async fn list_sessions(
    State(state): State<ApiState>,
    Path(project_key): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let sql = format!(
        "SELECT {} FROM insight_sessions WHERE project_key = ?1 ORDER BY created_at DESC",
        SESSION_COLS
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return db_error(e).into_response(),
    };
    let rows: Vec<_> = match stmt.query_map([&project_key], map_session) {
        Ok(r) => r.filter_map(|x| x.ok()).collect(),
        Err(e) => return db_error(e).into_response(),
    };
    Json(rows).into_response()
}

/// GET /api/v1/projects/{key}/insight/findings?sessionId=&category=&status=
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingsQuery {
    pub session_id: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
}

pub async fn list_findings(
    State(state): State<ApiState>,
    Path(project_key): Path<String>,
    Query(q): Query<FindingsQuery>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);

    // Build WHERE clause — all filters optional.
    let mut where_parts = vec!["project_key = ?1".to_string()];
    let mut args: Vec<String> = vec![project_key];

    if let Some(s) = &q.session_id {
        where_parts.push(format!("session_id = ?{}", args.len() + 1));
        args.push(s.clone());
    }
    if let Some(c) = &q.category {
        where_parts.push(format!("category = ?{}", args.len() + 1));
        args.push(c.clone());
    }
    if let Some(st) = &q.status {
        where_parts.push(format!("status = ?{}", args.len() + 1));
        args.push(st.clone());
    }

    let sql = format!(
        "SELECT {} FROM insight_findings WHERE {} \
         ORDER BY CASE severity WHEN 'critical' THEN 0 WHEN 'major' THEN 1 \
         WHEN 'minor' THEN 2 ELSE 3 END, created_at",
        FINDING_COLS,
        where_parts.join(" AND ")
    );

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(e) => return db_error(e).into_response(),
    };
    let params_slice: Vec<&dyn rusqlite::ToSql> =
        args.iter().map(|a| a as &dyn rusqlite::ToSql).collect();
    let rows: Vec<_> = match stmt.query_map(params_slice.as_slice(), map_finding) {
        Ok(r) => r.filter_map(|x| x.ok()).collect(),
        Err(e) => return db_error(e).into_response(),
    };
    Json(rows).into_response()
}

/// GET /api/v1/projects/{key}/insight/findings/count?status=open
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountQuery {
    pub status: Option<String>,
}

pub async fn count_findings(
    State(state): State<ApiState>,
    Path(project_key): Path<String>,
    Query(q): Query<CountQuery>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let status = q.status.unwrap_or_else(|| "open".to_string());
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM insight_findings f
             JOIN insight_sessions s ON s.id = f.session_id
             WHERE s.project_key = ?1 AND f.status = ?2",
            params![project_key, status],
            |r| r.get(0),
        )
        .unwrap_or(0);
    Json(serde_json::json!({ "count": count })).into_response()
}

/// POST /api/v1/insight/findings/{id}/status
/// Body: { status: "open"|"resolved"|"wont_fix", resolution?: string, planId?: string }
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusUpdate {
    pub status: String,
    pub resolution: Option<String>,
    pub plan_id: Option<String>,
}

pub async fn update_finding_status(
    State(state): State<ApiState>,
    Path(finding_id): Path<String>,
    Json(body): Json<StatusUpdate>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    if let Err(e) = conn.execute(
        "UPDATE insight_findings SET status = ?1,
         resolution = COALESCE(?2, resolution),
         plan_id = COALESCE(?3, plan_id)
         WHERE id = ?4",
        params![body.status, body.resolution, body.plan_id, finding_id],
    ) {
        return db_error(e).into_response();
    }
    let sql = format!(
        "SELECT {} FROM insight_findings WHERE id = ?1",
        FINDING_COLS
    );
    match conn.query_row(&sql, [&finding_id], map_finding) {
        Ok(row) => Json(row).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "insight finding not found").into_response(),
    }
}
