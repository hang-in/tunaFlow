use rusqlite::params;
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use crate::db::{
    migrations::{now_epoch, now_epoch_ms},
    models::{Branch, Message},
    DbState,
};
use crate::errors::AppError;

/// Convert a user-provided branch label to git-style slug.
///
/// Rules:
/// - Spaces and common special chars → hyphens
/// - CJK/Korean characters: preserved (valid in git branch names via unicode)
/// - Consecutive hyphens → single hyphen
/// - Leading/trailing hyphens trimmed
///
/// Examples:
/// - "My Feature Branch" → "my-feature-branch"
/// - "Fix: auth bug" → "fix-auth-bug"
/// - "Insight Review (3건)" → "insight-review-3건"
fn slugify_label(label: &str) -> String {
    let mut result = String::with_capacity(label.len());
    let mut prev_hyphen = false;

    for ch in label.chars() {
        if ch.is_whitespace() || matches!(ch, '/' | '\\' | ':' | '!' | '?' | '*' | '[' | ']' | '(' | ')' | '{' | '}' | ',' | ';' | '@' | '#' | '$' | '%' | '^' | '&' | '=' | '+' | '|' | '<' | '>' | '~' | '`' | '\'' | '"')
            || matches!(ch, '\u{2014}' | '\u{2013}' | '\u{2012}' | '\u{2212}') // em dash, en dash, figure dash, minus sign
        {
            if !prev_hyphen && !result.is_empty() {
                result.push('-');
                prev_hyphen = true;
            }
        } else if ch.is_ascii() {
            result.push(ch.to_ascii_lowercase());
            prev_hyphen = false;
        } else {
            // Non-ASCII (Korean, CJK, etc.) — preserve as-is
            result.push(ch);
            prev_hyphen = false;
        }
    }

    // Trim trailing hyphen
    let trimmed = result.trim_end_matches('-');
    trimmed.to_string()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchInput {
    pub conversation_id: String,
    /// Auto-generated as b1, b1.1, etc. if not provided
    pub label: Option<String>,
    pub checkpoint_id: Option<String>,
    pub parent_branch_id: Option<String>,
    /// "chat" (default) | "roundtable"
    pub mode: Option<String>,
    /// Plan subtask this branch implements (developer lane linkage)
    pub subtask_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdoptBranchInput {
    pub branch_id: String,
    pub conversation_id: String,
}

#[tauri::command]
pub fn list_branches(
    conversation_id: String,
    state: State<DbState>,
) -> Result<Vec<Branch>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, label, custom_label, status,
                checkpoint_id, parent_branch_id, session_id, git_branch, mode, subtask_id, created_at
         FROM branches WHERE conversation_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map([&conversation_id], |row| {
            Ok(Branch {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                label: row.get(2)?,
                custom_label: row.get(3)?,
                status: row.get(4)?,
                checkpoint_id: row.get(5)?,
                parent_branch_id: row.get(6)?,
                session_id: row.get(7)?,
                git_branch: row.get(8)?,
                mode: row.get(9)?,
                subtask_id: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn create_branch(
    input: CreateBranchInput,
    state: State<DbState>,
) -> Result<Branch, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let id = Uuid::new_v4().to_string();
    // 프로젝트 컨벤션: 타임스탬프는 ms. 과거 `now_epoch()` 사용으로 branches.created_at
    // 이 초 단위로 저장되던 버그 수정(2026-04-18).
    let now = now_epoch_ms();

    // Resolve root conversation ID — if conversation_id is a shadow conv (branch:xxx),
    // walk up to find the real root conversation
    let root_conv_id = if input.conversation_id.starts_with("branch:") {
        // Shadow conv → look up parent_id chain until we find a non-branch conversation
        let mut current = input.conversation_id.clone();
        loop {
            let parent: Option<String> = conn
                .query_row(
                    "SELECT parent_id FROM conversations WHERE id = ?1",
                    [&current],
                    |row| row.get(0),
                )
                .ok()
                .flatten();
            match parent {
                Some(p) if p.starts_with("branch:") => current = p,
                Some(p) => { current = p; break; }
                None => break, // fallback: use current
            }
        }
        current
    } else {
        input.conversation_id.clone()
    };

    // Auto-generate label: b1, b1.1, b2, etc.
    // User-provided labels are slugified (spaces → hyphens, git-style).
    let label = match input.label {
        Some(l) => {
            let slug = slugify_label(l.trim());
            if slug.is_empty() { l.trim().to_string() } else { slug }
        }
        None => {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM branches WHERE conversation_id = ?1",
                [&root_conv_id],
                |row| row.get(0),
            )?;
            match &input.parent_branch_id {
                None => format!("b{}", count + 1),
                Some(parent_id) => {
                    // Nested: derive from parent label
                    let parent_label: String = conn
                        .query_row(
                            "SELECT label FROM branches WHERE id = ?1",
                            [parent_id],
                            |row| row.get(0),
                        )
                        .unwrap_or_else(|_| format!("b{}", count));
                    let nested_count: i64 = conn.query_row(
                        "SELECT COUNT(*) FROM branches WHERE parent_branch_id = ?1",
                        [parent_id],
                        |row| row.get(0),
                    )?;
                    format!("{}.{}", parent_label, nested_count + 1)
                }
            }
        }
    };

    let branch_mode = input.mode.as_deref().unwrap_or("chat");
    // Resolve git_branch default: inherit from parent branch, or detect from project path
    let git_branch: Option<String> = if let Some(ref parent_id) = input.parent_branch_id {
        // Inherit from parent branch
        conn.query_row("SELECT git_branch FROM branches WHERE id = ?1", [parent_id], |row| row.get(0))
            .ok()
            .flatten()
    } else {
        // Try to detect current git branch from project path
        let project_path: Option<String> = conn
            .query_row("SELECT path FROM projects WHERE key = (SELECT project_key FROM conversations WHERE id = ?1)", [&root_conv_id], |row| row.get(0))
            .ok()
            .flatten();
        project_path.and_then(|p| {
            std::process::Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .current_dir(&p)
                .output()
                .ok()
                .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None })
        })
    };

    conn.execute(
        "INSERT INTO branches
         (id, conversation_id, label, status, checkpoint_id, parent_branch_id, mode, subtask_id, git_branch, created_at)
         VALUES (?1, ?2, ?3, 'active', ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            root_conv_id,
            label,
            input.checkpoint_id,
            input.parent_branch_id,
            branch_mode,
            input.subtask_id,
            git_branch,
            now,
        ],
    )?;

    Ok(Branch {
        id,
        conversation_id: root_conv_id,
        label,
        custom_label: None,
        status: "active".into(),
        checkpoint_id: input.checkpoint_id,
        parent_branch_id: input.parent_branch_id,
        session_id: None,
        git_branch,
        mode: Some(branch_mode.to_string()),
        subtask_id: input.subtask_id,
        created_at: now,
    })
}

/// Open (or ensure) a branch-dedicated conversation stream (DATA_MODEL §1.4, §1.5).
///
/// Branch messages are stored with `conversation_id = "branch:{branch_id}"`.
/// Because the `messages` table has FK → `conversations`, this command
/// creates a shadow `conversations` row with that id on first call (idempotent).
/// Returns the branch conversation id (`"branch:{branch_id}"`).
#[tauri::command]
pub fn open_branch_stream(
    branch_id: String,
    state: State<DbState>,
) -> Result<String, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let branch_conv_id = format!("branch:{}", branch_id);

    // Idempotent: skip creation if shadow row already exists
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM conversations WHERE id = ?1",
            [&branch_conv_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if !exists {
        // Resolve branch → parent conversation → project_key + branch mode
        let (parent_conv_id, branch_label, branch_mode): (String, String, String) = conn
            .query_row(
                "SELECT conversation_id, label, COALESCE(mode, 'chat') FROM branches WHERE id = ?1",
                [&branch_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|_| AppError::NotFound(format!("Branch '{}' not found", branch_id)))?;

        let project_key: String = conn
            .query_row(
                "SELECT project_key FROM conversations WHERE id = ?1",
                [&parent_conv_id],
                |row| row.get(0),
            )
            .map_err(|_| {
                AppError::NotFound(format!("Conversation '{}' not found", parent_conv_id))
            })?;

        // 컨벤션 통일 — conversations.created_at/updated_at 도 ms. 과거 `now_epoch()`
        // 이 섞여 있어 일부 row 는 초, 일부 row 는 ms. 마이그레이션으로 정규화.
        let now = now_epoch_ms();
        conn.execute(
            "INSERT INTO conversations
             (id, project_key, label, type, mode, parent_id, source,
              created_at, updated_at, total_input_tokens, total_output_tokens, total_cost_usd)
             VALUES (?1, ?2, ?3, 'branch', ?4, ?5, 'tunadish', ?6, ?6, 0, 0, 0.0)",
            params![
                branch_conv_id,
                project_key,
                format!("Branch {}", branch_label),
                branch_mode,
                parent_conv_id,
                now,
            ],
        )?;
    }

    Ok(branch_conv_id)
}

/// Set or clear the user-facing display label for a branch.
/// Empty string → NULL (fallback to auto-generated label).
/// Non-empty labels are slugified (spaces → hyphens, git-style).
#[tauri::command]
pub fn rename_branch(id: String, custom_label: String, state: State<DbState>) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let value: Option<String> = if custom_label.trim().is_empty() {
        None
    } else {
        let slug = slugify_label(custom_label.trim());
        Some(if slug.is_empty() { custom_label.trim().to_string() } else { slug })
    };
    conn.execute(
        "UPDATE branches SET custom_label = ?1 WHERE id = ?2",
        params![value, id],
    )?;
    Ok(())
}

/// Link or unlink a git branch to a tunaFlow branch.
#[tauri::command]
pub fn link_git_branch(id: String, git_branch: Option<String>, state: State<DbState>) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let value = git_branch.as_deref().and_then(|v| if v.trim().is_empty() { None } else { Some(v.trim()) });
    conn.execute("UPDATE branches SET git_branch = ?1 WHERE id = ?2", params![value, id])?;
    Ok(())
}

/// Create a real git branch linked to a tunaFlow branch.
#[tauri::command]
pub fn create_git_branch(branch_id: String, state: State<DbState>) -> Result<String, AppError> {
    use std::process::Command;
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let (conv_id, git_branch): (String, Option<String>) = conn
        .query_row("SELECT conversation_id, git_branch FROM branches WHERE id = ?1", [&branch_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|_| AppError::NotFound("Branch not found".into()))?;
    let git_branch = git_branch.ok_or_else(|| AppError::Agent("No git branch linked".into()))?;
    let project_path: String = conn
        .query_row("SELECT path FROM projects WHERE key = (SELECT project_key FROM conversations WHERE id = ?1)", [&conv_id], |row| row.get(0))
        .map_err(|_| AppError::Agent("Project path not found".into()))?;

    let exists = Command::new("git").args(["rev-parse", "--verify", &git_branch]).current_dir(&project_path).output().map(|o| o.status.success()).unwrap_or(false);
    if exists { return Ok(format!("Git branch '{}' already exists", git_branch)); }

    let out = Command::new("git").args(["branch", &git_branch]).current_dir(&project_path).output().map_err(|e| AppError::Agent(e.to_string()))?;
    if !out.status.success() { return Err(AppError::Agent(String::from_utf8_lossy(&out.stderr).trim().to_string())); }
    Ok(format!("Created git branch '{}'", git_branch))
}

/// Checkout linked git branch. Blocks if workspace is dirty.
#[tauri::command]
pub fn checkout_git_branch(branch_id: String, state: State<DbState>) -> Result<String, AppError> {
    use std::process::Command;
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let (conv_id, git_branch): (String, Option<String>) = conn
        .query_row("SELECT conversation_id, git_branch FROM branches WHERE id = ?1", [&branch_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|_| AppError::NotFound("Branch not found".into()))?;
    let git_branch = git_branch.ok_or_else(|| AppError::Agent("No git branch linked".into()))?;
    let project_path: String = conn
        .query_row("SELECT path FROM projects WHERE key = (SELECT project_key FROM conversations WHERE id = ?1)", [&conv_id], |row| row.get(0))
        .map_err(|_| AppError::Agent("Project path not found".into()))?;

    let dirty = Command::new("git").args(["status", "--porcelain"]).current_dir(&project_path).output().map(|o| !o.stdout.is_empty()).unwrap_or(false);
    if dirty { return Err(AppError::Agent("작업 디렉토리에 변경사항이 있습니다. 먼저 commit하거나 stash하세요.".into())); }

    let out = Command::new("git").args(["checkout", &git_branch]).current_dir(&project_path).output().map_err(|e| AppError::Agent(e.to_string()))?;
    if !out.status.success() { return Err(AppError::Agent(String::from_utf8_lossy(&out.stderr).trim().to_string())); }
    Ok(format!("Checked out '{}'", git_branch))
}

/// Archive a branch — sets status to 'archived', preserving messages for read-only viewing.
#[tauri::command]
pub fn archive_branch(
    id: String,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    conn.execute("UPDATE branches SET status = 'archived' WHERE id = ?1", [&id])?;
    Ok(())
}

/// Delete a branch and its descendants.
/// - Active branches: full delete (branch + shadow conv + messages + memos + artifacts)
/// - Adopted/archived branches: pointer-only delete (branch row removed, shadow conv + messages preserved)
///   This mirrors git branch -d: the "commits" (messages) remain accessible via shadow conversation ID.
#[tauri::command]
pub fn delete_branch(
    id: String,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;

    // Collect all descendant branch IDs (recursive) with their status
    let mut to_delete: Vec<(String, String)> = vec![];
    {
        let status: String = conn
            .query_row("SELECT COALESCE(status, 'active') FROM branches WHERE id = ?1", [&id], |row| row.get(0))
            .unwrap_or_else(|_| "active".to_string());
        to_delete.push((id.clone(), status));
    }
    let mut i = 0;
    while i < to_delete.len() {
        let parent_id = to_delete[i].0.clone();
        let mut stmt = conn.prepare("SELECT id, COALESCE(status, 'active') FROM branches WHERE parent_branch_id = ?1")?;
        let children: Vec<(String, String)> = stmt
            .query_map([&parent_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        to_delete.extend(children);
        i += 1;
    }

    // Delete from deepest to shallowest (reverse order)
    for (branch_id, status) in to_delete.iter().rev() {
        let branch_conv_id = format!("branch:{}", branch_id);

        if status == "active" {
            // Active: full delete — remove everything including FK dependents
            conn.execute("DELETE FROM conversation_memory WHERE conversation_id = ?1", [&branch_conv_id])?;
            conn.execute("DELETE FROM trace_log WHERE conversation_id = ?1", [&branch_conv_id])?;
            conn.execute("DELETE FROM agent_jobs WHERE conversation_id = ?1", [&branch_conv_id])?;
            conn.execute("DELETE FROM messages WHERE conversation_id = ?1", [&branch_conv_id])?;
            conn.execute("DELETE FROM memos WHERE conversation_id = ?1", [&branch_conv_id])?;
            conn.execute("DELETE FROM artifacts WHERE conversation_id = ?1", [&branch_conv_id])?;
            conn.execute("DELETE FROM conversations WHERE id = ?1", [&branch_conv_id])?;
        }
        // Adopted/archived: shadow conv + messages preserved (git-style pointer-only delete)

        // plans 3개 FK(branch_id / implementation_branch_id / review_branch_id) 는
        // ON DELETE CASCADE 가 없으므로 branch row 삭제 전에 NULL 로 비운다.
        // 남기지 않으면 `FOREIGN KEY constraint failed` 로 DELETE 실패 (특히 RT
        // Review branch 가 plan.review_branch_id 로 연결된 경우 재현됨).
        conn.execute("UPDATE plans SET branch_id = NULL WHERE branch_id = ?1", [branch_id])?;
        conn.execute("UPDATE plans SET implementation_branch_id = NULL WHERE implementation_branch_id = ?1", [branch_id])?;
        conn.execute("UPDATE plans SET review_branch_id = NULL WHERE review_branch_id = ?1", [branch_id])?;

        // Always remove the branch row itself
        conn.execute("DELETE FROM branches WHERE id = ?1", [branch_id])?;
    }

    Ok(())
}

/// DATA_MODEL §3.3 Adopt flow (simplified):
/// 1. Branch.status → 'adopted'
/// 2. Insert placeholder adopt-summary message in parent Conversation
#[tauri::command]
pub fn adopt_branch(
    input: AdoptBranchInput,
    state: State<DbState>,
) -> Result<Message, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;

    // Validate branch exists and is active
    let (branch_label, branch_mode): (String, String) = conn
        .query_row(
            "SELECT label, COALESCE(mode, 'chat') FROM branches WHERE id = ?1 AND status = 'active'",
            [&input.branch_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| AppError::NotFound(format!("Active branch '{}' not found", input.branch_id)))?;

    // 1. Check content BEFORE changing status — so empty_branch error doesn't leave status as 'adopted'
    let shadow_id = format!("branch:{}", input.branch_id);
    let is_rt = branch_mode == "roundtable";

    // Try to get RT brief first
    let brief: Option<String> = conn
        .query_row(
            "SELECT content FROM memos
             WHERE conversation_id = ?1 AND type = 'roundtable_brief'
             ORDER BY created_at DESC LIMIT 1",
            [&shadow_id],
            |row| row.get(0),
        )
        .ok();

    let summary_body = if let Some(ref brief_content) = brief {
        // Extract Key Positions section from brief
        let lines: Vec<&str> = brief_content.lines().collect();
        let pos_start = lines.iter().position(|l| l.contains("Key Positions"));
        let key_points = if let Some(idx) = pos_start {
            lines[idx + 1..].iter()
                .filter(|l| l.starts_with("- "))
                .take(4)
                .copied()
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            // Fallback: first meaningful lines from brief
            lines.iter()
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .take(3)
                .copied()
                .collect::<Vec<_>>()
                .join("\n")
        };
        format!("### Key Points\n\n{}", key_points)
    } else {
        // Fallback for non-RT or RT without brief: last assistant message from branch
        let last_msg: Option<String> = conn
            .query_row(
                "SELECT content FROM messages
                 WHERE conversation_id = ?1 AND role = 'assistant' AND status = 'done'
                 ORDER BY timestamp DESC LIMIT 1",
                [&shadow_id],
                |row| row.get(0),
            )
            .ok();
        match last_msg {
            Some(msg) => {
                let preview = if msg.len() > 300 {
                    let end = msg.char_indices().map(|(i,_)|i).take_while(|&i| i <= 300).last().unwrap_or(0);
                    format!("{}...", &msg[..end])
                } else {
                    msg
                };
                format!("### Last Response\n\n{}", preview)
            }
            None => String::new(), // Empty branch — frontend will handle
        }
    };

    // If branch has no content, return error BEFORE changing status
    if summary_body.is_empty() {
        return Err(AppError::Agent("empty_branch".into()));
    }

    // Now safe to mark as adopted — content exists
    conn.execute(
        "UPDATE branches SET status = 'adopted' WHERE id = ?1",
        [&input.branch_id],
    )?;

    // Archive all descendant branches recursively
    conn.execute_batch(&format!(
        "WITH RECURSIVE descendants AS (
           SELECT id FROM branches WHERE parent_branch_id = '{bid}'
           UNION ALL
           SELECT b.id FROM branches b JOIN descendants d ON b.parent_branch_id = d.id
         )
         UPDATE branches SET status = 'archived'
         WHERE id IN (SELECT id FROM descendants) AND status = 'active';",
        bid = input.branch_id,
    ))?;

    // Get engine/model from the last assistant message in the branch
    let (last_engine, last_model): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT engine, model FROM messages
             WHERE conversation_id = ?1 AND role = 'assistant' AND status = 'done'
             ORDER BY timestamp DESC LIMIT 1",
            [&shadow_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap_or((None, None));

    let msg_id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    let type_label = if is_rt { "Roundtable" } else { "Thread" };
    let content = format!(
        "## {} Adopted: {}\n\n{}\n",
        type_label, branch_label, summary_body
    );
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, status, engine, model)
         VALUES (?1, ?2, 'assistant', ?3, ?4, 'done', ?5, ?6)",
        params![msg_id, input.conversation_id, content, now, last_engine, last_model],
    )?;

    Ok(Message {
        id: msg_id,
        conversation_id: input.conversation_id,
        role: "assistant".into(),
        content,
        timestamp: now,
        status: "done".into(),
        progress_content: None,
        engine: last_engine,
        model: last_model,
        persona: None,
        duration_ms: None, input_tokens: None, output_tokens: None, cost_usd: None,
    })
}

#[cfg(test)]
mod tests {
    use super::slugify_label;

    #[test]
    fn slugify_spaces_to_hyphens() {
        assert_eq!(slugify_label("My Feature Branch"), "my-feature-branch");
    }

    #[test]
    fn slugify_colon_and_special() {
        assert_eq!(slugify_label("Fix: auth bug"), "fix-auth-bug");
    }

    #[test]
    fn slugify_preserves_korean() {
        assert_eq!(slugify_label("기능 추가"), "기능-추가");
    }

    #[test]
    fn slugify_mixed_korean_english() {
        assert_eq!(slugify_label("Insight Review (3건)"), "insight-review-3건");
    }

    #[test]
    fn slugify_no_trailing_hyphen() {
        assert_eq!(slugify_label("Feature!"), "feature");
    }

    #[test]
    fn slugify_auto_label_unchanged() {
        // Auto-generated labels like "b1", "b1.1" should pass through cleanly
        assert_eq!(slugify_label("b1"), "b1");
        assert_eq!(slugify_label("b2.1"), "b2.1");
    }

    #[test]
    fn slugify_empty_input() {
        assert_eq!(slugify_label(""), "");
    }

    #[test]
    fn slugify_em_dash_separator() {
        // "P21 — 시맨틱 엣지" should collapse the em dash + surrounding spaces into one hyphen
        assert_eq!(slugify_label("P21 — 시맨틱 엣지"), "p21-시맨틱-엣지");
    }

    #[test]
    fn slugify_en_dash_separator() {
        assert_eq!(slugify_label("Feature – sub item"), "feature-sub-item");
    }
}
