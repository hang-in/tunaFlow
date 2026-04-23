//! metaAgent Phase 4 — background job enqueue / cancel / 조회.
//!
//! Schema 는 v46 에서 `agent_jobs` 에 추가된 `priority`, `dedupe_key`,
//! `visibility` 를 사용한다. `priority = -1` 만 background worker 대상.
//!
//! Invariants:
//! - INV-3: `background_insight_enabled` 가 OFF 면 enqueue 는 허용하되 worker 가 pick
//!   하지 않음. 과거 큐 보존 — 사용자가 재활성화하면 재개.
//! - INV-6: worker 는 foreground job 진행 중이면 양보 (pick 금지).

use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::{params, Connection};
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use crate::db::{migrations::now_epoch_ms, DbState};
use crate::errors::AppError;

/// INV-3: 프로젝트 전반 toggle. 기본 ON. OFF 시 worker loop 가 pick 금지.
/// 프로젝트별 토글은 후속 PR (Settings UI) 에서 persist 레이어 추가.
pub static BACKGROUND_INSIGHT_ENABLED: AtomicBool = AtomicBool::new(true);

/// Worker 폴링 주기. 30 초 (metaAgentPlan P4-3).
pub const WORKER_POLL_INTERVAL_SECS: u64 = 30;

/// Worker 가 대상으로 하는 priority 값.
pub const BG_PRIORITY: i64 = -1;

#[derive(Debug, Clone)]
pub struct BackgroundJob {
    pub id: String,
    pub conversation_id: Option<String>,
    pub engine: String,
    pub kind: String,
    pub status: String,
    pub priority: i64,
    pub dedupe_key: Option<String>,
    pub visibility: String,
    pub started_at: i64,
}

/// Background job 삽입 — `dedupe_key` 가 있고 같은 키의 pending/running 이 이미
/// 있으면 skip 후 `Ok(None)`. 없으면 새 id 로 insert.
///
/// Caller (metaAgent Phase 3, 또는 향후 외부 모듈) 는 이 함수를 통해 bg 작업을
/// 예약한다. visibility 기본값은 `"visible"` (StatusBar 노출).
pub fn enqueue_background_job(
    conn: &Connection,
    conversation_id: Option<&str>,
    engine: &str,
    kind: &str,
    dedupe_key: Option<&str>,
    visibility: &str,
) -> Result<Option<String>, AppError> {
    // dedupe 체크 — 같은 kind + dedupe_key + 미완료 상태면 skip
    if let Some(key) = dedupe_key {
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM agent_jobs \
                 WHERE kind = ?1 \
                   AND dedupe_key = ?2 \
                   AND priority = ?3 \
                   AND status IN ('pending', 'running') \
                 LIMIT 1",
                params![kind, key, BG_PRIORITY],
                |row| row.get(0),
            )
            .ok();
        if let Some(prior) = existing {
            eprintln!(
                "[bg-job] dedup skip kind={} key={} prior_id={}",
                kind, key, prior
            );
            return Ok(None);
        }
    }

    let id = format!("job-{}", Uuid::new_v4());
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO agent_jobs \
           (id, conversation_id, message_id, engine, kind, status, started_at, updated_at, priority, dedupe_key, visibility) \
         VALUES (?1, ?2, NULL, ?3, ?4, 'pending', ?5, ?5, ?6, ?7, ?8)",
        params![id, conversation_id, engine, kind, now, BG_PRIORITY, dedupe_key, visibility],
    )?;
    Ok(Some(id))
}

/// 다음 background job 을 하나 pick. priority=-1, status='pending', FIFO (started_at ASC).
/// Worker loop 가 매 tick 호출.
pub fn pick_next_background_job(conn: &Connection) -> Result<Option<BackgroundJob>, AppError> {
    let row: Option<BackgroundJob> = conn
        .query_row(
            "SELECT id, conversation_id, engine, kind, status, priority, dedupe_key, visibility, started_at \
             FROM agent_jobs \
             WHERE priority = ?1 AND status = 'pending' \
             ORDER BY started_at ASC LIMIT 1",
            params![BG_PRIORITY],
            |r| {
                Ok(BackgroundJob {
                    id: r.get(0)?,
                    conversation_id: r.get(1)?,
                    engine: r.get(2)?,
                    kind: r.get(3)?,
                    status: r.get(4)?,
                    priority: r.get(5)?,
                    dedupe_key: r.get(6)?,
                    visibility: r.get(7)?,
                    started_at: r.get(8)?,
                })
            },
        )
        .ok();
    Ok(row)
}

/// INV-6: foreground job (priority = 0, status = 'running') 이 있는지 확인.
/// 있으면 worker 는 해당 tick 을 skip 한다.
pub fn has_foreground_running(conn: &Connection) -> Result<bool, AppError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM agent_jobs WHERE priority = 0 AND status = 'running'",
        [],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Job 상태 변경. running / done / failed / cancelled 전이.
pub fn mark_job_status(
    conn: &Connection,
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

// ─── Tauri commands ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueBackgroundJobInput {
    pub conversation_id: Option<String>,
    pub engine: String,
    pub kind: String,
    pub dedupe_key: Option<String>,
    /// `"visible"` (기본) | `"silent"`
    #[serde(default = "default_visibility")]
    pub visibility: String,
}

fn default_visibility() -> String {
    "visible".to_string()
}

#[tauri::command]
pub fn enqueue_background_job_cmd(
    input: EnqueueBackgroundJobInput,
    state: State<DbState>,
) -> Result<Option<String>, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    enqueue_background_job(
        &conn,
        input.conversation_id.as_deref(),
        &input.engine,
        &input.kind,
        input.dedupe_key.as_deref(),
        &input.visibility,
    )
}

/// pending / running 상태 job 을 'cancelled' 로 전환. running 은 best-effort —
/// worker 는 다음 tick 에서 상태를 보고 후속 emit 를 스킵.
#[tauri::command]
pub fn cancel_background_job(
    id: String,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    conn.execute(
        "UPDATE agent_jobs SET status = 'cancelled', updated_at = ?1 \
         WHERE id = ?2 AND priority = ?3 AND status IN ('pending', 'running')",
        params![now, id, BG_PRIORITY],
    )?;
    Ok(())
}

/// StatusBar 폴링용 — pending 및 running 의 bg job 카운트.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundJobCounts {
    pub pending: i64,
    pub running: i64,
}

#[tauri::command]
pub fn count_background_jobs(state: State<DbState>) -> Result<BackgroundJobCounts, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let pending: i64 = conn.query_row(
        "SELECT COUNT(*) FROM agent_jobs WHERE priority = ?1 AND status = 'pending'",
        params![BG_PRIORITY],
        |r| r.get(0),
    )?;
    let running: i64 = conn.query_row(
        "SELECT COUNT(*) FROM agent_jobs WHERE priority = ?1 AND status = 'running'",
        params![BG_PRIORITY],
        |r| r.get(0),
    )?;
    Ok(BackgroundJobCounts { pending, running })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBackgroundInsightEnabledInput {
    pub enabled: bool,
}

#[tauri::command]
pub fn get_background_insight_enabled() -> Result<bool, AppError> {
    Ok(BACKGROUND_INSIGHT_ENABLED.load(Ordering::Relaxed))
}

#[tauri::command]
pub fn set_background_insight_enabled(
    input: SetBackgroundInsightEnabledInput,
) -> Result<(), AppError> {
    BACKGROUND_INSIGHT_ENABLED.store(input.enabled, Ordering::Relaxed);
    Ok(())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE agent_jobs (
                id              TEXT PRIMARY KEY,
                conversation_id TEXT,
                message_id      TEXT,
                engine          TEXT NOT NULL,
                kind            TEXT NOT NULL DEFAULT 'agent',
                status          TEXT NOT NULL DEFAULT 'running',
                error           TEXT,
                started_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL,
                priority        INTEGER NOT NULL DEFAULT 0,
                dedupe_key      TEXT,
                visibility      TEXT NOT NULL DEFAULT 'visible'
            );
            CREATE INDEX idx_agent_jobs_bg_pending ON agent_jobs(priority, status, started_at);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn enqueue_creates_job_with_bg_priority_and_pending_status() {
        let conn = test_conn();
        let id = enqueue_background_job(&conn, Some("c1"), "claude", "identity_analysis", None, "visible")
            .unwrap()
            .expect("new id");
        let (priority, status, visibility): (i64, String, String) = conn
            .query_row(
                "SELECT priority, status, visibility FROM agent_jobs WHERE id = ?1",
                [&id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(priority, BG_PRIORITY);
        assert_eq!(status, "pending");
        assert_eq!(visibility, "visible");
    }

    #[test]
    fn enqueue_dedup_skips_duplicate_key() {
        let conn = test_conn();
        let first =
            enqueue_background_job(&conn, Some("c1"), "claude", "insight_background", Some("k1"), "visible")
                .unwrap();
        assert!(first.is_some());
        let second =
            enqueue_background_job(&conn, Some("c1"), "claude", "insight_background", Some("k1"), "visible")
                .unwrap();
        assert!(second.is_none(), "같은 dedupe_key + 미완료 상태면 skip");
    }

    #[test]
    fn enqueue_without_dedupe_key_always_creates() {
        let conn = test_conn();
        let a = enqueue_background_job(&conn, None, "claude", "identity_analysis", None, "visible").unwrap();
        let b = enqueue_background_job(&conn, None, "claude", "identity_analysis", None, "visible").unwrap();
        assert!(a.is_some() && b.is_some());
        assert_ne!(a, b);
    }

    #[test]
    fn pick_returns_oldest_pending_first() {
        let conn = test_conn();
        // 시간 순서 보장 위해 started_at 직접 제어
        conn.execute(
            "INSERT INTO agent_jobs (id, conversation_id, engine, kind, status, started_at, updated_at, priority) \
             VALUES ('j-old', NULL, 'c', 'k', 'pending', 10, 10, -1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO agent_jobs (id, conversation_id, engine, kind, status, started_at, updated_at, priority) \
             VALUES ('j-new', NULL, 'c', 'k', 'pending', 20, 20, -1)",
            [],
        )
        .unwrap();
        let picked = pick_next_background_job(&conn).unwrap().expect("job");
        assert_eq!(picked.id, "j-old");
    }

    #[test]
    fn pick_ignores_running_and_foreground_jobs() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO agent_jobs (id, conversation_id, engine, kind, status, started_at, updated_at, priority) \
             VALUES ('j-fg', NULL, 'c', 'k', 'pending', 1, 1, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO agent_jobs (id, conversation_id, engine, kind, status, started_at, updated_at, priority) \
             VALUES ('j-running', NULL, 'c', 'k', 'running', 2, 2, -1)",
            [],
        )
        .unwrap();
        let picked = pick_next_background_job(&conn).unwrap();
        assert!(picked.is_none(), "foreground 와 running bg 는 pick 대상 아님");
    }

    #[test]
    fn has_foreground_running_detects_fg_runner() {
        let conn = test_conn();
        assert!(!has_foreground_running(&conn).unwrap());
        conn.execute(
            "INSERT INTO agent_jobs (id, conversation_id, engine, kind, status, started_at, updated_at, priority) \
             VALUES ('j-fg', NULL, 'c', 'agent', 'running', 0, 0, 0)",
            [],
        )
        .unwrap();
        assert!(has_foreground_running(&conn).unwrap());
    }

    #[test]
    fn mark_job_status_transitions_fields() {
        let conn = test_conn();
        let id = enqueue_background_job(&conn, None, "c", "k", None, "visible")
            .unwrap()
            .unwrap();
        mark_job_status(&conn, &id, "done", None).unwrap();
        let status: String = conn
            .query_row(
                "SELECT status FROM agent_jobs WHERE id = ?1",
                [&id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "done");
    }

    #[test]
    fn background_insight_enabled_toggle_roundtrip() {
        // 글로벌 정적 상태라 원본 값 복원 주의
        let prev = BACKGROUND_INSIGHT_ENABLED.load(Ordering::Relaxed);
        BACKGROUND_INSIGHT_ENABLED.store(false, Ordering::Relaxed);
        assert!(!BACKGROUND_INSIGHT_ENABLED.load(Ordering::Relaxed));
        BACKGROUND_INSIGHT_ENABLED.store(true, Ordering::Relaxed);
        assert!(BACKGROUND_INSIGHT_ENABLED.load(Ordering::Relaxed));
        BACKGROUND_INSIGHT_ENABLED.store(prev, Ordering::Relaxed);
    }
}
