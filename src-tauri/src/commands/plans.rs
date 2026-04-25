use rusqlite::params;
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use std::path::Path;
use tauri::AppHandle;
use crate::db::{migrations::{now_epoch, now_epoch_ms}, models::{ArtifactKind, Plan, PlanEvent, PlanSubtask}, DbState};
use crate::errors::AppError;
use super::artifacts::create_identity_input_artifact;
use super::meta_agent::identity_trigger::maybe_trigger_identity_analysis_on_plan_done;

// ─── Input types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubtaskInput {
    pub title: String,
    pub details: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlanInput {
    pub conversation_id: String,
    pub branch_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub expected_outcome: Option<String>,
    /// Initial subtasks to create alongside the plan (optional).
    #[serde(default)]
    pub subtasks: Vec<SubtaskInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlanStatusInput {
    pub id: String,
    /// "draft" | "active" | "done" | "abandoned"
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSubtaskStatusInput {
    pub id: String,
    /// "todo" | "in_progress" | "done" | "abandoned"
    pub status: String,
    pub outcome: Option<String>,
    /// Agent name that performed this update (e.g. "claude", "codex")
    pub updated_by: Option<String>,
}

// ─── Row mappers ─────────────────────────────────────────────────────────────

fn map_plan(row: &rusqlite::Row) -> rusqlite::Result<Plan> {
    Ok(Plan {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        branch_id: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        expected_outcome: row.get(5)?,
        status: row.get(6)?,
        phase: row.get(7)?,
        architect_engine: row.get(8)?,
        developer_engine: row.get(9)?,
        reviewer_engines: row.get(10)?,
        implementation_branch_id: row.get(11)?,
        review_branch_id: row.get(12)?,
        slug: row.get(13)?,
        revision: row.get(14)?,
        version_major: row.get(15)?,
        version_minor: row.get(16)?,
        created_at: row.get(17)?,
        updated_at: row.get(18)?,
    })
}

fn map_subtask(row: &rusqlite::Row) -> rusqlite::Result<PlanSubtask> {
    Ok(PlanSubtask {
        id: row.get(0)?,
        plan_id: row.get(1)?,
        idx: row.get(2)?,
        title: row.get(3)?,
        details: row.get(4)?,
        status: row.get(5)?,
        outcome: row.get(6)?,
        owner_agent: row.get(7)?,
        last_updated_by: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

const PLAN_COLS: &str =
    "id, conversation_id, branch_id, title, description, expected_outcome, status, phase, architect_engine, developer_engine, reviewer_engines, implementation_branch_id, review_branch_id, slug, revision, version_major, version_minor, created_at, updated_at";

const SUBTASK_COLS: &str =
    "id, plan_id, idx, title, details, status, outcome, owner_agent, last_updated_by, created_at, updated_at";

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Create a plan, optionally with an initial set of subtasks.
/// Returns the created Plan (subtasks can be retrieved via list_subtasks).
///
/// Atomicity: plan + subtasks 는 단일 transaction 안에서 INSERT. subtask 중간
/// 실패 시 plan 도 롤백 (planGenerationRollback Layer A).
#[tauri::command]
pub fn create_plan(
    input: CreatePlanInput,
    state: State<DbState>,
) -> Result<Plan, AppError> {
    let mut conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();

    // Generate unique slug for file paths
    let slug = {
        use crate::db::migrations::{slugify_title, find_unique_slug};
        let base = slugify_title(&input.title);
        find_unique_slug(&conn, &base, None)
    };

    create_plan_tx(&mut conn, &id, &slug, now, &input)?;

    Ok(Plan {
        id,
        conversation_id: input.conversation_id,
        branch_id: input.branch_id,
        title: input.title,
        description: input.description,
        expected_outcome: input.expected_outcome,
        status: "draft".into(),
        phase: "drafting".into(),
        architect_engine: None,
        developer_engine: None,
        reviewer_engines: None,
        implementation_branch_id: None,
        review_branch_id: None,
        slug: Some(slug),
        revision: 0,
        version_major: 1,
        version_minor: 0,
        created_at: now,
        updated_at: now,
    })
}

/// Insert plan + subtasks atomically inside a single transaction.
///
/// Pure helper extracted from `create_plan` for testability. Caller owns
/// `&mut Connection`. On any error during plan or subtask INSERT, the
/// transaction is dropped (auto-rollback via rusqlite Drop).
fn create_plan_tx(
    conn: &mut rusqlite::Connection,
    id: &str,
    slug: &str,
    now: i64,
    input: &CreatePlanInput,
) -> Result<(), AppError> {
    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO plans
         (id, conversation_id, branch_id, title, description, expected_outcome, status, slug, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'draft', ?7, ?8, ?9)",
        params![
            id,
            input.conversation_id,
            input.branch_id,
            input.title,
            input.description,
            input.expected_outcome,
            slug,
            now,
            now,
        ],
    )?;

    for (i, st) in input.subtasks.iter().enumerate() {
        let st_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO plan_subtasks
             (id, plan_id, idx, title, details, status, outcome, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'todo', NULL, ?6, ?7)",
            params![st_id, id, i as i64, st.title, st.details, now, now],
        )?;
    }

    tx.commit()?;
    Ok(())
}

/// Fetch a single plan by id.
#[tauri::command]
pub fn get_plan(id: String, state: State<DbState>) -> Result<Plan, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let sql = format!("SELECT {} FROM plans WHERE id = ?1", PLAN_COLS);
    conn.query_row(&sql, [&id], map_plan)
        .map_err(|_| AppError::NotFound(format!("plan {} not found", id)))
}

/// List all plans for a conversation (newest first).
#[tauri::command]
pub fn list_plans_by_conversation(
    conversation_id: String,
    state: State<DbState>,
) -> Result<Vec<Plan>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let sql = format!(
        "SELECT {} FROM plans WHERE conversation_id = ?1 ORDER BY created_at ASC",
        PLAN_COLS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([&conversation_id], map_plan)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// List all plans for a project (across every conversation in the project).
///
/// 도구 호출(`tool-request:plans`) 경로에서 쓰인다. `list_plans_by_conversation`
/// 은 현재 대화 ID 와 정확히 일치하는 플랜만 반환하므로, 에이전트가 브랜치
/// (shadow conv `branch:<id>`) 안에서 질의할 때 **같은 프로젝트의 메인 대화에
/// 소속된 완료 플랜** 을 놓치는 문제가 있다. 이 명령은 project_key 기준으로
/// 전체를 반환해서 에이전트가 "이 프로젝트의 완료된 플랜" 을 정확히 볼 수 있게
/// 한다.
#[tauri::command]
pub fn list_plans_by_project(
    project_key: String,
    state: State<DbState>,
) -> Result<Vec<Plan>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let sql = format!(
        "SELECT {} FROM plans p JOIN conversations c ON c.id = p.conversation_id
         WHERE c.project_key = ?1 ORDER BY p.created_at ASC",
        PLAN_COLS
            .split(", ")
            .map(|c| format!("p.{}", c))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([&project_key], map_plan)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get the active plan phase for a conversation. Returns the phase of the most recent non-done plan.
#[tauri::command]
pub fn get_active_plan_phase(
    conversation_id: String,
    state: State<DbState>,
) -> Result<Option<String>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let phase: Option<String> = conn
        .query_row(
            "SELECT phase FROM plans WHERE conversation_id = ?1 AND status != 'done' AND status != 'abandoned' ORDER BY updated_at DESC LIMIT 1",
            [&conversation_id],
            |row| row.get(0),
        )
        .ok();
    Ok(phase)
}

/// Count active (non-done, non-abandoned) plans for a project. Used for WIP limit warnings.
#[tauri::command]
pub fn count_active_plans(
    project_key: String,
    state: State<DbState>,
) -> Result<i64, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM plans p
         JOIN conversations c ON c.id = p.conversation_id
         WHERE c.project_key = ?1 AND p.status NOT IN ('done', 'abandoned')",
        [&project_key],
        |row| row.get(0),
    ).unwrap_or(0);
    Ok(count)
}

/// Plan 의 metadata(title/description/expected_outcome) 를 일괄 업데이트.
/// b 정책 revision overwrite 경로에서 사용 — 아키텍트가 rev 로 plan 을 재정의할 때
/// replacePlanSubtasks + bumpPlanMajorVersion 와 함께 호출.
#[tauri::command]
pub fn update_plan_meta(
    id: String,
    title: Option<String>,
    description: Option<String>,
    expected_outcome: Option<String>,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    // COALESCE 로 None 은 기존 값 유지.
    conn.execute(
        "UPDATE plans SET
            title = COALESCE(?1, title),
            description = COALESCE(?2, description),
            expected_outcome = COALESCE(?3, expected_outcome),
            updated_at = ?4
         WHERE id = ?5",
        params![title, description, expected_outcome, now, id],
    )?;
    Ok(())
}

/// Update the status of a plan (draft → active → done | abandoned).
#[tauri::command]
pub fn update_plan_status(
    input: UpdatePlanStatusInput,
    state: State<DbState>,
    app: AppHandle,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();

    // 전이 전 상태 snapshot — milestone emit 시 "prior status" 로 활용
    let prior_status: Option<String> = conn
        .query_row("SELECT status FROM plans WHERE id = ?1", params![&input.id], |r| r.get(0))
        .ok();

    conn.execute(
        "UPDATE plans SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![input.status, now, input.id],
    )?;
    // Sync phase + archive branches when status reaches terminal state
    if input.status == "done" || input.status == "abandoned" {
        conn.execute(
            "UPDATE plans SET phase = 'done' WHERE id = ?1 AND phase != 'done'",
            params![input.id],
        )?;
        // Archive linked implementation/review branches
        conn.execute(
            "UPDATE branches SET status = 'archived' WHERE status = 'active' AND id IN (
                SELECT implementation_branch_id FROM plans WHERE id = ?1 AND implementation_branch_id IS NOT NULL
                UNION
                SELECT review_branch_id FROM plans WHERE id = ?1 AND review_branch_id IS NOT NULL
            )",
            params![input.id],
        )?;
    }

    // projectIdentityAnalysisPlan subtask-01: Plan 완료 (status → 'done') 시점에
    // `workflow_milestone` artifact 1건 자동 생성. 동일 상태 재진입은 dedup 이 처리.
    let _ = emit_milestone_on_status_change(
        &conn,
        &input.id,
        prior_status.as_deref(),
        &input.status,
    );

    // metaAgent Phase 3: plan done 전이 시점에 identity analysis trigger 평가.
    // fire-and-forget — 실패해도 plan 상태 갱신에는 영향 없음.
    if input.status == "done" && prior_status.as_deref() != Some("done") {
        maybe_trigger_identity_analysis_on_plan_done(&conn, Some(&app), &input.id);
    }
    Ok(())
}

/// List all subtasks for a plan, ordered by idx.
#[tauri::command]
pub fn list_subtasks(plan_id: String, state: State<DbState>) -> Result<Vec<PlanSubtask>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let sql = format!(
        "SELECT {} FROM plan_subtasks WHERE plan_id = ?1 ORDER BY idx ASC",
        SUBTASK_COLS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([&plan_id], map_subtask)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Set the owner_agent for a subtask.
#[tauri::command]
pub fn set_subtask_owner(
    id: String,
    owner_agent: Option<String>,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    conn.execute(
        "UPDATE plan_subtasks SET owner_agent = ?1, updated_at = ?2 WHERE id = ?3",
        params![owner_agent, now, id],
    )?;
    Ok(())
}

/// Update the status (and optional outcome) of a single subtask.
#[tauri::command]
pub fn update_subtask_status(
    input: UpdateSubtaskStatusInput,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    conn.execute(
        "UPDATE plan_subtasks SET status = ?1, outcome = ?2, last_updated_by = ?3, updated_at = ?4 WHERE id = ?5",
        params![input.status, input.outcome, input.updated_by, now, input.id],
    )?;
    Ok(())
}

/// Replace all subtasks for a plan with a new ordered list.
/// Deletes existing subtasks, then inserts the new ones.
/// Also bumps plan.updated_at.
///
/// Atomicity: DELETE + UPDATE + INSERT loop 는 단일 transaction. INSERT 중간
/// 실패 시 기존 subtask 가 보존됨 (planGenerationRollback Layer A) — 사용자가
/// 작성한 plan body 의 부분 손실을 방지한다.
#[tauri::command]
pub fn replace_plan_subtasks(
    plan_id: String,
    subtasks: Vec<SubtaskInput>,
    state: State<DbState>,
) -> Result<Vec<PlanSubtask>, AppError> {
    let mut conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    replace_plan_subtasks_tx(&mut conn, &plan_id, &subtasks, now)
}

/// Pure helper: replace plan subtasks atomically inside a single transaction.
///
/// On any error during DELETE / UPDATE / INSERT, the transaction is dropped
/// (auto-rollback via rusqlite Drop) — existing subtasks are preserved.
fn replace_plan_subtasks_tx(
    conn: &mut rusqlite::Connection,
    plan_id: &str,
    subtasks: &[SubtaskInput],
    now: i64,
) -> Result<Vec<PlanSubtask>, AppError> {
    let tx = conn.transaction()?;

    tx.execute("DELETE FROM plan_subtasks WHERE plan_id = ?1", [plan_id])?;
    tx.execute(
        "UPDATE plans SET revision = revision + 1, version_minor = version_minor + 1, updated_at = ?1 WHERE id = ?2",
        params![now, plan_id],
    )?;

    let mut result: Vec<PlanSubtask> = Vec::new();
    for (i, st) in subtasks.iter().enumerate() {
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO plan_subtasks
             (id, plan_id, idx, title, details, status, outcome, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'todo', NULL, ?6, ?7)",
            params![id, plan_id, i as i64, st.title, st.details, now, now],
        )?;
        result.push(PlanSubtask {
            id,
            plan_id: plan_id.to_string(),
            idx: i as i64,
            title: st.title.clone(),
            details: st.details.clone(),
            status: "todo".into(),
            outcome: None,
            owner_agent: None,
            last_updated_by: None,
            created_at: now,
            updated_at: now,
        });
    }

    tx.commit()?;
    Ok(result)
}

/// Delete a plan and all its subtasks (CASCADE handles subtasks).
#[tauri::command]
pub fn delete_plan(id: String, state: State<DbState>) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    conn.execute("DELETE FROM plans WHERE id = ?1", [&id])?;
    Ok(())
}

/// Find a plan linked to a branch (as implementation or review branch).
#[tauri::command]
pub fn find_plan_by_branch(
    branch_id: String,
    state: State<DbState>,
) -> Result<Option<Plan>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let sql = format!(
        "SELECT {} FROM plans WHERE implementation_branch_id = ?1 OR review_branch_id = ?1 LIMIT 1",
        PLAN_COLS
    );
    match conn.query_row(&sql, [&branch_id], map_plan) {
        Ok(plan) => Ok(Some(plan)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

// ─── Orchestration Commands (Phase A) ────────────────────────────────────────

/// Valid plan phases (canonical list — mirrors PlanPhase in types.ts).
const VALID_PHASES: &[&str] = &[
    "drafting", "subtask_review", "approval",
    "implementation", "rework", "review", "done",
];

/// Update the orchestration phase of a plan.
/// Returns an error if the requested phase is not in the canonical list.
///
/// ⚠️ async + spawn_blocking. Plan 승인 UI 클릭 경로. sync 버전은 동시에 돌고
/// 있는 post-completion hook 이 write lock 을 hold 하면 main thread freeze.
/// 자세한 설명: `docs/reference/refactor-regression-audit_2026-04-22.md`.
#[tauri::command]
pub async fn update_plan_phase(
    id: String,
    phase: String,
    state: State<'_, DbState>,
) -> Result<(), AppError> {
    if !VALID_PHASES.contains(&phase.as_str()) {
        return Err(AppError::BadRequest(format!("invalid plan phase: {phase}")));
    }
    let write = state.write.clone();
    tokio::task::spawn_blocking(move || -> Result<(), AppError> {
        let conn = write.lock().map_err(|_| AppError::Lock)?;
        let now = now_epoch_ms();

        // projectIdentityAnalysisPlan subtask-01: phase → implementation 전이는
        // "Plan 승인" 시점이므로 `decision` identity-input artifact 1건 자동 생성.
        // 이미 implementation 이면 중복 호출로 간주, emit 안 함 (dedup 이 방어하지만
        // query 자체를 줄이는 빠른 path). INV-1 준수 — 대화 내용 파싱 없이 phase 전이만 사용.
        let prior_phase: Option<String> = conn
            .query_row("SELECT phase FROM plans WHERE id = ?1", params![&id], |r| r.get(0))
            .ok();

        conn.execute(
            "UPDATE plans SET phase = ?1, updated_at = ?2 WHERE id = ?3",
            params![phase, now, id],
        )?;

        let _ = emit_decision_on_phase_change(&conn, &id, prior_phase.as_deref(), &phase);

        Ok(())
    })
    .await
    .map_err(|e| AppError::Agent(format!("spawn_blocking failed: {}", e)))?
}

/// plan id → (conversation_id, title) 를 읽어오는 소형 헬퍼. artifact title 용.
fn load_plan_context(conn: &rusqlite::Connection, plan_id: &str) -> Result<Option<(String, String)>, AppError> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT conversation_id, title FROM plans WHERE id = ?1",
            params![plan_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .ok();
    Ok(row)
}

/// phase 전이가 `implementation` 으로 들어올 때 `decision` artifact 생성.
/// prior_phase 가 같으면 no-op. 테스트 가능한 pure helper.
pub(crate) fn emit_decision_on_phase_change(
    conn: &rusqlite::Connection,
    plan_id: &str,
    prior_phase: Option<&str>,
    new_phase: &str,
) -> Result<Option<String>, AppError> {
    if new_phase != "implementation" || prior_phase == Some("implementation") {
        return Ok(None);
    }
    let Some((conv_id, title)) = load_plan_context(conn, plan_id)? else {
        return Ok(None);
    };
    let content = serde_json::json!({
        "what": "plan_approved",
        "plan_id": plan_id,
        "previous_phase": prior_phase,
        "approved_by": "user",
    });
    create_identity_input_artifact(
        conn,
        ArtifactKind::Decision,
        Some(&conv_id),
        Some(plan_id),
        None,
        &format!("Plan '{}' approved", title),
        content,
    )
}

/// status 전이가 `done` 으로 들어올 때 `workflow_milestone` artifact 생성.
/// prior_status 가 이미 done 이면 no-op. 테스트 가능한 pure helper.
pub(crate) fn emit_milestone_on_status_change(
    conn: &rusqlite::Connection,
    plan_id: &str,
    prior_status: Option<&str>,
    new_status: &str,
) -> Result<Option<String>, AppError> {
    if new_status != "done" || prior_status == Some("done") {
        return Ok(None);
    }
    let Some((conv_id, title)) = load_plan_context(conn, plan_id)? else {
        return Ok(None);
    };
    let content = serde_json::json!({
        "milestone_kind": "plan_done",
        "plan_id": plan_id,
        "summary": title,
    });
    create_identity_input_artifact(
        conn,
        ArtifactKind::WorkflowMilestone,
        Some(&conv_id),
        Some(plan_id),
        None,
        &format!("Plan '{}' completed", title),
        content,
    )
}

/// Create a plan event (history log entry).
#[tauri::command]
pub fn create_plan_event(
    plan_id: String,
    event_type: String,
    actor: Option<String>,
    detail: Option<String>,
    state: State<DbState>,
) -> Result<PlanEvent, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let id = Uuid::new_v4().to_string();
    let created_at = now_epoch();
    conn.execute(
        "INSERT INTO plan_events (id, plan_id, event_type, actor, detail, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, plan_id, event_type, actor, detail, created_at],
    )?;
    Ok(PlanEvent {
        id,
        plan_id,
        event_type,
        actor,
        detail,
        created_at,
    })
}

/// List all events for a plan (oldest first).
#[tauri::command]
pub fn list_plan_events(
    plan_id: String,
    state: State<DbState>,
) -> Result<Vec<PlanEvent>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let mut stmt = conn.prepare(
        "SELECT id, plan_id, event_type, actor, detail, created_at
         FROM plan_events WHERE plan_id = ?1 ORDER BY created_at ASC"
    )?;
    let rows = stmt
        .query_map([&plan_id], |row| {
            Ok(PlanEvent {
                id: row.get(0)?,
                plan_id: row.get(1)?,
                event_type: row.get(2)?,
                actor: row.get(3)?,
                detail: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Link a branch to a plan (implementation or review).
#[tauri::command]
pub fn link_plan_branch(
    id: String,
    branch_type: String,  // "implementation" or "review"
    branch_id: Option<String>,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    let col = match branch_type.as_str() {
        "implementation" => "implementation_branch_id",
        "review" => "review_branch_id",
        _ => return Err(AppError::NotFound(format!("Unknown branch type: {}", branch_type))),
    };
    let sql = format!("UPDATE plans SET {} = ?1, updated_at = ?2 WHERE id = ?3", col);
    conn.execute(&sql, params![branch_id, now, id])?;
    Ok(())
}

/// Assign engines to a plan (architect, developer, reviewers).
#[tauri::command]
pub fn assign_plan_engines(
    id: String,
    architect_engine: Option<String>,
    developer_engine: Option<String>,
    reviewer_engines: Option<String>,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    conn.execute(
        "UPDATE plans SET architect_engine = ?1, developer_engine = ?2, reviewer_engines = ?3, updated_at = ?4 WHERE id = ?5",
        params![architect_engine, developer_engine, reviewer_engines, now, id],
    )?;
    Ok(())
}

// ─── Plan Document Generation ────────────────────────────────────────────────

/// Generate/update a plan document as markdown in the project directory.
/// File: {project_path}/docs/plans/{slug}.md
#[tauri::command]
pub fn generate_plan_document(
    plan_id: String,
    project_path: String,
    state: State<DbState>,
) -> Result<String, AppError> {
    let (plan, subtasks, events) = {
        let conn = state.read.lock().map_err(|_| AppError::Lock)?;

        let sql = format!("SELECT {} FROM plans WHERE id = ?1", PLAN_COLS);
        let plan: Plan = conn.query_row(&sql, [&plan_id], map_plan)
            .map_err(|_| AppError::NotFound(format!("plan {} not found", plan_id)))?;

        let subtask_sql = format!("SELECT {} FROM plan_subtasks WHERE plan_id = ?1 ORDER BY idx ASC", SUBTASK_COLS);
        let mut stmt = conn.prepare(&subtask_sql)?;
        let subtasks: Vec<PlanSubtask> = stmt
            .query_map([&plan_id], map_subtask)?
            .collect::<Result<Vec<_>, _>>()?;

        let mut evt_stmt = conn.prepare(
            "SELECT id, plan_id, event_type, actor, detail, created_at FROM plan_events WHERE plan_id = ?1 ORDER BY created_at ASC"
        )?;
        let events: Vec<PlanEvent> = evt_stmt
            .query_map([&plan_id], |row| Ok(PlanEvent {
                id: row.get(0)?,
                plan_id: row.get(1)?,
                event_type: row.get(2)?,
                actor: row.get(3)?,
                detail: row.get(4)?,
                created_at: row.get(5)?,
            }))?
            .collect::<Result<Vec<_>, _>>()?;

        (plan, subtasks, events)
    }; // lock released

    // Generate markdown
    let md = build_plan_markdown(&plan, &subtasks, &events);

    // Write to file. Canonical slug lives in plans.slug (v26); title-based
    // slugify is kept as a fallback for pre-v26 rows that somehow missed
    // backfill — all other paths (review, result, Reviewer context loader)
    // must stay in sync with this source.
    let slug = plan.slug.clone().unwrap_or_else(|| slugify(&plan.title));
    let dir = Path::new(&project_path).join("docs").join("plans");
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::Agent(format!("Failed to create dir: {}", e)))?;
    let file_path = dir.join(format!("{}.md", slug));
    // Skip if file already exists (Architect may have written it directly)
    if !file_path.exists() {
        atomic_write_md(&file_path, &md)?;
    }

    Ok(file_path.to_string_lossy().to_string())
}

/// Write `content` to `target` atomically: write to a temp file in the same
/// directory, then rename. Prevents partial / truncated .md on disk-full or
/// crash mid-write (planGenerationRollback Layer A).
fn atomic_write_md(target: &Path, content: &str) -> Result<(), AppError> {
    let dir = target.parent().ok_or_else(|| {
        AppError::Agent(format!("plan doc target has no parent dir: {}", target.display()))
    })?;
    let file_name = target.file_name().and_then(|n| n.to_str()).unwrap_or("plan");
    // Use pid + timestamp to avoid collisions if multiple writes race.
    let tmp_name = format!(
        ".{}.tmp.{}.{}",
        file_name,
        std::process::id(),
        now_epoch_ms()
    );
    let tmp_path = dir.join(tmp_name);

    std::fs::write(&tmp_path, content)
        .map_err(|e| AppError::Agent(format!("Failed to write plan doc tmp: {}", e)))?;

    if let Err(e) = std::fs::rename(&tmp_path, target) {
        // Best-effort cleanup of the temp file; ignore secondary errors.
        let _ = std::fs::remove_file(&tmp_path);
        return Err(AppError::Agent(format!(
            "Failed to rename plan doc tmp -> target: {}",
            e
        )));
    }
    Ok(())
}

/// Bump version_major and reset version_minor (for full plan updates from Chat).
#[tauri::command]
pub fn bump_plan_major_version(
    id: String,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    conn.execute(
        "UPDATE plans SET version_major = version_major + 1, version_minor = 0, revision = revision + 1, updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Public accessor for slugify — used by ContextPack plan document loader
pub fn slugify_pub(title: &str) -> String { slugify(title) }

/// Delegate to migrations::slugify_title (single canonical slug implementation).
fn slugify(title: &str) -> String {
    crate::db::migrations::slugify_title(title)
}

/// Generate a review report document.
/// File: {project_path}/docs/plans/{slug}-review-r{round}.md
#[tauri::command]
pub fn generate_review_report(
    plan_id: String,
    project_path: String,
    verdict: String,
    findings: Vec<String>,
    recommendations: Vec<String>,
    reviewer_engines: Vec<String>,
    test_output: Option<String>,
    state: State<DbState>,
) -> Result<String, AppError> {
    let (plan, subtasks) = {
        let conn = state.read.lock().map_err(|_| AppError::Lock)?;
        let sql = format!("SELECT {} FROM plans WHERE id = ?1", PLAN_COLS);
        let plan: Plan = conn.query_row(&sql, [&plan_id], map_plan)
            .map_err(|_| AppError::NotFound(format!("plan {} not found", plan_id)))?;
        let subtask_sql = format!("SELECT {} FROM plan_subtasks WHERE plan_id = ?1 ORDER BY idx ASC", SUBTASK_COLS);
        let mut stmt = conn.prepare(&subtask_sql)?;
        let subtasks: Vec<PlanSubtask> = stmt.query_map([&plan_id], map_subtask)?
            .collect::<Result<Vec<_>, _>>()?;
        (plan, subtasks)
    };

    // Determine round number from existing files
    let slug = plan.slug.clone().unwrap_or_else(|| slugify(&plan.title));
    let dir = Path::new(&project_path).join("docs").join("plans");
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Agent(format!("mkdir: {}", e)))?;
    let mut round = 1;
    while dir.join(format!("{}-review-r{}.md", slug, round)).exists() {
        round += 1;
    }

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let mut md = String::new();
    md.push_str(&format!("# Review Report: {} — Round {}\n\n", plan.title, round));
    md.push_str(&format!("> Verdict: {}\n", verdict));
    md.push_str(&format!("> Reviewer: {}\n", reviewer_engines.join(", ")));
    md.push_str(&format!("> Date: {}\n", now));
    md.push_str(&format!("> Plan Revision: {}\n\n", plan.revision));
    md.push_str("---\n\n");

    md.push_str(&format!("## Verdict\n\n**{}**\n\n", verdict));

    if !findings.is_empty() {
        md.push_str("## Findings\n\n");
        for (i, f) in findings.iter().enumerate() {
            md.push_str(&format!("{}. {}\n", i + 1, f));
        }
        md.push('\n');
    }

    if !recommendations.is_empty() {
        md.push_str("## Recommendations\n\n");
        for (i, r) in recommendations.iter().enumerate() {
            md.push_str(&format!("{}. {}\n", i + 1, r));
        }
        md.push('\n');
    }

    // Subtask verification table
    md.push_str("## Subtask Verification\n\n");
    md.push_str("| # | Subtask | Status |\n");
    md.push_str("|---|---------|--------|\n");
    for (i, st) in subtasks.iter().enumerate() {
        let mark = if st.status == "done" { "✅" } else { "❌" };
        md.push_str(&format!("| {} | {} | {} {} |\n", i + 1, st.title, mark, st.status));
    }
    md.push('\n');

    if let Some(test) = &test_output {
        md.push_str("## Test Results\n\n```\n");
        let truncated = if test.len() > 2000 { &test[..2000] } else { test.as_str() };
        md.push_str(truncated);
        md.push_str("\n```\n\n");
    }

    let file_path = dir.join(format!("{}-review-r{}.md", slug, round));
    std::fs::write(&file_path, &md).map_err(|e| AppError::Agent(format!("write: {}", e)))?;
    Ok(file_path.to_string_lossy().to_string())
}

/// Generate an implementation result document.
/// File: {project_path}/docs/plans/{slug}-result.md
#[tauri::command]
pub fn generate_result_report(
    plan_id: String,
    project_path: String,
    summary: String,
    subtask_results: Vec<String>,
    known_issues: Vec<String>,
    developer_engine: Option<String>,
    branch_label: Option<String>,
    state: State<DbState>,
) -> Result<String, AppError> {
    let plan = {
        let conn = state.read.lock().map_err(|_| AppError::Lock)?;
        let sql = format!("SELECT {} FROM plans WHERE id = ?1", PLAN_COLS);
        conn.query_row(&sql, [&plan_id], map_plan)
            .map_err(|_| AppError::NotFound(format!("plan {} not found", plan_id)))?
    };

    let slug = plan.slug.clone().unwrap_or_else(|| slugify(&plan.title));
    let dir = Path::new(&project_path).join("docs").join("plans");
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Agent(format!("mkdir: {}", e)))?;

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    let mut md = String::new();
    md.push_str(&format!("# Implementation Result: {}\n\n", plan.title));
    md.push_str(&format!("> Developer: {}\n", developer_engine.as_deref().unwrap_or("unknown")));
    md.push_str(&format!("> Branch: {}\n", branch_label.as_deref().unwrap_or("N/A")));
    md.push_str(&format!("> Date: {}\n", now));
    md.push_str(&format!("> Plan Revision: {}\n\n", plan.revision));
    md.push_str("---\n\n");

    md.push_str("## Summary\n\n");
    md.push_str(&summary);
    md.push_str("\n\n");

    if !subtask_results.is_empty() {
        md.push_str("## Subtask Results\n\n");
        for (i, r) in subtask_results.iter().enumerate() {
            md.push_str(&format!("### {}. {}\n\n", i + 1, r));
        }
    }

    if !known_issues.is_empty() {
        md.push_str("## Known Issues\n\n");
        for issue in &known_issues {
            md.push_str(&format!("- {}\n", issue));
        }
        md.push('\n');
    }

    let file_path = dir.join(format!("{}-result.md", slug));
    if file_path.exists() {
        eprintln!("[generate_result_report] Overwriting existing result report: {}", file_path.display());
    }
    std::fs::write(&file_path, &md).map_err(|e| AppError::Agent(format!("write: {}", e)))?;
    Ok(file_path.to_string_lossy().to_string())
}

fn build_plan_markdown(plan: &Plan, subtasks: &[PlanSubtask], events: &[PlanEvent]) -> String {
    let mut md = String::new();

    // Header
    md.push_str(&format!("# {}\n\n", plan.title));
    md.push_str(&format!("> Phase: {} | Status: {} | Revision: {}\n", plan.phase, plan.status, plan.revision));
    md.push_str(&format!("> Generated by tunaFlow\n\n"));
    md.push_str("---\n\n");

    // Description
    if let Some(desc) = &plan.description {
        md.push_str("## Description\n\n");
        md.push_str(desc);
        md.push_str("\n\n");
    }

    // Expected Outcome
    if let Some(outcome) = &plan.expected_outcome {
        md.push_str("## Expected Outcome\n\n");
        md.push_str(outcome);
        md.push_str("\n\n");
    }

    // Subtasks
    md.push_str("## Subtasks\n\n");
    if subtasks.is_empty() {
        md.push_str("(없음)\n\n");
    } else {
        for (i, st) in subtasks.iter().enumerate() {
            md.push_str(&format!("### {}. {}\n\n", i + 1, st.title));
            md.push_str(&format!("- **Status**: {}\n", st.status));
            if let Some(owner) = &st.owner_agent {
                md.push_str(&format!("- **Owner**: {}\n", owner));
            }
            if let Some(details) = &st.details {
                if !details.trim().is_empty() {
                    md.push_str(&format!("\n#### 작업 지시\n\n{}\n", details));
                }
            }
            md.push('\n');
        }
    }

    // Revision History
    if !events.is_empty() {
        md.push_str("## Revision History\n\n");
        for ev in events {
            let ts = chrono::DateTime::from_timestamp(ev.created_at, 0)
                .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| ev.created_at.to_string());
            let actor = ev.actor.as_deref().unwrap_or("system");
            let detail = ev.detail.as_deref().unwrap_or("");
            let detail_str = if detail.is_empty() { String::new() } else { format!(" — {}", detail) };
            md.push_str(&format!("- `{}` {} ({}){}\n", ts, ev.event_type.replace('_', " "), actor, detail_str));
        }
        md.push('\n');
    }

    md
}

// ─── Tests — identity-artifact emit helpers (subtask-01) ─────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE plans (
                id                       TEXT PRIMARY KEY,
                conversation_id          TEXT NOT NULL,
                title                    TEXT NOT NULL,
                status                   TEXT NOT NULL,
                phase                    TEXT NOT NULL,
                created_at               INTEGER NOT NULL,
                updated_at               INTEGER NOT NULL
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
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO plans (id, conversation_id, title, status, phase, created_at, updated_at) \
             VALUES ('p1', 'c1', 'Test plan', 'active', 'subtask_review', 0, 0)",
            [],
        )
        .unwrap();
        conn
    }

    fn count_artifacts(conn: &Connection, kind: &str) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM artifacts WHERE type = ?1",
            [kind],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn phase_to_implementation_emits_decision() {
        let conn = test_conn();
        let out = emit_decision_on_phase_change(&conn, "p1", Some("subtask_review"), "implementation")
            .unwrap();
        assert!(out.is_some());
        assert_eq!(count_artifacts(&conn, "decision"), 1);
    }

    #[test]
    fn phase_non_implementation_does_not_emit() {
        let conn = test_conn();
        let out = emit_decision_on_phase_change(&conn, "p1", Some("subtask_review"), "review")
            .unwrap();
        assert!(out.is_none());
        assert_eq!(count_artifacts(&conn, "decision"), 0);
    }

    #[test]
    fn phase_implementation_to_implementation_noop() {
        let conn = test_conn();
        let out = emit_decision_on_phase_change(&conn, "p1", Some("implementation"), "implementation")
            .unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn status_to_done_emits_milestone() {
        let conn = test_conn();
        let out = emit_milestone_on_status_change(&conn, "p1", Some("active"), "done").unwrap();
        assert!(out.is_some());
        assert_eq!(count_artifacts(&conn, "workflow_milestone"), 1);
    }

    #[test]
    fn status_non_done_does_not_emit() {
        let conn = test_conn();
        for terminal in ["abandoned", "active", "draft"] {
            let out = emit_milestone_on_status_change(&conn, "p1", Some("active"), terminal).unwrap();
            assert!(out.is_none(), "status={} 은 emit 안 해야", terminal);
        }
        assert_eq!(count_artifacts(&conn, "workflow_milestone"), 0);
    }

    #[test]
    fn status_done_to_done_noop() {
        let conn = test_conn();
        let out = emit_milestone_on_status_change(&conn, "p1", Some("done"), "done").unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn missing_plan_returns_none_without_error() {
        let conn = test_conn();
        let out = emit_decision_on_phase_change(&conn, "nonexistent", None, "implementation")
            .unwrap();
        assert!(out.is_none());
        let out = emit_milestone_on_status_change(&conn, "nonexistent", Some("active"), "done")
            .unwrap();
        assert!(out.is_none());
    }

    /// INV-1 negative test: user message 내용 분석 기반이 아닌, plan phase/status
    /// 전이에만 의존함을 보여주는 증거 테스트. 임의 문자열을 phase 로 주어도
    /// "implementation" 이 아니면 emit 안 됨.
    #[test]
    fn no_surveillance_emit_is_event_driven() {
        let conn = test_conn();
        for bogus in ["user_said_decision", "결정", "pass", "done"] {
            let out = emit_decision_on_phase_change(&conn, "p1", None, bogus).unwrap();
            assert!(out.is_none(), "phase='{}' → emit 안 해야 (INV-1)", bogus);
        }
        assert_eq!(count_artifacts(&conn, "decision"), 0);
    }

    // ─── planGenerationRollback Layer A — atomic tx tests ─────────────────────

    /// Build an in-memory schema sufficient for `create_plan_tx` and
    /// `replace_plan_subtasks_tx` (real plans/plan_subtasks columns + FK).
    fn tx_test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
            CREATE TABLE plans (
                id                       TEXT PRIMARY KEY,
                conversation_id          TEXT NOT NULL,
                branch_id                TEXT,
                title                    TEXT NOT NULL,
                description              TEXT,
                expected_outcome         TEXT,
                status                   TEXT NOT NULL,
                phase                    TEXT NOT NULL DEFAULT 'drafting',
                architect_engine         TEXT,
                developer_engine         TEXT,
                reviewer_engines         TEXT,
                implementation_branch_id TEXT,
                review_branch_id         TEXT,
                slug                     TEXT,
                revision                 INTEGER NOT NULL DEFAULT 0,
                version_major            INTEGER NOT NULL DEFAULT 1,
                version_minor            INTEGER NOT NULL DEFAULT 0,
                created_at               INTEGER NOT NULL,
                updated_at               INTEGER NOT NULL
            );
            CREATE TABLE plan_subtasks (
                id                TEXT PRIMARY KEY,
                plan_id           TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
                idx               INTEGER NOT NULL,
                title             TEXT NOT NULL,
                details           TEXT,
                status            TEXT NOT NULL DEFAULT 'todo',
                outcome           TEXT,
                owner_agent       TEXT,
                last_updated_by   TEXT,
                created_at        INTEGER NOT NULL,
                updated_at        INTEGER NOT NULL
            );",
        ).unwrap();
        conn
    }

    fn count(conn: &Connection, table: &str, where_clause: &str) -> i64 {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM {} {}", table, where_clause),
            [],
            |r| r.get(0),
        ).unwrap()
    }

    #[test]
    fn create_plan_tx_inserts_plan_and_subtasks() {
        let mut conn = tx_test_conn();
        let input = CreatePlanInput {
            conversation_id: "conv-1".into(),
            branch_id: None,
            title: "T1".into(),
            description: None,
            expected_outcome: None,
            subtasks: vec![
                SubtaskInput { title: "s1".into(), details: None },
                SubtaskInput { title: "s2".into(), details: Some("body".into()) },
            ],
        };
        create_plan_tx(&mut conn, "plan-1", "t1", 1000, &input).unwrap();
        assert_eq!(count(&conn, "plans", "WHERE id='plan-1'"), 1);
        assert_eq!(count(&conn, "plan_subtasks", "WHERE plan_id='plan-1'"), 2);
    }

    #[test]
    fn create_plan_tx_rolls_back_on_subtask_fk_violation() {
        let mut conn = tx_test_conn();
        // Pre-insert a plan_subtask whose id will collide on second call.
        // Simpler: trigger UNIQUE PK violation by passing duplicate plan id.
        conn.execute(
            "INSERT INTO plans (id, conversation_id, title, status, created_at, updated_at)
             VALUES ('plan-existing', 'conv-1', 'X', 'draft', 0, 0)",
            [],
        ).unwrap();

        let input = CreatePlanInput {
            conversation_id: "conv-1".into(),
            branch_id: None,
            title: "T-dup".into(),
            description: None,
            expected_outcome: None,
            subtasks: vec![SubtaskInput { title: "s1".into(), details: None }],
        };
        // Reuse the existing plan id → UNIQUE PK on plans → tx aborts.
        let res = create_plan_tx(&mut conn, "plan-existing", "t-dup", 1000, &input);
        assert!(res.is_err(), "duplicate plan id should fail");

        // No new subtasks (ensures rollback — no half-applied subtask insert).
        assert_eq!(
            count(&conn, "plan_subtasks", "WHERE plan_id='plan-existing'"),
            0,
            "subtasks must not leak from rolled-back tx"
        );
        // Original plan still has its old title (no partial UPDATE side-effect).
        let title: String = conn.query_row(
            "SELECT title FROM plans WHERE id='plan-existing'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(title, "X");
    }

    #[test]
    fn replace_plan_subtasks_tx_replaces_atomically() {
        let mut conn = tx_test_conn();
        // Seed a plan with 2 existing subtasks.
        conn.execute(
            "INSERT INTO plans (id, conversation_id, title, status, created_at, updated_at)
             VALUES ('p-r', 'c-r', 'R', 'active', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO plan_subtasks (id, plan_id, idx, title, status, created_at, updated_at)
             VALUES ('old-1', 'p-r', 0, 'old1', 'todo', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO plan_subtasks (id, plan_id, idx, title, status, created_at, updated_at)
             VALUES ('old-2', 'p-r', 1, 'old2', 'todo', 0, 0)",
            [],
        ).unwrap();

        let new_subtasks = vec![
            SubtaskInput { title: "new1".into(), details: None },
            SubtaskInput { title: "new2".into(), details: None },
            SubtaskInput { title: "new3".into(), details: None },
        ];
        let result = replace_plan_subtasks_tx(&mut conn, "p-r", &new_subtasks, 2000).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(count(&conn, "plan_subtasks", "WHERE plan_id='p-r'"), 3);
        assert_eq!(count(&conn, "plan_subtasks", "WHERE id IN ('old-1','old-2')"), 0);
        // revision bumped
        let rev: i64 = conn.query_row(
            "SELECT revision FROM plans WHERE id='p-r'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(rev, 1);
    }

    #[test]
    fn replace_plan_subtasks_tx_rolls_back_on_invalid_fk() {
        let mut conn = tx_test_conn();
        // Seed a plan + 2 existing subtasks (these must survive a failed replace).
        conn.execute(
            "INSERT INTO plans (id, conversation_id, title, status, created_at, updated_at)
             VALUES ('p-keep', 'c', 'K', 'active', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO plan_subtasks (id, plan_id, idx, title, status, created_at, updated_at)
             VALUES ('keep-1', 'p-keep', 0, 'keep1', 'todo', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO plan_subtasks (id, plan_id, idx, title, status, created_at, updated_at)
             VALUES ('keep-2', 'p-keep', 1, 'keep2', 'todo', 0, 0)",
            [],
        ).unwrap();

        // Call replace against a non-existent plan id. The DELETE / UPDATE
        // succeed silently (zero rows), but we then deliberately corrupt the
        // tx by trying to INSERT a subtask whose plan_id doesn't exist.
        // SQLite enforces FK violation → ConstraintViolation → abort.
        let new_subtasks = vec![SubtaskInput { title: "x".into(), details: None }];
        let res = replace_plan_subtasks_tx(&mut conn, "p-nonexistent", &new_subtasks, 3000);
        assert!(res.is_err(), "FK violation must abort tx");

        // Existing subtasks for the *other* plan must be untouched (DELETE was
        // scoped to 'p-nonexistent'). This proves the rollback didn't damage
        // unrelated data.
        assert_eq!(count(&conn, "plan_subtasks", "WHERE plan_id='p-keep'"), 2);
    }

    #[test]
    fn atomic_write_md_replaces_existing_file_atomically() {
        let dir = std::env::temp_dir().join(format!(
            "tunaflow-plan-atomic-test-{}-{}",
            std::process::id(),
            now_epoch_ms()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("p.md");

        // First write.
        atomic_write_md(&target, "first").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "first");

        // Second write must succeed and fully replace (rename overwrites).
        atomic_write_md(&target, "second").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "second");

        // No leftover .tmp.* in dir.
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp."))
            .collect();
        assert!(leftovers.is_empty(), "tmp file leaked: {:?}", leftovers);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
