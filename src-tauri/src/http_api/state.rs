//! Project/state endpoints + document RAG endpoints.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;

use super::{ApiState, db_error, lock_conn, with_read_db};

pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")}))
}

pub async fn list_projects(State(state): State<ApiState>) -> impl IntoResponse {
    match with_read_db(&state, |conn| {
        let mut stmt = conn.prepare(
            "SELECT key, name, path, type FROM projects WHERE hidden = 0 ORDER BY name"
        ).map_err(|e| e.to_string())?;
        let rows: Vec<serde_json::Value> = stmt.query_map([], |r| {
            Ok(serde_json::json!({
                "key": r.get::<_, String>(0)?,
                "name": r.get::<_, String>(1)?,
                "path": r.get::<_, Option<String>>(2)?,
                "type": r.get::<_, String>(3)?,
            }))
        }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }).await {
        Ok(rows) => Json(serde_json::json!(rows)).into_response(),
        Err(resp) => resp,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    pub key: String,
    pub name: String,
    pub path: Option<String>,
}

pub async fn create_project(
    State(state): State<ApiState>,
    Json(input): Json<CreateProjectInput>,
) -> impl IntoResponse {
    let conn = lock_conn(&state.db.write);
    let now = crate::db::migrations::now_epoch_ms();
    if let Err(e) = conn.execute(
        "INSERT OR IGNORE INTO projects (key, name, path, type, source, hidden, updated_at) VALUES (?1, ?2, ?3, 'project', 'api', 0, ?4)",
        rusqlite::params![input.key, input.name, input.path, now],
    ) {
        return db_error(e);
    }
    (StatusCode::CREATED, Json(serde_json::json!({"key": input.key, "name": input.name, "path": input.path}))).into_response()
}

// ─── Document RAG endpoints ───────────────────────────────────────────

pub async fn index_project_documents(
    State(state): State<ApiState>,
    Path(project_key): Path<String>,
) -> impl IntoResponse {
    let db = state.db.clone();
    let pk = project_key.clone();

    let project_path = match with_read_db(&state, move |conn| {
        conn.query_row("SELECT path FROM projects WHERE key = ?1", [&pk], |r| r.get::<_, Option<String>>(0))
            .map_err(|e| format!("project lookup: {}", e))?
            .ok_or_else(|| "project has no path".to_string())
    }).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    let event_tx = state.event_tx.clone();
    let pk2 = project_key.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::commands::document_index::index_project_documents(&db, &project_key, &project_path)
        }));
        match result {
            Ok(Ok(r)) => {
                eprintln!("[doc-index] completed: files={}, chunks={}, edges={}, errors={}",
                    r.files_indexed, r.chunks_created, r.edges_created, r.errors.len());
                if !r.errors.is_empty() {
                    for e in &r.errors[..r.errors.len().min(5)] { eprintln!("[doc-index]   error: {}", e); }
                }
                super::broadcast_event(&event_tx, &db, serde_json::json!({
                    "type": "document:indexed", "projectKey": pk2, "result": r,
                }));
            }
            Ok(Err(e)) => {
                eprintln!("[doc-index] failed: {}", e);
                super::broadcast_event(&event_tx, &db, serde_json::json!({
                    "type": "document:error", "projectKey": pk2, "error": e.to_string(),
                }));
            }
            Err(panic_err) => {
                let msg = panic_err.downcast_ref::<String>()
                    .map(|s| s.as_str())
                    .or_else(|| panic_err.downcast_ref::<&str>().copied())
                    .unwrap_or("unknown panic");
                eprintln!("[doc-index] PANIC: {}", msg);
            }
        }
    });

    (StatusCode::ACCEPTED, Json(serde_json::json!({
        "status": "indexing",
        "info": "Document indexing started in background. Listen on /ws/events for document:indexed event.",
    }))).into_response()
}

#[derive(Deserialize)]
pub struct DocumentSearchInput {
    pub query: String,
    pub limit: Option<usize>,
}

pub async fn search_project_documents(
    State(state): State<ApiState>,
    Path(project_key): Path<String>,
    Json(input): Json<DocumentSearchInput>,
) -> impl IntoResponse {
    let db = state.db.clone();
    match tokio::task::spawn_blocking(move || {
        crate::commands::document_index::search_documents(&db, &project_key, &input.query, input.limit.unwrap_or(10))
    }).await {
        Ok(Ok(results)) => (StatusCode::OK, Json(serde_json::json!(results))).into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
        Err(e) => db_error(format!("task join: {}", e)),
    }
}

pub async fn get_document_graph(
    State(state): State<ApiState>,
    Path(project_key): Path<String>,
) -> impl IntoResponse {
    match with_read_db(&state, move |conn| {
        Ok(crate::commands::document_index::get_document_graph(conn, &project_key))
    }).await {
        Ok(edges) => (StatusCode::OK, Json(serde_json::json!(edges))).into_response(),
        Err(resp) => resp,
    }
}

pub async fn get_orphan_documents(
    State(state): State<ApiState>,
    Path(project_key): Path<String>,
) -> impl IntoResponse {
    match with_read_db(&state, move |conn| {
        Ok(crate::commands::document_index::find_orphan_documents(conn, &project_key))
    }).await {
        Ok(orphans) => (StatusCode::OK, Json(serde_json::json!(orphans))).into_response(),
        Err(resp) => resp,
    }
}

pub async fn get_document_index_status(
    State(state): State<ApiState>,
    Path(project_key): Path<String>,
) -> impl IntoResponse {
    match with_read_db(&state, move |conn| {
        Ok(crate::commands::document_index::get_index_status(conn, &project_key))
    }).await {
        Ok(status) => (StatusCode::OK, Json(serde_json::json!(status))).into_response(),
        Err(resp) => resp,
    }
}
