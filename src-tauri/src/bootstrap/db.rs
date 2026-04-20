//! DB path resolution, connection init, stale-state cleanup, and `DbState`
//! registration. Split out of `lib.rs` so startup failures surface per-step.

use tauri::Manager;

use crate::db::{self, DbState};

/// Resolve DB path, initialize connections, clean up stale streaming state
/// from a previous crash/shutdown, and register `DbState` on the app.
pub fn init_db(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = resolve_db_path(app)?;
    eprintln!("[bootstrap/db] path: {}", db_path.display());

    let (write_conn, read_conn) = db::init(db_path)?;

    // Cleanup stale streaming messages from previous crash/shutdown.
    let cleaned = write_conn
        .execute(
            "UPDATE messages SET status = 'error', content = CASE WHEN content = '' THEN '(이전 세션에서 중단됨)' ELSE content END WHERE status = 'streaming'",
            [],
        )
        .unwrap_or_else(|e| {
            eprintln!("[bootstrap/db] stale message cleanup failed: {e}");
            0
        });
    let jobs = write_conn
        .execute(
            "UPDATE agent_jobs SET status = 'failed', error = 'app restart' WHERE status = 'running'",
            [],
        )
        .unwrap_or_else(|e| {
            eprintln!("[bootstrap/db] stale job cleanup failed: {e}");
            0
        });
    if cleaned > 0 || jobs > 0 {
        eprintln!(
            "[bootstrap/db] Cleaned {} stale streaming messages, {} stale jobs",
            cleaned, jobs
        );
    }

    app.manage(DbState {
        write: std::sync::Arc::new(std::sync::Mutex::new(write_conn)),
        read: std::sync::Arc::new(std::sync::Mutex::new(read_conn)),
    });

    Ok(())
}

/// DB storage strategy:
/// - dev     (debug build):  `~/.tunaflow/db/tunaflow.db`
///   AppCleaner searches by bundle id (com.tunaflow.app) so anything
///   under Application Support/<bundle-id>/ gets wiped when the .app
///   is deleted. We already lost a 37M DB this way. Moving the dev
///   DB under `~/.tunaflow/` (dotfile, not matched by AppCleaner)
///   keeps real work safe across app reinstalls.
/// - release (release build): Application Support/<bundle-id>/tunaflow.db
///   Intentionally inside the bundle-id folder so that AppCleaner
///   (and scripts/build.sh --wipe-sandbox) can reset it on every
///   install, giving a fresh onboarding surface every build.
fn resolve_db_path(app: &tauri::App) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let path = if cfg!(debug_assertions) {
        let home = std::env::var("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let dir = home.join(".tunaflow").join("db");
        std::fs::create_dir_all(&dir)?;
        dir.join("tunaflow.db")
    } else {
        let dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from(".tunaflow_data"));
        std::fs::create_dir_all(&dir)?;
        dir.join("tunaflow.db")
    };
    Ok(path)
}
