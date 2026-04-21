//! WebSocket event broadcasting with persistent log + replay (Finding 2-6).
//!
//! Every outbound WS event goes through `broadcast_event`, which
//!   1. appends the payload to `ws_event_log` (best-effort DB write), and
//!   2. fans it out to the live `tokio::sync::broadcast` channel.
//!
//! Mobile clients that reconnect with `?since=<ms>` ask the server to
//! dump everything from the log newer than that timestamp before the
//! live subscription begins. See `http_api::ws::handle_ws`.
//!
//! Non-goals: at-least-once guarantees, exactly-once, or distributed
//! replay. The log is a convenience for short reconnect windows
//! (default retention 24h, trimmed hourly by `spawn_ttl_cleanup`).
use tokio::sync::broadcast;

use crate::db::DbState;

/// How long rows live in `ws_event_log` before the background TTL
/// cleanup removes them. 24h is comfortably longer than realistic
/// mobile reconnect windows; bumping this trades storage for coverage.
pub const WS_EVENT_LOG_RETENTION_MS: i64 = 24 * 3600 * 1000;

/// Interval at which the TTL cleanup task runs. 1h is frequent enough
/// that the table never drifts much past the retention window, and
/// sparse enough to be invisible on the write-lock profile.
pub const WS_EVENT_LOG_CLEANUP_INTERVAL: std::time::Duration =
    std::time::Duration::from_secs(3600);

/// Upper bound on rows `/ws/events?since=` replays in one connection.
/// A burst of thousands of events within the reconnect window is
/// unusual; capping at 2 000 keeps the initial replay bounded while
/// still covering reasonable multi-minute gaps.
pub const WS_REPLAY_MAX_ROWS: i64 = 2_000;

/// Persist the event to `ws_event_log` and broadcast it to every
/// currently-connected WebSocket subscriber. DB write is best-effort —
/// a lock contention miss is logged to stderr but never blocks the
/// broadcast.
///
/// Callers pass a `serde_json::Value` rather than a pre-stringified
/// payload so the helper can read the `type` field for the column
/// (avoids re-parsing JSON for a 1-depth lookup).
pub fn broadcast_event(
    tx: &broadcast::Sender<String>,
    db: &DbState,
    payload: serde_json::Value,
) {
    let event_type = payload
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let json_str = payload.to_string();
    let now = crate::db::migrations::now_epoch_ms();
    match db.write.lock() {
        Ok(conn) => {
            if let Err(e) = conn.execute(
                "INSERT INTO ws_event_log (event_type, payload, created_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![event_type, json_str, now],
            ) {
                eprintln!("[ws-event-log] insert failed (type={event_type}): {e}");
            }
        }
        Err(e) => {
            eprintln!("[ws-event-log] lock poisoned: {e}");
        }
    }
    let _ = tx.send(json_str);
}

/// Fetch recent events newer than `since_ms` for WS replay. Ordered
/// oldest-first so the client receives events in their original
/// emission order. Capped at `WS_REPLAY_MAX_ROWS`.
pub fn fetch_events_since(db: &DbState, since_ms: i64) -> Vec<String> {
    let conn = match db.read.lock() {
        Ok(c) => c,
        Err(p) => p.into_inner(),
    };
    let mut stmt = match conn.prepare(
        "SELECT payload FROM ws_event_log
         WHERE created_at >= ?1
         ORDER BY created_at ASC, id ASC
         LIMIT ?2",
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[ws-event-log] fetch_events_since prepare failed: {e}");
            return Vec::new();
        }
    };
    stmt.query_map(rusqlite::params![since_ms, WS_REPLAY_MAX_ROWS], |r| {
        r.get::<_, String>(0)
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// Spawn a background task that trims rows older than
/// `WS_EVENT_LOG_RETENTION_MS` every `WS_EVENT_LOG_CLEANUP_INTERVAL`.
/// Runs for the lifetime of the server; Tauri shuts it down with the
/// process.
///
/// Uses `tauri::async_runtime::spawn` so callers can invoke this from
/// the main thread during `start_server` — before the axum task has
/// created its own runtime. Plain `tokio::spawn` would panic here
/// ("no reactor running") because `start_server` runs synchronously
/// on the bootstrap thread.
pub fn spawn_ttl_cleanup(db: DbState) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(WS_EVENT_LOG_CLEANUP_INTERVAL).await;
            let cutoff = crate::db::migrations::now_epoch_ms() - WS_EVENT_LOG_RETENTION_MS;
            let deleted = match db.write.lock() {
                Ok(conn) => conn
                    .execute("DELETE FROM ws_event_log WHERE created_at < ?1", [cutoff])
                    .unwrap_or(0),
                Err(p) => p
                    .into_inner()
                    .execute("DELETE FROM ws_event_log WHERE created_at < ?1", [cutoff])
                    .unwrap_or(0),
            };
            if deleted > 0 {
                eprintln!("[ws-event-log] trimmed {deleted} rows older than 24h");
            }
        }
    });
}
