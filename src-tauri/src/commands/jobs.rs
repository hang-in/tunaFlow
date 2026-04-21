use rusqlite::params;
use serde::Serialize;
use tauri::State;

use crate::db::{migrations::now_epoch_ms, DbState};
use crate::errors::AppError;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentJob {
    pub id: String,
    pub conversation_id: String,
    pub message_id: Option<String>,
    pub engine: String,
    pub kind: String,
    pub status: String,
    pub error: Option<String>,
    pub started_at: i64,
    pub updated_at: i64,
}

/// Create a new job record. Called from start_* commands.
pub fn create_job(
    conn: &rusqlite::Connection,
    id: &str,
    conversation_id: &str,
    message_id: Option<&str>,
    engine: &str,
    kind: &str,
) -> Result<(), AppError> {
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO agent_jobs (id, conversation_id, message_id, engine, kind, status, started_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'running', ?6, ?6)",
        params![id, conversation_id, message_id, engine, kind, now],
    )?;
    Ok(())
}

/// Update job status. Called from background threads on completion/error.
pub fn complete_job(
    conn: &rusqlite::Connection,
    id: &str,
    status: &str,
    error: Option<&str>,
) -> Result<(), AppError> {
    let now = now_epoch_ms();
    conn.execute(
        "UPDATE agent_jobs SET status = ?1, error = ?2, updated_at = ?3 WHERE id = ?4",
        params![status, error, now, id],
    )?;
    Ok(())
}

/// List active (running) jobs. Used by frontend to detect in-progress work.
#[tauri::command]
pub fn list_active_jobs(state: State<DbState>) -> Result<Vec<AgentJob>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, message_id, engine, kind, status, error, started_at, updated_at
         FROM agent_jobs WHERE status = 'running' ORDER BY started_at DESC",
    )?;
    let jobs = stmt.query_map([], |row| {
        Ok(AgentJob {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            message_id: row.get(2)?,
            engine: row.get(3)?,
            kind: row.get(4)?,
            status: row.get(5)?,
            error: row.get(6)?,
            started_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    })?.filter_map(|r| r.ok()).collect();
    Ok(jobs)
}

/// Post-completion background tasks for a conversation.
/// Replaces the frontend setTimeout chain (compress → session_links → index_chunks → rawq_index).
/// Returns immediately; all work is done in background threads.
#[tauri::command]
pub fn on_run_completed(conversation_id: String, state: State<DbState>) -> Result<(), AppError> {
    let db = state.inner().clone();

    // 1–3: memory compression + session links + vector chunks (existing helper)
    let db_clone = db.clone();
    let cid = conversation_id.clone();
    crate::commands::agents_helpers::send_common::spawn_post_completion_tasks(db_clone, cid);

    // 4: rawq code-search index rebuild (best-effort; skips if daemon not ready)
    if crate::agents::rawq::is_daemon_ready() {
        let db_rawq = db;
        let cid_rawq = conversation_id;
        std::thread::spawn(move || {
            let project_path: Option<String> = db_rawq.read.lock().ok().and_then(|conn| {
                conn.query_row(
                    "SELECT p.path FROM projects p \
                     JOIN conversations c ON c.project_key = p.key WHERE c.id = ?1",
                    [&cid_rawq], |r| r.get::<_, Option<String>>(0),
                ).ok().flatten()
            });
            if let Some(path) = project_path {
                if let Err(e) = crate::agents::rawq::ensure_index(&path) {
                    eprintln!("[on_run_completed] rawq index error: {}", e);
                }
            }
        });
    }

    Ok(())
}

/// Cleanup stale jobs: mark 'running' jobs as 'stale' and fix orphaned streaming messages.
/// Called on app startup and webview reload to recover from interrupted runs.
///
/// ⚠️ async + spawn_blocking — webview 리로드 시 AppShell init 에서 invoke
/// 되는데 sync 버전은 main thread 에서 write lock 대기 → beach ball. 이전 turn
/// 의 post-completion hook (vector indexing) 이 lock hold 중이면 정확히 재현.
/// 2026-04-22 sample(1) stack 으로 재확인.
#[tauri::command]
pub async fn cleanup_stale_jobs(state: State<'_, DbState>) -> Result<i64, AppError> {
    let write = state.write.clone();
    tokio::task::spawn_blocking(move || -> Result<i64, AppError> {
        let conn = write.lock().map_err(|_| AppError::Lock)?;
        let now = now_epoch_ms();

        // Mark all running jobs as stale
        let job_count = conn.execute(
            "UPDATE agent_jobs SET status = 'stale', updated_at = ?1 WHERE status = 'running'",
            params![now],
        )?;

        // Fix orphaned streaming messages (from interrupted background threads)
        conn.execute(
            "UPDATE messages SET status = 'error', content = CASE WHEN content = '' THEN '(interrupted)' ELSE content END
             WHERE status = 'streaming'",
            [],
        )?;

        Ok(job_count as i64)
    })
    .await
    .map_err(|e| AppError::Agent(format!("spawn_blocking failed: {}", e)))?
}
