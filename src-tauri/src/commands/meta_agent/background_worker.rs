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

    // Dispatcher — identity_analysis 는 subtask-03 에서 실제 핸들러 연결됨.
    let result = dispatch(&job, db).await;

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

/// Kind 별 dispatcher. `identity_analysis` 는 subtask-03 에서 실 핸들러 연결됨.
/// `insight_background` 는 userWorldview 잔여 경로로 현재 no-op (후속 plan).
async fn dispatch(job: &BackgroundJob, db: &DbState) -> Result<(), AppError> {
    match job.kind.as_str() {
        "identity_analysis" => run_identity_analysis_dispatch(job, db).await,
        "insight_background" => {
            eprintln!(
                "[bg-worker] insight_background kind={} — no handler registered yet",
                job.id
            );
            Err(AppError::Agent(
                "insight_background kind handler not implemented".into(),
            ))
        }
        other => Err(AppError::BadRequest(format!(
            "unknown background kind: {}",
            other
        ))),
    }
}

/// identity_analysis 실행. sync 경로 (`identity_analyzer::run_identity_analysis_inner`)
/// 를 `spawn_blocking` 으로 감싸 Tokio 워커 스레드에서 실행.
///
/// dedupe_key 포맷 `identity-analysis-{project_key}-{done_count}` 에서 project_key
/// 를 추출. parse 실패 시 job 실패 처리.
async fn run_identity_analysis_dispatch(
    job: &BackgroundJob,
    db: &DbState,
) -> Result<(), AppError> {
    let Some(dedupe_key) = &job.dedupe_key else {
        return Err(AppError::BadRequest(
            "identity_analysis job missing dedupe_key — cannot resolve project_key".into(),
        ));
    };
    // 포맷: "identity-analysis-{project}-{count}". count 이전의 '-' 분리 기반 역추출.
    let project_key = parse_project_key_from_dedupe(dedupe_key).ok_or_else(|| {
        AppError::BadRequest(format!(
            "identity_analysis: cannot parse project_key from dedupe_key '{}'",
            dedupe_key
        ))
    })?;
    let project_key = project_key.to_string();
    let db_write = db.write.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), AppError> {
        let conn = db_write.lock().map_err(|_| AppError::Lock)?;
        let outcome = crate::agents::identity_analyzer::run_identity_analysis_inner(
            &conn,
            &project_key,
            &crate::agents::identity_analyzer::InvokeAnalyzer::Real { model: None },
        )?;
        eprintln!(
            "[identity-analyzer] job done artifact_id={} regenerated={}",
            outcome.artifact_id, outcome.regenerated
        );
        Ok(())
    })
    .await
    .map_err(|e| AppError::Agent(format!("spawn_blocking join error: {}", e)))?
}

/// `identity-analysis-{project_key}-{count}` 에서 project_key 부분 추출. prefix 고정
/// 길이 + trailing `-{digits}` 제거. project_key 에 '-' 가 포함돼도 된다.
fn parse_project_key_from_dedupe(dedupe_key: &str) -> Option<&str> {
    const PREFIX: &str = "identity-analysis-";
    let rest = dedupe_key.strip_prefix(PREFIX)?;
    // trailing "-<digits>" 제거
    let trailing_dash = rest.rfind('-')?;
    let (project, tail) = rest.split_at(trailing_dash);
    if tail.len() >= 2 && tail[1..].chars().all(|c| c.is_ascii_digit()) {
        Some(project)
    } else {
        None
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::parse_project_key_from_dedupe;

    #[test]
    fn parse_simple_project_key() {
        assert_eq!(
            parse_project_key_from_dedupe("identity-analysis-myproj-3"),
            Some("myproj")
        );
    }

    #[test]
    fn parse_project_key_with_dashes() {
        assert_eq!(
            parse_project_key_from_dedupe("identity-analysis-my-cool-proj-9"),
            Some("my-cool-proj")
        );
    }

    #[test]
    fn parse_rejects_unknown_prefix() {
        assert_eq!(parse_project_key_from_dedupe("wrong-prefix-p-3"), None);
    }

    #[test]
    fn parse_rejects_non_numeric_suffix() {
        assert_eq!(parse_project_key_from_dedupe("identity-analysis-proj-x"), None);
    }
}
