//! Plan and artifact endpoints.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use rusqlite::Connection;
use serde::Deserialize;

use super::{ApiState, db_error, lock_conn};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanQuery {
    pub conversation_id: Option<String>,
}

/// Canonical plan columns mirroring `commands::plans::PLAN_COLS`. Kept in one
/// place so the list and detail endpoints return the same shape.
const PLAN_COLS: &str = "id, conversation_id, branch_id, title, description, expected_outcome, \
    status, phase, architect_engine, developer_engine, reviewer_engines, \
    implementation_branch_id, review_branch_id, slug, revision, \
    version_major, version_minor, created_at, updated_at";

fn plan_row_to_json(r: &rusqlite::Row<'_>) -> rusqlite::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "id":                       r.get::<_, String>(0)?,
        "conversationId":           r.get::<_, String>(1)?,
        "branchId":                 r.get::<_, Option<String>>(2)?,
        "title":                    r.get::<_, String>(3)?,
        "description":              r.get::<_, Option<String>>(4)?,
        "expectedOutcome":          r.get::<_, Option<String>>(5)?,
        "status":                   r.get::<_, String>(6)?,
        "phase":                    r.get::<_, String>(7)?,
        "architectEngine":          r.get::<_, Option<String>>(8)?,
        "developerEngine":          r.get::<_, Option<String>>(9)?,
        "reviewerEngines":          r.get::<_, Option<String>>(10)?,
        "implementationBranchId":   r.get::<_, Option<String>>(11)?,
        "reviewBranchId":           r.get::<_, Option<String>>(12)?,
        "slug":                     r.get::<_, Option<String>>(13)?,
        "revision":                 r.get::<_, Option<i64>>(14)?,
        "versionMajor":             r.get::<_, Option<i64>>(15)?,
        "versionMinor":             r.get::<_, Option<i64>>(16)?,
        "createdAt":                r.get::<_, i64>(17)?,
        "updatedAt":                r.get::<_, i64>(18)?,
    }))
}

/// Load subtasks for a plan (id, planId, idx, title, details, status, outcome,
/// ownerAgent, lastUpdatedBy, createdAt, updatedAt). Returns empty vec on error.
fn load_subtasks(conn: &Connection, plan_id: &str) -> Vec<serde_json::Value> {
    let sql = "SELECT id, plan_id, idx, title, details, status, outcome, owner_agent, \
               last_updated_by, created_at, updated_at FROM plan_subtasks \
               WHERE plan_id = ?1 ORDER BY idx ASC";
    let mut stmt = match conn.prepare(sql) { Ok(s) => s, Err(_) => return Vec::new() };
    stmt.query_map([plan_id], |r| Ok(serde_json::json!({
        "id":             r.get::<_, String>(0)?,
        "planId":         r.get::<_, String>(1)?,
        "idx":            r.get::<_, i64>(2)?,
        "title":          r.get::<_, String>(3)?,
        "details":        r.get::<_, Option<String>>(4)?,
        "status":         r.get::<_, String>(5)?,
        "outcome":        r.get::<_, Option<String>>(6)?,
        "ownerAgent":     r.get::<_, Option<String>>(7)?,
        "lastUpdatedBy":  r.get::<_, Option<String>>(8)?,
        "createdAt":      r.get::<_, i64>(9)?,
        "updatedAt":      r.get::<_, i64>(10)?,
    })))
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

fn load_events(conn: &Connection, plan_id: &str) -> Vec<serde_json::Value> {
    let sql = "SELECT id, event_type, actor, detail, created_at FROM plan_events \
               WHERE plan_id = ?1 ORDER BY created_at ASC";
    let mut stmt = match conn.prepare(sql) { Ok(s) => s, Err(_) => return Vec::new() };
    stmt.query_map([plan_id], |r| Ok(serde_json::json!({
        "id":        r.get::<_, String>(0)?,
        "eventType": r.get::<_, String>(1)?,
        "actor":     r.get::<_, Option<String>>(2)?,
        "detail":    r.get::<_, Option<String>>(3)?,
        "createdAt": r.get::<_, i64>(4)?,
    })))
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// Parse `?include=subtasks,events` into booleans. Unknown tokens silently ignored.
fn parse_includes(include: &Option<String>) -> (bool, bool) {
    let Some(s) = include.as_deref() else { return (false, false) };
    let tokens: Vec<&str> = s.split(',').map(|t| t.trim()).filter(|t| !t.is_empty()).collect();
    (tokens.contains(&"subtasks"), tokens.contains(&"events"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDetailQuery {
    pub include: Option<String>,
}

pub async fn list_plans(
    State(state): State<ApiState>,
    Query(q): Query<PlanQuery>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let sql_with_conv = format!(
        "SELECT {PLAN_COLS} FROM plans WHERE conversation_id = ?1 ORDER BY created_at DESC"
    );
    let sql_without_conv = format!(
        "SELECT {PLAN_COLS} FROM plans ORDER BY created_at DESC LIMIT 20"
    );
    let sql = if q.conversation_id.is_some() { &sql_with_conv } else { &sql_without_conv };
    let mut stmt = match conn.prepare(sql) { Ok(s) => s, Err(e) => return db_error(e) };
    let rows: Vec<serde_json::Value> = if let Some(ref cid) = q.conversation_id {
        match stmt.query_map([cid], plan_row_to_json) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => return db_error(e),
        }
    } else {
        match stmt.query_map([], plan_row_to_json) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => return db_error(e),
        }
    };
    Json(serde_json::json!(rows)).into_response()
}

pub async fn get_plan(
    State(state): State<ApiState>,
    Path(plan_id): Path<String>,
    Query(q): Query<PlanDetailQuery>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let sql = format!("SELECT {PLAN_COLS} FROM plans WHERE id = ?1");
    let plan = conn.query_row(&sql, [&plan_id], plan_row_to_json);
    match plan {
        Ok(mut p) => {
            let (inc_sub, inc_ev) = parse_includes(&q.include);
            if inc_sub {
                if let Some(obj) = p.as_object_mut() {
                    obj.insert("subtasks".into(), serde_json::Value::Array(load_subtasks(&conn, &plan_id)));
                }
            }
            if inc_ev {
                if let Some(obj) = p.as_object_mut() {
                    obj.insert("events".into(), serde_json::Value::Array(load_events(&conn, &plan_id)));
                }
            }
            Json(p).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "plan not found"}))).into_response(),
    }
}

pub async fn list_plan_events(
    State(state): State<ApiState>,
    Path(plan_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let mut stmt = match conn.prepare(
        "SELECT id, event_type, actor, detail, created_at FROM plan_events WHERE plan_id = ?1 ORDER BY created_at ASC"
    ) { Ok(s) => s, Err(e) => return db_error(e) };
    let rows: Vec<serde_json::Value> = match stmt.query_map([&plan_id], |r| Ok(serde_json::json!({
        "id": r.get::<_, String>(0)?, "eventType": r.get::<_, String>(1)?,
        "actor": r.get::<_, Option<String>>(2)?, "detail": r.get::<_, Option<String>>(3)?,
        "createdAt": r.get::<_, i64>(4)?,
    }))) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => return db_error(e),
    };
    Json(serde_json::json!(rows)).into_response()
}

pub async fn approve_plan(
    State(state): State<ApiState>,
    Path(plan_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    let updated = conn.execute(
        "UPDATE plans SET status = 'active', phase = 'implementation' WHERE id = ?1 AND status != 'done'",
        [&plan_id],
    ).unwrap_or(0);
    if updated > 0 {
        let now = crate::db::migrations::now_epoch_ms();
        let event_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO plan_events (id, plan_id, event_type, actor, created_at) VALUES (?1, ?2, 'approved', 'api', ?3)",
            rusqlite::params![event_id, plan_id, now],
        ).ok();
        drop(conn);
        let _ = state.event_tx.send(serde_json::json!({
            "type": "plan:status_changed",
            "planId": plan_id, "toStatus": "active"
        }).to_string());
        let _ = state.event_tx.send(serde_json::json!({
            "type": "plan:phase_changed",
            "planId": plan_id, "toPhase": "implementation"
        }).to_string());
        Json(serde_json::json!({"approved": true, "planId": plan_id})).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "plan not found or already done"}))).into_response()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactQuery {
    pub conversation_id: Option<String>,
}

pub async fn reject_plan(
    State(state): State<ApiState>,
    Path(plan_id): Path<String>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    let updated = conn.execute(
        "UPDATE plans SET status = 'rejected', phase = 'done' WHERE id = ?1 AND status != 'done'",
        [&plan_id],
    ).unwrap_or(0);
    if updated > 0 {
        let now = crate::db::migrations::now_epoch_ms();
        let event_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO plan_events (id, plan_id, event_type, actor, created_at) VALUES (?1, ?2, 'rejected', 'api', ?3)",
            rusqlite::params![event_id, plan_id, now],
        ).ok();
        drop(conn);
        let _ = state.event_tx.send(serde_json::json!({
            "type": "plan:status_changed",
            "planId": plan_id, "toStatus": "rejected"
        }).to_string());
        let _ = state.event_tx.send(serde_json::json!({
            "type": "plan:phase_changed",
            "planId": plan_id, "toPhase": "done"
        }).to_string());
        Json(serde_json::json!({"rejected": true, "planId": plan_id})).into_response()
    } else {
        (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "plan not found or already done"}))).into_response()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSubtaskStatusInput {
    pub status: String,
    pub outcome: Option<String>,
    pub updated_by: Option<String>,
}

/// Phase 2 Finding 2-4: change a subtask's status over HTTP and
/// broadcast `plan:subtask_status_changed` to WS subscribers so mobile
/// clients don't need to poll.
pub async fn update_subtask_status(
    State(state): State<ApiState>,
    Path((plan_id, subtask_id)): Path<(String, String)>,
    Json(input): Json<UpdateSubtaskStatusInput>,
) -> impl IntoResponse {
    // Guard: the known states match the desktop enum in `types/index.ts`.
    // Reject unknown values up front instead of letting them land in the
    // DB where they'd confuse every reader.
    const VALID: &[&str] = &["todo", "approved", "in_progress", "done", "abandoned"];
    if !VALID.contains(&input.status.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid status", "allowed": VALID})),
        )
            .into_response();
    }
    // Verify the subtask actually belongs to this plan — `update_subtask_status`
    // on the Tauri command path doesn't enforce this because the desktop
    // UI can't send a mismatched pair, but HTTP callers can.
    let conn = lock_conn(&state.db.write);
    let owner_check: Result<String, _> = conn.query_row(
        "SELECT plan_id FROM plan_subtasks WHERE id = ?1",
        [&subtask_id],
        |r| r.get(0),
    );
    let owner = match owner_check {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "subtask not found"})),
            )
                .into_response()
        }
    };
    if owner != plan_id {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "subtask does not belong to plan"})),
        )
            .into_response();
    }
    let now = crate::db::migrations::now_epoch_ms();
    if let Err(e) = conn.execute(
        "UPDATE plan_subtasks SET status = ?1, outcome = ?2, last_updated_by = ?3, updated_at = ?4 WHERE id = ?5",
        rusqlite::params![input.status, input.outcome, input.updated_by, now, subtask_id],
    ) {
        return db_error(e);
    }
    drop(conn);

    let _ = state.event_tx.send(
        serde_json::json!({
            "type": "plan:subtask_status_changed",
            "planId": plan_id,
            "subtaskId": subtask_id,
            "status": input.status,
        })
        .to_string(),
    );

    Json(serde_json::json!({
        "planId": plan_id,
        "subtaskId": subtask_id,
        "status": input.status,
        "updatedAt": now,
    }))
    .into_response()
}

pub async fn list_artifacts(
    State(state): State<ApiState>,
    Query(q): Query<ArtifactQuery>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let sql = if q.conversation_id.is_some() {
        "SELECT id, conversation_id, type, title, status FROM artifacts WHERE conversation_id = ?1 ORDER BY created_at DESC"
    } else {
        "SELECT id, conversation_id, type, title, status FROM artifacts ORDER BY created_at DESC LIMIT 20"
    };
    let mut stmt = match conn.prepare(sql) { Ok(s) => s, Err(e) => return db_error(e) };
    let rows: Vec<serde_json::Value> = if let Some(ref cid) = q.conversation_id {
        match stmt.query_map([cid], |r| Ok(serde_json::json!({
            "id": r.get::<_, String>(0)?, "conversationId": r.get::<_, String>(1)?,
            "type": r.get::<_, String>(2)?, "title": r.get::<_, String>(3)?,
            "status": r.get::<_, String>(4)?,
        }))) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => return db_error(e),
        }
    } else {
        match stmt.query_map([], |r| Ok(serde_json::json!({
            "id": r.get::<_, String>(0)?, "conversationId": r.get::<_, String>(1)?,
            "type": r.get::<_, String>(2)?, "title": r.get::<_, String>(3)?,
            "status": r.get::<_, String>(4)?,
        }))) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(e) => return db_error(e),
        }
    };
    Json(serde_json::json!(rows)).into_response()
}
