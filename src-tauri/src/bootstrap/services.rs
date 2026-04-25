//! Background services started at app bootstrap: HTTP API server, PTY /
//! rawq-indexing state registration, rawq daemon, bge-m3 embedder, orphaned
//! agent-process cleanup, and the vector backfill job.

use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::Mutex;
use tauri::Manager;

use crate::commands;
use crate::db::DbState;
use crate::http_api;

/// Start every background service. The `cancel_arc` is shared with HTTP API
/// handlers so external callers (e.g. mobile clients) can request cancellation
/// of running agent runs.
pub fn start_background_services(
    app: &tauri::App,
    cancel_arc: Arc<Mutex<HashSet<String>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. HTTP API server (E2E testing + mobile access + MCP)
    {
        let db_state = app.state::<DbState>().inner().clone();
        let api_token = http_api::start_server(db_state, app.handle().clone(), cancel_arc);
        eprintln!("[bootstrap/services] HTTP API token: {}", api_token);
    }

    // 2. Per-process state registration
    app.manage(commands::pty::PtyState::new());
    app.manage(commands::projects::RawqIndexing::new());

    // 3. Start rawq daemon in background — pre-loads embedding model for fast
    //    indexing/search.
    std::thread::spawn(|| {
        crate::agents::rawq::ensure_daemon();
    });

    // 4. Initialize bge-m3 embedder (document/conversation search). Try sync
    //    init first (if model already cached), then async download if needed.
    if let Err(e) = crate::agents::embedder::init_global_embedder() {
        eprintln!("[bootstrap/services] bge-m3 sync init error: {}", e);
    }
    if crate::agents::embedder::get_embedder().is_none() {
        tauri::async_runtime::spawn(async {
            if let Err(e) = crate::agents::embedder::init_global_embedder_async().await {
                eprintln!(
                    "[bootstrap/services] bge-m3 async download/init error: {}",
                    e
                );
            }
        });
    }

    // 5. Kill orphaned sdk-url/app-server processes from previous runs.
    //    These can silently consume rate limit quota if left alive.
    crate::agents::claude_sdk_session::kill_orphan_sdk_processes();

    // 6. Backfill NULL-embedding chunks left over from v32 (bge-m3 migration).
    //    Sleeps 15s before starting to let embedder/rawq settle, then processes
    //    one conversation/project at a time with throttling.
    crate::commands::vector_search::spawn_startup_backfill(
        app.state::<DbState>().inner().clone(),
    );

    // 7. metaAgent Phase 4 — background insight worker loop. 30s tick,
    //    concurrency=1. foreground busy 시 자연 양보 (INV-6). Settings 토글
    //    (`background_insight_enabled`, INV-3) 로 OFF 가능.
    {
        let db_arc = Arc::new(app.state::<DbState>().inner().clone());
        crate::commands::meta_agent::background_worker::spawn_background_worker(
            app.handle().clone(),
            db_arc,
        );
    }

    Ok(())
}
