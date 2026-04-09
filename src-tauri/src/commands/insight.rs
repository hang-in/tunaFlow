use rusqlite::params;
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;
use std::path::Path;

use crate::db::{migrations::now_epoch, models::{InsightSession, InsightFinding, InsightReport}, DbState};
use crate::errors::AppError;

// ─── Input types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInsightSessionInput {
    pub project_key: String,
    pub categories: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInsightFindingInput {
    pub session_id: String,
    pub project_key: String,
    pub category: String,
    pub severity: String,
    pub fix_difficulty: String,
    pub title: String,
    pub description: String,
    pub file_path: Option<String>,
    pub line_number: Option<i64>,
    pub snippet: Option<String>,
    pub estimated_files: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInsightReportInput {
    pub session_id: String,
    pub project_key: String,
    pub report_type: String,
    pub category: Option<String>,
    pub content: String,
}

// ─── Row mappers ─────────────────────────────────────────────────────────────

const SESSION_COLS: &str =
    "id, project_key, status, categories, test_output, summary, created_at, completed_at";

const FINDING_COLS: &str =
    "id, session_id, project_key, category, severity, fix_difficulty, title, description, \
     file_path, line_number, snippet, estimated_files, resolution, plan_id, status, created_at";

const REPORT_COLS: &str =
    "id, session_id, project_key, type, category, content, created_at";

fn map_session(row: &rusqlite::Row) -> rusqlite::Result<InsightSession> {
    Ok(InsightSession {
        id: row.get(0)?,
        project_key: row.get(1)?,
        status: row.get(2)?,
        categories: row.get(3)?,
        test_output: row.get(4)?,
        summary: row.get(5)?,
        created_at: row.get(6)?,
        completed_at: row.get(7)?,
    })
}

fn map_finding(row: &rusqlite::Row) -> rusqlite::Result<InsightFinding> {
    Ok(InsightFinding {
        id: row.get(0)?,
        session_id: row.get(1)?,
        project_key: row.get(2)?,
        category: row.get(3)?,
        severity: row.get(4)?,
        fix_difficulty: row.get(5)?,
        title: row.get(6)?,
        description: row.get(7)?,
        file_path: row.get(8)?,
        line_number: row.get(9)?,
        snippet: row.get(10)?,
        estimated_files: row.get(11)?,
        resolution: row.get(12)?,
        plan_id: row.get(13)?,
        status: row.get(14)?,
        created_at: row.get(15)?,
    })
}

fn map_report(row: &rusqlite::Row) -> rusqlite::Result<InsightReport> {
    Ok(InsightReport {
        id: row.get(0)?,
        session_id: row.get(1)?,
        project_key: row.get(2)?,
        report_type: row.get(3)?,
        category: row.get(4)?,
        content: row.get(5)?,
        created_at: row.get(6)?,
    })
}

// ─── Session commands ────────────────────────────────────────────────────────

#[tauri::command]
pub fn create_insight_session(
    input: CreateInsightSessionInput,
    state: State<DbState>,
) -> Result<InsightSession, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let id = Uuid::new_v4().to_string();
    let now = now_epoch();
    let categories_json = input.categories.map(|c| serde_json::to_string(&c).unwrap_or_default());
    conn.execute(
        "INSERT INTO insight_sessions (id, project_key, status, categories, created_at)
         VALUES (?1, ?2, 'pending', ?3, ?4)",
        params![id, input.project_key, categories_json, now],
    )?;
    let sql = format!("SELECT {} FROM insight_sessions WHERE id = ?1", SESSION_COLS);
    let session = conn.query_row(&sql, [&id], map_session)
        .map_err(|_| AppError::NotFound("session not found after insert".into()))?;
    Ok(session)
}

#[tauri::command]
pub fn get_insight_session(
    session_id: String,
    state: State<DbState>,
) -> Result<InsightSession, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let sql = format!("SELECT {} FROM insight_sessions WHERE id = ?1", SESSION_COLS);
    conn.query_row(&sql, [&session_id], map_session)
        .map_err(|_| AppError::NotFound("insight session not found".into()))
}

#[tauri::command]
pub fn list_insight_sessions(
    project_key: String,
    state: State<DbState>,
) -> Result<Vec<InsightSession>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let sql = format!(
        "SELECT {} FROM insight_sessions WHERE project_key = ?1 ORDER BY created_at DESC",
        SESSION_COLS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([&project_key], map_session)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn update_insight_session_status(
    session_id: String,
    status: String,
    summary: Option<String>,
    test_output: Option<String>,
    state: State<DbState>,
) -> Result<InsightSession, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let completed_at = if status == "completed" || status == "failed" {
        Some(now_epoch())
    } else {
        None
    };
    conn.execute(
        "UPDATE insight_sessions SET status = ?1, summary = COALESCE(?2, summary),
         test_output = COALESCE(?3, test_output), completed_at = COALESCE(?4, completed_at)
         WHERE id = ?5",
        params![status, summary, test_output, completed_at, session_id],
    )?;
    let sql = format!("SELECT {} FROM insight_sessions WHERE id = ?1", SESSION_COLS);
    conn.query_row(&sql, [&session_id], map_session)
        .map_err(|_| AppError::NotFound("insight session not found".into()))
}

// ─── Finding commands ────────────────────────────────────────────────────────

#[tauri::command]
pub fn create_insight_findings_batch(
    findings: Vec<CreateInsightFindingInput>,
    state: State<DbState>,
) -> Result<Vec<InsightFinding>, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch();
    let mut result = Vec::with_capacity(findings.len());
    for f in findings {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO insight_findings
             (id, session_id, project_key, category, severity, fix_difficulty,
              title, description, file_path, line_number, snippet, estimated_files, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                id, f.session_id, f.project_key, f.category, f.severity, f.fix_difficulty,
                f.title, f.description, f.file_path, f.line_number, f.snippet,
                f.estimated_files.unwrap_or(1), now
            ],
        )?;
        let sql = format!("SELECT {} FROM insight_findings WHERE id = ?1", FINDING_COLS);
        let finding = conn.query_row(&sql, [&id], map_finding)
            .map_err(|_| AppError::NotFound("finding not found after insert".into()))?;
        result.push(finding);
    }
    Ok(result)
}

#[tauri::command]
pub fn list_insight_findings(
    session_id: String,
    category: Option<String>,
    state: State<DbState>,
) -> Result<Vec<InsightFinding>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let (sql, param_values): (String, Vec<String>) = match category {
        Some(ref cat) => (
            format!(
                "SELECT {} FROM insight_findings WHERE session_id = ?1 AND category = ?2
                 ORDER BY CASE severity WHEN 'critical' THEN 0 WHEN 'major' THEN 1
                 WHEN 'minor' THEN 2 ELSE 3 END, created_at",
                FINDING_COLS
            ),
            vec![session_id, cat.clone()],
        ),
        None => (
            format!(
                "SELECT {} FROM insight_findings WHERE session_id = ?1
                 ORDER BY CASE severity WHEN 'critical' THEN 0 WHEN 'major' THEN 1
                 WHEN 'minor' THEN 2 ELSE 3 END, created_at",
                FINDING_COLS
            ),
            vec![session_id],
        ),
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = match param_values.len() {
        1 => stmt.query_map(params![param_values[0]], map_finding)?,
        _ => stmt.query_map(params![param_values[0], param_values[1]], map_finding)?,
    };
    Ok(rows.filter_map(|r| r.ok()).collect())
}

#[tauri::command]
pub fn update_insight_finding_status(
    finding_id: String,
    status: String,
    resolution: Option<String>,
    plan_id: Option<String>,
    state: State<DbState>,
) -> Result<InsightFinding, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    conn.execute(
        "UPDATE insight_findings SET status = ?1,
         resolution = COALESCE(?2, resolution),
         plan_id = COALESCE(?3, plan_id)
         WHERE id = ?4",
        params![status, resolution, plan_id, finding_id],
    )?;
    let sql = format!("SELECT {} FROM insight_findings WHERE id = ?1", FINDING_COLS);
    conn.query_row(&sql, [&finding_id], map_finding)
        .map_err(|_| AppError::NotFound("insight finding not found".into()))
}

#[tauri::command]
pub fn update_insight_findings_batch_status(
    finding_ids: Vec<String>,
    status: String,
    state: State<DbState>,
) -> Result<usize, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let mut count = 0usize;
    for id in &finding_ids {
        count += conn.execute(
            "UPDATE insight_findings SET status = ?1 WHERE id = ?2",
            params![status, id],
        )?;
    }
    Ok(count)
}

/// Resolve all open findings for a session when Plan is done.
#[tauri::command]
pub fn resolve_insight_findings_by_plan(
    plan_id: String,
    state: State<DbState>,
) -> Result<usize, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let count = conn.execute(
        "UPDATE insight_findings SET status = 'resolved'
         WHERE plan_id = ?1 AND status IN ('open', 'selected', 'in_progress')",
        params![plan_id],
    )?;
    Ok(count)
}

// ─── Report commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn create_insight_report(
    input: CreateInsightReportInput,
    state: State<DbState>,
) -> Result<InsightReport, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let id = Uuid::new_v4().to_string();
    let now = now_epoch();
    conn.execute(
        "INSERT INTO insight_reports (id, session_id, project_key, type, category, content, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, input.session_id, input.project_key, input.report_type, input.category, input.content, now],
    )?;
    let sql = format!("SELECT {} FROM insight_reports WHERE id = ?1", REPORT_COLS);
    conn.query_row(&sql, [&id], map_report)
        .map_err(|_| AppError::NotFound("report not found after insert".into()))
}

#[tauri::command]
pub fn list_insight_reports(
    session_id: String,
    state: State<DbState>,
) -> Result<Vec<InsightReport>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let sql = format!(
        "SELECT {} FROM insight_reports WHERE session_id = ?1 ORDER BY created_at",
        REPORT_COLS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([&session_id], map_report)?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ─── Export to files ────────────────────────────────────────────────────────

/// Export insight findings and report to project files (docs/insight/).
/// Creates the directory if it doesn't exist. Returns the number of files written.
#[tauri::command]
pub fn export_insight_to_files(
    session_id: String,
    project_path: String,
    state: State<DbState>,
) -> Result<usize, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;

    // Load findings
    let sql = format!(
        "SELECT {} FROM insight_findings WHERE session_id = ?1
         ORDER BY CASE severity WHEN 'critical' THEN 0 WHEN 'major' THEN 1 WHEN 'minor' THEN 2 ELSE 3 END",
        FINDING_COLS
    );
    let mut stmt = conn.prepare(&sql)?;
    let findings: Vec<InsightFinding> = stmt.query_map([&session_id], map_finding)?
        .filter_map(|r| r.ok()).collect();

    // Load session
    let ssql = format!("SELECT {} FROM insight_sessions WHERE id = ?1", SESSION_COLS);
    let session = conn.query_row(&ssql, [&session_id], map_session)
        .map_err(|_| AppError::NotFound("session not found".into()))?;

    if findings.is_empty() {
        return Ok(0);
    }

    // Create directory
    let insight_dir = Path::new(&project_path).join("docs").join("insight");
    let findings_dir = insight_dir.join("findings");
    std::fs::create_dir_all(&findings_dir)
        .map_err(|e| AppError::Agent(format!("Failed to create insight dir: {}", e)))?;

    let mut count = 0usize;

    // Write individual finding files
    for f in &findings {
        let slug = f.title.chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .collect::<String>()
            .to_lowercase();
        let slug = slug.trim_matches('-');
        let filename = format!("{}-{}.md", f.category.chars().take(3).collect::<String>().to_uppercase(), slug);
        let mut md = format!(
            "# {}\n\n- **Category**: {}\n- **Severity**: {}\n- **Fix Difficulty**: {}\n- **Status**: {}\n",
            f.title, f.category, f.severity, f.fix_difficulty, f.status
        );
        if let Some(ref fp) = f.file_path {
            md.push_str(&format!("- **File**: {}{}\n", fp,
                f.line_number.map(|n| format!(":{}", n)).unwrap_or_default()));
        }
        md.push_str(&format!("\n## Description\n\n{}\n", f.description));
        if let Some(ref snippet) = f.snippet {
            md.push_str(&format!("\n## Snippet\n\n```\n{}\n```\n", snippet));
        }
        if let Some(ref resolution) = f.resolution {
            md.push_str(&format!("\n## Resolution\n\n{}\n", resolution));
        }
        let path = findings_dir.join(&filename);
        std::fs::write(&path, &md)
            .map_err(|e| AppError::Agent(format!("Failed to write finding {}: {}", filename, e)))?;
        count += 1;
    }

    // Write latest-report.md (summary + all findings)
    let ts = chrono::NaiveDateTime::from_timestamp_opt(session.created_at, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| session.created_at.to_string());

    let mut report = format!("# Insight Report — {}\n\n", ts);
    if let Some(ref summary) = session.summary {
        report.push_str(&format!("{}\n\n", summary));
    }

    // Group by category
    let mut by_cat: std::collections::BTreeMap<String, Vec<&InsightFinding>> = std::collections::BTreeMap::new();
    for f in &findings {
        by_cat.entry(f.category.clone()).or_default().push(f);
    }

    for (cat, cat_findings) in &by_cat {
        report.push_str(&format!("## {}\n\n", cat));
        for f in cat_findings {
            let status_icon = match f.status.as_str() {
                "resolved" => "✅",
                "dismissed" => "⊘",
                "in_progress" => "🔧",
                _ => "⬜",
            };
            report.push_str(&format!("- {} **{}** [{}] — {}\n",
                status_icon, f.title, f.severity,
                f.file_path.as_deref().unwrap_or("")));
        }
        report.push('\n');
    }

    let report_path = insight_dir.join("latest-report.md");
    std::fs::write(&report_path, &report)
        .map_err(|e| AppError::Agent(format!("Failed to write report: {}", e)))?;
    count += 1;

    Ok(count)
}
