//! metaAgent Phase 4 — background worker loop.
//!
//! 앱 startup 에서 spawn 되어, 매 tick 마다:
//! 1. Settings toggle (INV-3) 확인 — OFF 면 즉시 skip
//! 2. foreground job 진행 중 (INV-6) 이면 skip
//! 3. pending background job 1건 pick → running 으로 전이 → 이벤트 emit
//! 4. kind 별 dispatcher 호출 (현재 stub) — Phase 3 구현 완료 후 실제 실행
//! 5. 완료 상태로 mark + 완료 이벤트 emit + trace_log 기록
//!
//! concurrency = 1 (loop 구조로 자연 보장). tick 주기 = 30s.
//!
//! **Dispatcher 는 현재 skeleton 상태** — `identity_analysis` / `insight_background`
//! 는 no-op (warn 로그만). subtask-03 / 후속 PR 에서 실제 핸들러를 붙인다.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::db::DbState;
use crate::errors::AppError;

use super::background_jobs::{
    has_foreground_running, mark_job_status, pick_next_background_job, BackgroundJob,
    BACKGROUND_INSIGHT_ENABLED, WORKER_POLL_INTERVAL_SECS,
};

/// App bootstrap 에서 호출. tokio task 를 spawn 해 무한 loop 진입.
/// 단일 인스턴스 (concurrency=1) — 여러 번 호출하면 중복 worker 가 돈다.
pub fn spawn_background_worker(app: AppHandle, db: Arc<DbState>) {
    tauri::async_runtime::spawn(async move {
        eprintln!("[bg-worker] started (tick={}s)", WORKER_POLL_INTERVAL_SECS);
        loop {
            tokio::time::sleep(Duration::from_secs(WORKER_POLL_INTERVAL_SECS)).await;
            if let Err(e) = worker_tick(&app, &db).await {
                eprintln!("[bg-worker] tick error: {}", e);
            }
        }
    });
}

/// Worker 1 tick. DB 접근은 잠금 후 즉시 해제 (await 지점에서 Mutex 유지 금지).
async fn worker_tick(app: &AppHandle, db: &DbState) -> Result<(), AppError> {
    // INV-3: Settings OFF 면 skip
    if !BACKGROUND_INSIGHT_ENABLED.load(Ordering::Relaxed) {
        return Ok(());
    }

    // INV-6: foreground busy 면 양보
    {
        let conn = db.write.lock().map_err(|_| AppError::Lock)?;
        if has_foreground_running(&conn)? {
            return Ok(());
        }
    }

    // Pick + mark running — 한 transaction 안에서 묶어 race 회피
    let job: Option<BackgroundJob> = {
        let conn = db.write.lock().map_err(|_| AppError::Lock)?;
        match pick_next_background_job(&conn)? {
            Some(j) => {
                mark_job_status(&conn, &j.id, "running", None)?;
                Some(j)
            }
            None => None,
        }
    };
    let Some(job) = job else { return Ok(()) };

    if job.visibility != "silent" {
        let _ = app.emit(
            "background_insight_progress",
            json!({"jobId": job.id, "state": "started", "kind": job.kind}),
        );
    }

    // Dispatcher — 현재 skeleton. 실제 핸들러는 subtask-03 / metaAgent Phase 3 이후 추가.
    let result = dispatch(&job).await;

    let status = match &result {
        Ok(_) => "done",
        Err(_) => "failed",
    };
    {
        let conn = db.write.lock().map_err(|_| AppError::Lock)?;
        mark_job_status(
            &conn,
            &job.id,
            status,
            result.as_ref().err().map(|e| e.to_string()).as_deref(),
        )?;
    }

    if job.visibility != "silent" {
        let _ = app.emit(
            "background_insight_progress",
            json!({"jobId": job.id, "state": status, "kind": job.kind}),
        );
    }

    Ok(())
}

/// Kind 별 dispatcher. 현재 모든 kind 는 "not implemented" 로 실패 처리되도록 둬,
/// skeleton 임을 운영 시 명확히 드러낸다. 실 핸들러가 붙으면 match arm 으로 교체.
async fn dispatch(job: &BackgroundJob) -> Result<(), AppError> {
    match job.kind.as_str() {
        "identity_analysis" | "insight_background" => {
            // metaAgent Phase 3 / subtask-03 에서 실제 핸들러 추가 예정.
            eprintln!(
                "[bg-worker] dispatch stub kind={} job={} — handler not yet implemented",
                job.kind, job.id
            );
            Err(AppError::Agent(format!(
                "background kind '{}' handler not implemented (Phase 4 skeleton)",
                job.kind
            )))
        }
        other => Err(AppError::BadRequest(format!(
            "unknown background kind: {}",
            other
        ))),
    }
}
