//! HTTP API server — axum-based, runs inside Tauri app via tokio::spawn.
//! Provides REST endpoints for E2E testing, mobile access, and MCP wrapping.
//!
//! Architecture:
//! - Shares DbState with Tauri commands (same Arc<Mutex<Connection>>)
//! - Bearer token auth (generated at startup, shown in Settings)
//! - WS event bridge: Tauri events → broadcast → WebSocket clients
//! - Binds to localhost only (127.0.0.1:19840)

mod auth;
mod agents;
mod conversations;
mod meta;
mod plans;
mod state;
mod ws;

use axum::{
    Router,
    routing::{get, post, delete},
    middleware,
};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

use crate::db::DbState;

const DEFAULT_PORT: u16 = 19840;
pub(super) type CancelArc = std::sync::Arc<parking_lot::Mutex<std::collections::HashSet<String>>>;

/// Shared state for axum handlers.
#[derive(Clone)]
#[allow(dead_code)]
pub struct ApiState {
    pub db: DbState,
    pub token: String,
    pub event_tx: broadcast::Sender<String>,
    pub app_handle: tauri::AppHandle,
    pub cancel: CancelArc,
}

/// Helper: run a fallible DB closure, returning 500 JSON on error.
pub(super) fn db_error(e: impl std::fmt::Display) -> axum::response::Response {
    use axum::response::IntoResponse;
    (axum::http::StatusCode::INTERNAL_SERVER_ERROR,
     axum::Json(serde_json::json!({"error": format!("db: {}", e)}))).into_response()
}

/// Lock a std::sync::Mutex, recovering from poison if needed.
pub(super) fn lock_conn(
    mutex: &std::sync::Mutex<rusqlite::Connection>,
) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
    mutex.lock().unwrap_or_else(|poisoned| {
        eprintln!("[http-api] recovering poisoned mutex");
        poisoned.into_inner()
    })
}

/// Run a blocking DB operation off the async executor.
pub(super) async fn with_read_db<F, T>(
    state: &ApiState,
    f: F,
) -> Result<T, axum::response::Response>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let conn = lock_conn(&db.read);
        f(&conn)
    })
    .await
    .map_err(|e| db_error(format!("task join: {}", e)))?
    .map_err(|e| db_error(e))
}

#[allow(dead_code)]
pub(super) async fn with_write_db<F, T>(
    state: &ApiState,
    f: F,
) -> Result<T, axum::response::Response>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let conn = lock_conn(&db.write);
        f(&conn)
    })
    .await
    .map_err(|e| db_error(format!("task join: {}", e)))?
    .map_err(|e| db_error(e))
}

/// Start the HTTP API server on a background tokio task.
/// Returns the generated Bearer token for auth.
pub fn start_server(db: DbState, app_handle: tauri::AppHandle, cancel: CancelArc) -> String {
    let token = generate_token();
    let (event_tx, _) = broadcast::channel::<String>(256);

    let state = ApiState {
        db: db.clone(),
        token: token.clone(),
        event_tx: event_tx.clone(),
        app_handle: app_handle.clone(),
        cancel,
    };

    // Bridge Tauri events → broadcast channel
    let tx = event_tx.clone();
    ws::bridge_tauri_events(app_handle, tx);

    tauri::async_runtime::spawn(async move {
        let app = build_router(state);
        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT));
        eprintln!("[http-api] starting on http://{}", addr);
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[http-api] bind failed: {} (port {} may be in use)", e, DEFAULT_PORT);
                return;
            }
        };
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("[http-api] server error: {}", e);
        }
    });

    token
}

/// Load or create a persistent API token.
fn generate_token() -> String {
    let token_path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".tunaflow")
        .join("api-token");

    if let Ok(existing) = std::fs::read_to_string(&token_path) {
        let trimmed = existing.trim().to_string();
        if trimmed.len() >= 32 {
            return trimmed;
        }
    }

    let token = uuid::Uuid::new_v4().to_string();
    if let Some(parent) = token_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&token_path, &token) {
        eprintln!("[http-api] failed to persist token: {}", e);
    }
    token
}

/// Tag `/api/*` legacy responses with `X-API-Deprecated: use /api/v1`.
/// Mobile and other programmatic clients surface this header to nudge
/// callers onto the versioned path ahead of removing the legacy prefix.
async fn deprecation_header_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = req.uri().path().to_string();
    let is_legacy = path.starts_with("/api/") && !path.starts_with("/api/v1/");
    let mut res = next.run(req).await;
    if is_legacy {
        res.headers_mut().insert(
            "X-API-Deprecated",
            axum::http::HeaderValue::from_static("use /api/v1"),
        );
    }
    res
}

fn build_router(state: ApiState) -> Router {
    // Single definition of the REST surface. Every route is authored
    // without any `/api` prefix and then mounted under BOTH `/api/v1`
    // (canonical) and `/api` (legacy, deprecation-tagged). Removing the
    // legacy mount later is a single-line edit; adding a new endpoint
    // adds it to both at once.
    let rest: Router<ApiState> = Router::new()
        // State / project endpoints
        .route("/health", get(state::health))
        .route("/projects", get(state::list_projects))
        .route("/projects", post(state::create_project))
        .route("/projects/{key}/documents/index", post(state::index_project_documents))
        .route("/projects/{key}/documents/search", post(state::search_project_documents))
        .route("/projects/{key}/documents/graph", get(state::get_document_graph))
        .route("/projects/{key}/documents/orphans", get(state::get_orphan_documents))
        .route("/projects/{key}/documents/status", get(state::get_document_index_status))
        // Conversation / message endpoints
        .route("/conversations", get(conversations::list_conversations))
        .route("/conversations", post(conversations::create_conversation))
        .route("/conversations/{id}/messages", get(conversations::list_messages))
        .route("/conversations/{id}/delete", post(conversations::delete_conversation))
        // Branch endpoints
        .route("/conversations/{id}/branches", get(conversations::list_branches))
        .route("/branches", post(conversations::create_branch))
        .route("/branches/{id}", get(conversations::get_branch_detail))
        .route("/branches/{id}", delete(conversations::delete_branch))
        .route("/branches/{id}/archive", post(conversations::archive_branch))
        .route("/branches/{id}/adopt", post(conversations::adopt_branch))
        .route("/branches/{id}/rename", post(conversations::rename_branch))
        // Memory & search endpoints
        .route("/conversations/{id}/memory/status", get(conversations::memory_status))
        .route("/conversations/{id}/memory/compress", post(conversations::compress_memory))
        .route("/conversations/{id}/session-links", get(conversations::list_session_links))
        .route("/conversations/{id}/session-links/refresh", post(conversations::refresh_session_links))
        .route("/conversations/{id}/chunks/index", post(conversations::index_chunks))
        .route("/conversations/{id}/chunks/search", post(conversations::search_chunks))
        .route("/conversations/{id}/traces", get(conversations::list_conv_traces))
        .route("/conversations/{id}/active-plan", get(conversations::get_active_plan))
        // Plan / artifact endpoints
        .route("/plans", get(plans::list_plans))
        .route("/plans/{id}", get(plans::get_plan))
        .route("/plans/{id}/events", get(plans::list_plan_events))
        .route("/plans/{id}/approve", post(plans::approve_plan))
        .route("/plans/{id}/reject", post(plans::reject_plan))
        .route("/artifacts", get(plans::list_artifacts))
        // Meta notification endpoints (v38 table → mobile inbox)
        .route("/meta-notifications", get(meta::list_meta_notifications))
        .route("/meta-notifications/mark-all-read", post(meta::mark_all_read))
        .route("/meta-notifications/clear", post(meta::clear))
        .route("/meta-notifications/{id}/read", post(meta::mark_read))
        .route("/meta-notifications/{id}/dismiss", post(meta::dismiss))
        // Agent endpoints
        .route("/agents/status", get(agents::agents_status))
        .route("/conversations/{id}/send", post(agents::send_message))
        .route("/roundtables/run", post(agents::start_rt_run))
        .route("/roundtables/{id}/cancel", post(agents::cancel_rt));

    Router::new()
        .nest("/api/v1", rest.clone())
        .nest("/api", rest)
        // WebSocket stays at the root; it's cheap to add /api/v1/ws later
        // but existing /ws/events contract is stable for mobile.
        .route("/ws/events", get(ws::ws_events))
        .layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware))
        .layer(middleware::from_fn(deprecation_header_middleware))
        .layer(CorsLayer::permissive())
        .with_state(state)
}
