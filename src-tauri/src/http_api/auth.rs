//! Bearer token auth middleware.

use axum::{
    extract::State,
    http::{StatusCode, HeaderMap},
    middleware,
    response::{IntoResponse, Json},
};

use super::ApiState;

pub async fn auth_middleware(
    State(state): State<ApiState>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: middleware::Next,
) -> impl IntoResponse {
    // Skip this middleware for:
    //   - /api/health (public probe)
    //   - /ws/events (handles its own auth, accepting either the Authorization
    //     header OR a `?token=` query param; see `ws.rs:24-35`). Browsers
    //     cannot attach custom headers to a WebSocket handshake, so the query
    //     path is the only way mobile clients can authenticate. Running this
    //     header-only middleware in front of `ws_events` short-circuits with
    //     401 before the handler's query-parsing logic can ever execute,
    //     turning the ws.rs code path into dead code for query callers.
    let path = request.uri().path();
    if path == "/api/health" || path == "/ws/events" {
        return next.run(request).await;
    }

    let auth = headers.get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if auth == format!("Bearer {}", state.token) {
        next.run(request).await
    } else {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid token"}))).into_response()
    }
}
