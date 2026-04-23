//! metaAgent Phase 3 — identity analysis trigger.
//!
//! Plan 완료 시점에 두 조건 AND 가 충족되면 `identity_analysis` background job 을
//! enqueue 한다. 실제 분석 실행 (prompt assembly + LLM + identity_summary 생성)
//! 은 `projectIdentityAnalysisPlan` subtask-03 에서 `background_worker` 의
//! dispatcher 에 핸들러를 붙여 완성.
//!
//! 조건:
//! 1. `done_count % 3 == 0` (Plan 3개 완료 단위)
//! 2. 이전 `identity_summary` 이후 누적 eligible artifact ≥ threshold (default 10)
//!
//! Invariants:
//! - INV-3: `BACKGROUND_INSIGHT_ENABLED` 토글이 OFF 면 trigger skip.
//! - INV-6: `dedupe_key='identity-analysis-{project}-{since_ts}'` 로 race 시에도
//!   1회만 enqueue.
//! - INV-1 (parent plan): 자동 action 은 artifact insert 만. 파괴적 변경 없음.

use std::sync::atomic::Ordering;

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::db::DbState;
use crate::errors::AppError;

use super::background_jobs::{enqueue_background_job, BACKGROUND_INSIGHT_ENABLED};

/// 기본 eligible artifact threshold. env var `TUNAFLOW_IDENTITY_ANALYSIS_THRESHOLD`
/// 또는 후속 PR 의 Settings UI 가 override. 현재는 상수 + env.
pub const DEFAULT_IDENTITY_ANALYSIS_THRESHOLD: i64 = 10;

/// artifact taxonomy — trigger 의 조건 B 에서 카운트 대상. identity_summary 는
/// output 이라 제외. subtask-01 의 ArtifactKind::is_identity_input 와 정합.
const ELIGIBLE_ARTIFACT_KINDS: &[&str] = &[
    "decision",
    "review_outcome",
    "rework_reason",
    "finding_success",
    "finding_failure",
    "workflow_milestone",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityTriggerDecision {
    pub should_run: bool,
    pub done_plan_count: i64,
    pub eligible_artifact_count: i64,
    pub threshold: i64,
    /// 디버그용 사유 코드: `count_mod3` | `volume_min` | `disabled` | `ok` | `forced`.
    pub reason: String,
}

/// 현재 threshold 값. env var 우선, 없으면 상수 default.
/// `app_settings` 테이블은 아직 없어 DB 경로 미도입 — 후속 PR 에서 Settings UI 와 함께 추가.
pub fn load_threshold() -> i64 {
    std::env::var("TUNAFLOW_IDENTITY_ANALYSIS_THRESHOLD")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(DEFAULT_IDENTITY_ANALYSIS_THRESHOLD)
}

/// Trigger 조건 평가. Pure read-only — DB write 없음.
///
/// Enabled 플래그가 OFF 여도 decision 자체는 계산 (UI 가 "왜 안 돌았는가" 를
/// 설명할 수 있도록). 실제 enqueue 는 caller 가 `should_run && enabled` 확인.
pub fn evaluate_identity_trigger(
    conn: &Connection,
    project_key: &str,
) -> Result<IdentityTriggerDecision, AppError> {
    evaluate_identity_trigger_with_threshold(conn, project_key, load_threshold())
}

/// 테스트 / force 경로용 — threshold 를 명시적으로 주입. 프로덕션 경로는
/// `evaluate_identity_trigger` 를 쓰고, 이 함수는 env var 경쟁 없는 테스트에 사용.
pub fn evaluate_identity_trigger_with_threshold(
    conn: &Connection,
    project_key: &str,
    threshold: i64,
) -> Result<IdentityTriggerDecision, AppError> {

    // 조건 A: plan done count % 3 == 0 (0 은 제외 — 0 == trigger 대상 아님)
    let done_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM plans p \
             JOIN conversations c ON p.conversation_id = c.id \
             WHERE p.status = 'done' AND c.project_key = ?1",
            [project_key],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if done_count == 0 || done_count % 3 != 0 {
        return Ok(IdentityTriggerDecision {
            should_run: false,
            done_plan_count: done_count,
            eligible_artifact_count: 0,
            threshold,
            reason: "count_mod3".into(),
        });
    }

    // 이전 identity_summary 시점 이후 누적된 eligible artifact 카운트
    let last_summary_at: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(a.created_at), 0) FROM artifacts a \
             JOIN conversations c ON a.conversation_id = c.id \
             WHERE a.type = 'identity_summary' AND c.project_key = ?1",
            [project_key],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // IN (...) 의 placeholder 개수를 ELIGIBLE_ARTIFACT_KINDS 에 맞춰 동적 생성
    let placeholders = ELIGIBLE_ARTIFACT_KINDS
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 3))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT COUNT(*) FROM artifacts a \
         JOIN conversations c ON a.conversation_id = c.id \
         WHERE c.project_key = ?1 \
           AND a.created_at > ?2 \
           AND a.type IN ({})",
        placeholders
    );
    let mut params_vec: Vec<&dyn rusqlite::ToSql> = vec![&project_key, &last_summary_at];
    for k in ELIGIBLE_ARTIFACT_KINDS {
        params_vec.push(k);
    }
    let eligible_count: i64 = conn
        .query_row(&sql, rusqlite::params_from_iter(params_vec.iter()), |r| r.get(0))
        .unwrap_or(0);

    if eligible_count < threshold {
        return Ok(IdentityTriggerDecision {
            should_run: false,
            done_plan_count: done_count,
            eligible_artifact_count: eligible_count,
            threshold,
            reason: "volume_min".into(),
        });
    }

    Ok(IdentityTriggerDecision {
        should_run: true,
        done_plan_count: done_count,
        eligible_artifact_count: eligible_count,
        threshold,
        reason: "ok".into(),
    })
}

/// Background job enqueue + kick-off 이벤트 emit. 실제 분석은 worker 의 dispatcher
/// 가 subtask-03 에서 구현할 핸들러로 넘긴다.
///
/// dedupe_key = `identity-analysis-{project}-{done_plan_count}` — 같은 period
/// (plans 3개 단위) 에 중복 enqueue 방어. 사용자가 수동 force 트리거해도 period
/// key 가 같으면 skip (이미 큐에 있음).
pub fn enqueue_identity_analysis_job(
    conn: &Connection,
    app: Option<&AppHandle>,
    project_key: &str,
    decision: &IdentityTriggerDecision,
) -> Result<Option<String>, AppError> {
    let dedupe_key = format!(
        "identity-analysis-{}-{}",
        project_key, decision.done_plan_count
    );
    let job_id = enqueue_background_job(
        conn,
        None,
        "meta",
        "identity_analysis",
        Some(&dedupe_key),
        "visible",
    )?;

    if let (Some(app), Some(ref id)) = (app, &job_id) {
        let _ = app.emit(
            "identity_analysis_triggered",
            serde_json::json!({
                "jobId": id,
                "projectKey": project_key,
                "donePlanCount": decision.done_plan_count,
                "eligibleArtifacts": decision.eligible_artifact_count,
                "threshold": decision.threshold,
            }),
        );
    }
    Ok(job_id)
}

/// `update_plan_status` 말미에서 호출하는 fire-and-forget 훅. 실패는 무시.
///
/// INV-3 준수: BACKGROUND_INSIGHT_ENABLED OFF 면 trigger 평가 자체를 skip 해
/// 불필요한 쿼리도 돌지 않도록 한다.
pub fn maybe_trigger_identity_analysis_on_plan_done(
    conn: &Connection,
    app: Option<&AppHandle>,
    plan_id: &str,
) {
    if !BACKGROUND_INSIGHT_ENABLED.load(Ordering::Relaxed) {
        return;
    }

    // plan_id → project_key (JOIN conversations). 실패 시 조용히 반환.
    let project_key: Option<String> = conn
        .query_row(
            "SELECT c.project_key FROM plans p \
             JOIN conversations c ON p.conversation_id = c.id \
             WHERE p.id = ?1",
            [plan_id],
            |r| r.get(0),
        )
        .ok();
    let Some(project_key) = project_key else { return };

    let decision = match evaluate_identity_trigger(conn, &project_key) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[identity-trigger] evaluate failed: {}", e);
            return;
        }
    };
    if !decision.should_run {
        return;
    }
    if let Err(e) = enqueue_identity_analysis_job(conn, app, &project_key, &decision) {
        eprintln!("[identity-trigger] enqueue failed: {}", e);
    }
}

// ─── Tauri commands ─────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerIdentityAnalysisNowInput {
    pub project_key: String,
    /// threshold 조건을 무시하고 강제 실행. count_mod3 조건은 그대로 체크 —
    /// plan 완료 수가 3의 배수가 아니면 여전히 skip (partial period 는 분석 품질
    /// 악화 가능).
    #[serde(default)]
    pub force: bool,
}

/// Settings UI 의 "지금 확인" / "강제 실행" 버튼용. decision 을 리턴해 UI 가
/// reasoning 을 그대로 표시할 수 있도록.
#[tauri::command]
pub fn trigger_identity_analysis_now(
    input: TriggerIdentityAnalysisNowInput,
    state: State<DbState>,
    app: AppHandle,
) -> Result<IdentityTriggerDecision, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let mut decision = evaluate_identity_trigger(&conn, &input.project_key)?;
    if input.force && decision.done_plan_count > 0 && decision.done_plan_count % 3 == 0 {
        decision.should_run = true;
        decision.reason = "forced".into();
    }
    if decision.should_run {
        enqueue_identity_analysis_job(&conn, Some(&app), &input.project_key, &decision)?;
    }
    Ok(decision)
}

/// UI 가 상태 배지 (done_count / eligible / threshold) 를 보여주기 위한 조회용.
#[tauri::command]
pub fn get_identity_trigger_status(
    project_key: String,
    state: State<DbState>,
) -> Result<IdentityTriggerDecision, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    evaluate_identity_trigger(&conn, &project_key)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE conversations (
                id          TEXT PRIMARY KEY,
                project_key TEXT NOT NULL,
                created_at  INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE plans (
                id              TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                status          TEXT NOT NULL,
                phase           TEXT NOT NULL DEFAULT 'drafting',
                created_at      INTEGER NOT NULL DEFAULT 0,
                updated_at      INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE artifacts (
                id              TEXT PRIMARY KEY,
                conversation_id TEXT,
                branch_id       TEXT,
                subtask_id      TEXT,
                plan_id         TEXT,
                type            TEXT NOT NULL,
                title           TEXT NOT NULL,
                content         TEXT NOT NULL,
                status          TEXT NOT NULL DEFAULT 'draft',
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL
            );
            CREATE TABLE agent_jobs (
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
            INSERT INTO conversations (id, project_key) VALUES ('c1', 'proj-x');",
        )
        .unwrap();
        conn
    }

    fn insert_plan(conn: &Connection, id: &str, status: &str) {
        conn.execute(
            "INSERT INTO plans (id, conversation_id, status) VALUES (?1, 'c1', ?2)",
            [id, status],
        )
        .unwrap();
    }

    fn insert_artifact(conn: &Connection, id: &str, typ: &str, created_at: i64) {
        conn.execute(
            "INSERT INTO artifacts (id, conversation_id, type, title, content, status, created_at, updated_at) \
             VALUES (?1, 'c1', ?2, 't', '{}', 'draft', ?3, ?3)",
            params![id, typ, created_at],
        )
        .unwrap();
    }

    fn eval(conn: &Connection, threshold: i64) -> IdentityTriggerDecision {
        evaluate_identity_trigger_with_threshold(conn, "proj-x", threshold).unwrap()
    }

    #[test]
    fn count_mod3_fail_when_done_count_is_one_or_two() {
        let conn = test_conn();
        insert_plan(&conn, "p1", "done");
        let d = eval(&conn, 10);
        assert_eq!(d.done_plan_count, 1);
        assert!(!d.should_run);
        assert_eq!(d.reason, "count_mod3");

        insert_plan(&conn, "p2", "done");
        let d = eval(&conn, 10);
        assert_eq!(d.done_plan_count, 2);
        assert!(!d.should_run);
        assert_eq!(d.reason, "count_mod3");
    }

    #[test]
    fn count_zero_does_not_trigger_even_though_zero_mod_three() {
        let conn = test_conn();
        let d = eval(&conn, 10);
        assert_eq!(d.done_plan_count, 0);
        assert!(!d.should_run);
        assert_eq!(d.reason, "count_mod3");
    }

    #[test]
    fn volume_min_fail_when_artifacts_below_threshold() {
        let conn = test_conn();
        insert_plan(&conn, "p1", "done");
        insert_plan(&conn, "p2", "done");
        insert_plan(&conn, "p3", "done");
        // 5 artifacts only (< 10)
        for i in 0..5 {
            insert_artifact(&conn, &format!("a{}", i), "decision", 100 + i);
        }
        let d = eval(&conn, 10);
        assert_eq!(d.done_plan_count, 3);
        assert_eq!(d.eligible_artifact_count, 5);
        assert_eq!(d.threshold, 10);
        assert!(!d.should_run);
        assert_eq!(d.reason, "volume_min");
    }

    #[test]
    fn fires_when_both_conditions_satisfied() {
        let conn = test_conn();
        for i in 0..3 {
            insert_plan(&conn, &format!("p{}", i), "done");
        }
        // 10 eligible artifacts spanning several kinds
        let kinds = [
            "decision", "review_outcome", "rework_reason",
            "finding_success", "finding_failure", "workflow_milestone",
            "decision", "review_outcome", "decision", "finding_success",
        ];
        for (i, k) in kinds.iter().enumerate() {
            insert_artifact(&conn, &format!("a{}", i), k, 100 + i as i64);
        }
        let d = eval(&conn, 10);
        assert!(d.should_run);
        assert_eq!(d.done_plan_count, 3);
        assert_eq!(d.eligible_artifact_count, 10);
        assert_eq!(d.reason, "ok");
    }

    #[test]
    fn last_identity_summary_resets_eligible_window() {
        let conn = test_conn();
        for i in 0..3 {
            insert_plan(&conn, &format!("p{}", i), "done");
        }
        // 과거 artifacts 10개
        for i in 0..10 {
            insert_artifact(&conn, &format!("old{}", i), "decision", 10 + i);
        }
        // 중간에 identity_summary
        insert_artifact(&conn, "summary-1", "identity_summary", 100);
        // 새 artifacts 5개만 — summary 이후 (< threshold)
        for i in 0..5 {
            insert_artifact(&conn, &format!("new{}", i), "decision", 200 + i);
        }
        let d = eval(&conn, 10);
        assert!(!d.should_run);
        assert_eq!(d.eligible_artifact_count, 5, "summary 이후 artifact 만 카운트");
        assert_eq!(d.reason, "volume_min");
    }

    #[test]
    fn identity_summary_excluded_from_eligible_count() {
        let conn = test_conn();
        for i in 0..3 {
            insert_plan(&conn, &format!("p{}", i), "done");
        }
        for i in 0..5 {
            insert_artifact(&conn, &format!("e{}", i), "decision", 10 + i);
        }
        // identity_summary 는 last_summary_at 만 갱신하고 eligible count 에는 포함되지 않음.
        // 생성 시각을 artifact 보다 앞으로 두어 window 리셋은 없게 구성.
        insert_artifact(&conn, "summary-early", "identity_summary", 0);
        let d = eval(&conn, 5);
        assert!(d.should_run);
        assert_eq!(d.eligible_artifact_count, 5);
    }

    #[test]
    fn enqueue_identity_analysis_dedupes_same_period() {
        let conn = test_conn();
        let decision = IdentityTriggerDecision {
            should_run: true,
            done_plan_count: 3,
            eligible_artifact_count: 10,
            threshold: 10,
            reason: "ok".into(),
        };
        let first = enqueue_identity_analysis_job(&conn, None, "proj-x", &decision).unwrap();
        assert!(first.is_some());
        let second = enqueue_identity_analysis_job(&conn, None, "proj-x", &decision).unwrap();
        assert!(second.is_none(), "같은 period 는 dedupe 되어야 함");
    }

    #[test]
    fn enqueue_different_periods_both_succeed() {
        let conn = test_conn();
        let d1 = IdentityTriggerDecision {
            should_run: true,
            done_plan_count: 3,
            eligible_artifact_count: 10,
            threshold: 10,
            reason: "ok".into(),
        };
        let d2 = IdentityTriggerDecision {
            should_run: true,
            done_plan_count: 6,
            eligible_artifact_count: 10,
            threshold: 10,
            reason: "ok".into(),
        };
        let a = enqueue_identity_analysis_job(&conn, None, "proj-x", &d1).unwrap();
        let b = enqueue_identity_analysis_job(&conn, None, "proj-x", &d2).unwrap();
        assert!(a.is_some() && b.is_some() && a != b);
    }

    #[test]
    fn maybe_trigger_skips_when_toggle_off() {
        let conn = test_conn();
        for i in 0..3 {
            insert_plan(&conn, &format!("p{}", i), "done");
        }
        for i in 0..10 {
            insert_artifact(&conn, &format!("a{}", i), "decision", 100 + i);
        }
        let prev = BACKGROUND_INSIGHT_ENABLED.load(Ordering::Relaxed);
        BACKGROUND_INSIGHT_ENABLED.store(false, Ordering::Relaxed);
        maybe_trigger_identity_analysis_on_plan_done(&conn, None, "p0");
        BACKGROUND_INSIGHT_ENABLED.store(prev, Ordering::Relaxed);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM agent_jobs WHERE kind='identity_analysis'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "토글 OFF 면 job enqueue 안 해야");
    }

    #[test]
    fn maybe_trigger_invalid_plan_id_returns_silently() {
        let conn = test_conn();
        // plan 이 없는 id 로 호출해도 panic 없이 조용히 반환
        maybe_trigger_identity_analysis_on_plan_done(&conn, None, "nonexistent");
    }
}
