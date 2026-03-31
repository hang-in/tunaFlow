use rusqlite::params;
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use std::path::Path;
use crate::db::{migrations::{now_epoch, now_epoch_ms}, models::{Plan, PlanEvent, PlanSubtask}, DbState};
use crate::errors::AppError;

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
        revision: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
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
    "id, conversation_id, branch_id, title, description, expected_outcome, status, phase, architect_engine, developer_engine, reviewer_engines, implementation_branch_id, review_branch_id, revision, created_at, updated_at";

const SUBTASK_COLS: &str =
    "id, plan_id, idx, title, details, status, outcome, owner_agent, last_updated_by, created_at, updated_at";

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Create a plan, optionally with an initial set of subtasks.
/// Returns the created Plan (subtasks can be retrieved via list_subtasks).
#[tauri::command]
pub fn create_plan(
    input: CreatePlanInput,
    state: State<DbState>,
) -> Result<Plan, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();

    conn.execute(
        "INSERT INTO plans
         (id, conversation_id, branch_id, title, description, expected_outcome, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'draft', ?7, ?8)",
        params![
            id,
            input.conversation_id,
            input.branch_id,
            input.title,
            input.description,
            input.expected_outcome,
            now,
            now,
        ],
    )?;

    for (i, st) in input.subtasks.iter().enumerate() {
        let st_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO plan_subtasks
             (id, plan_id, idx, title, details, status, outcome, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'todo', NULL, ?6, ?7)",
            params![st_id, id, i as i64, st.title, st.details, now, now],
        )?;
    }

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
        revision: 0,
        created_at: now,
        updated_at: now,
    })
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
        "SELECT {} FROM plans WHERE conversation_id = ?1 ORDER BY created_at DESC",
        PLAN_COLS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([&conversation_id], map_plan)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Update the status of a plan (draft → active → done | abandoned).
#[tauri::command]
pub fn update_plan_status(
    input: UpdatePlanStatusInput,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    conn.execute(
        "UPDATE plans SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![input.status, now, input.id],
    )?;
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
#[tauri::command]
pub fn replace_plan_subtasks(
    plan_id: String,
    subtasks: Vec<SubtaskInput>,
    state: State<DbState>,
) -> Result<Vec<PlanSubtask>, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();

    conn.execute("DELETE FROM plan_subtasks WHERE plan_id = ?1", [&plan_id])?;
    conn.execute(
        "UPDATE plans SET revision = revision + 1, updated_at = ?1 WHERE id = ?2",
        params![now, plan_id],
    )?;

    let mut result: Vec<PlanSubtask> = Vec::new();
    for (i, st) in subtasks.iter().enumerate() {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO plan_subtasks
             (id, plan_id, idx, title, details, status, outcome, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'todo', NULL, ?6, ?7)",
            params![id, plan_id, i as i64, st.title, st.details, now, now],
        )?;
        result.push(PlanSubtask {
            id,
            plan_id: plan_id.clone(),
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

/// Update the orchestration phase of a plan.
#[tauri::command]
pub fn update_plan_phase(
    id: String,
    phase: String,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    conn.execute(
        "UPDATE plans SET phase = ?1, updated_at = ?2 WHERE id = ?3",
        params![phase, now, id],
    )?;
    Ok(())
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

    // Write to file
    let slug = slugify(&plan.title);
    let dir = Path::new(&project_path).join("docs").join("plans");
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::Agent(format!("Failed to create dir: {}", e)))?;
    let file_path = dir.join(format!("{}.md", slug));
    std::fs::write(&file_path, &md)
        .map_err(|e| AppError::Agent(format!("Failed to write plan doc: {}", e)))?;

    Ok(file_path.to_string_lossy().to_string())
}

fn slugify(title: &str) -> String {
    title.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
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
