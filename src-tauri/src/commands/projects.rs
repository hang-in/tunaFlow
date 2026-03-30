use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use rusqlite::params;
use serde::Deserialize;
use tauri::State;

use crate::db::{migrations::now_epoch, models::Project, DbState};
use crate::errors::AppError;

/// Tracks project paths currently being indexed by rawq (prevents duplicate builds).
pub struct RawqIndexing(pub Arc<Mutex<HashSet<String>>>);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectInput {
    pub key: String,
    pub name: String,
    pub path: Option<String>,
    #[serde(rename = "type")]
    pub project_type: String,
    pub default_engine: Option<String>,
    pub workspace_root: Option<String>,
    pub source: String,
}

#[tauri::command]
pub fn list_projects(state: State<DbState>) -> Result<Vec<Project>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let mut stmt = conn.prepare(
        "SELECT key, name, path, type, default_engine, workspace_root, source, updated_at
         FROM projects WHERE COALESCE(hidden, 0) = 0 ORDER BY updated_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Project {
                key: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                project_type: row.get(3)?,
                default_engine: row.get(4)?,
                workspace_root: row.get(5)?,
                source: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn create_project(
    input: CreateProjectInput,
    state: State<DbState>,
) -> Result<Project, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;

    // Normalize path for duplicate check (canonicalize resolves symlinks, case, slashes)
    let normalized_path = input.path.as_ref().and_then(|p| {
        std::fs::canonicalize(p).ok().map(|c| c.to_string_lossy().to_string())
    });

    // Duplicate path check — restore hidden project or reject active duplicate
    if let Some(ref np) = normalized_path {
        let existing: Option<(String, i64)> = conn
            .query_row(
                "SELECT key, COALESCE(hidden, 0) FROM projects WHERE path = ?1",
                [np],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        // Also check non-canonicalized path as fallback
        let existing = existing.or_else(|| {
            input.path.as_ref().and_then(|p| {
                conn.query_row(
                    "SELECT key, COALESCE(hidden, 0) FROM projects WHERE path = ?1",
                    [p],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok()
            })
        });
        if let Some((existing_key, hidden)) = existing {
            if hidden == 1 {
                // Restore hidden project — unhide and update metadata
                conn.execute(
                    "UPDATE projects SET hidden = 0, name = ?1, updated_at = ?2 WHERE key = ?3",
                    params![input.name, now_epoch(), existing_key],
                )?;
                // Scaffold on restore too (creates only missing files)
                if let Some(ref np) = normalized_path {
                    scaffold_project_dir(np, &input.name);
                }
                return conn.query_row(
                    "SELECT key, name, path, type, default_engine, workspace_root, source, updated_at
                     FROM projects WHERE key = ?1",
                    [&existing_key],
                    |row| Ok(Project {
                        key: row.get(0)?, name: row.get(1)?, path: row.get(2)?,
                        project_type: row.get(3)?, default_engine: row.get(4)?,
                        workspace_root: row.get(5)?, source: row.get(6)?, updated_at: row.get(7)?,
                    }),
                ).map_err(|_| AppError::NotFound("restored project not found".into()));
            }
            return Err(AppError::Agent(format!(
                "이 경로는 이미 프로젝트 '{}'로 등록되어 있습니다",
                existing_key
            )));
        }
    }

    let now = now_epoch();
    let store_path = normalized_path.or(input.path.clone());

    conn.execute(
        "INSERT INTO projects (key, name, path, type, default_engine, workspace_root, source, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            input.key,
            input.name,
            store_path,
            input.project_type,
            input.default_engine,
            input.workspace_root,
            input.source,
            now,
        ],
    )?;

    // Auto-create default conversation so the project is immediately usable
    let conv_id = uuid::Uuid::new_v4().to_string();
    let conv_now = crate::db::migrations::now_epoch_ms() / 1000;
    conn.execute(
        "INSERT INTO conversations (id, project_key, label, type, mode, source, created_at, updated_at,
         total_input_tokens, total_output_tokens, total_cost_usd)
         VALUES (?1, ?2, 'Main', 'main', 'chat', 'tunadish', ?3, ?3, 0, 0, 0.0)",
        params![conv_id, input.key, conv_now],
    )?;

    // Scaffold project directory with convention files
    if let Some(ref path) = store_path {
        scaffold_project_dir(path, &input.name);
    }

    Ok(Project {
        key: input.key,
        name: input.name,
        path: store_path,
        project_type: input.project_type,
        default_engine: input.default_engine,
        workspace_root: input.workspace_root,
        source: input.source,
        updated_at: now,
    })
}

/// Create minimal project directory structure and convention files.
///
/// Only creates files that don't already exist — safe to call on existing projects.
fn scaffold_project_dir(project_path: &str, project_name: &str) {
    use std::fs;
    use std::path::Path;

    let root = Path::new(project_path);
    if !root.is_dir() { return; }

    // Create standard directories
    for dir in &["docs/plans", "docs/reference", "docs/prompts"] {
        let _ = fs::create_dir_all(root.join(dir));
    }

    // CLAUDE.md — agent convention file (only if not present)
    let claude_md = root.join("CLAUDE.md");
    if !claude_md.exists() {
        let content = generate_claude_md(project_name);
        let _ = fs::write(&claude_md, content);
        eprintln!("[scaffold] created {}", claude_md.display());
    }

    // docs/plans/index.md
    let plans_index = root.join("docs/plans/index.md");
    if !plans_index.exists() {
        let _ = fs::write(&plans_index, "# Plans\n\nPlan document index. Register new plans here.\n");
    }

    // docs/reference/index.md
    let ref_index = root.join("docs/reference/index.md");
    if !ref_index.exists() {
        let _ = fs::write(&ref_index, "# Reference\n\nReference document index.\n");
    }
}

/// Generate a comprehensive CLAUDE.md for a new project.
fn generate_claude_md(project_name: &str) -> String {
    format!(r#"# {project_name} — Agent Instructions

> This file defines project-level rules for all agents in tunaFlow.
> All agents (Claude, Gemini, Codex, OpenCode) must follow these rules.

---

## 1. Project Overview

- Name: {project_name}
- Status: initial setup

> Describe project purpose and tech stack here.

---

## 2. File Storage Rules

**All documents and artifacts must be created within this project directory.**

- Do NOT create files in `~/.claude/`, `~/.gemini/`, or any external path
- Plans: `docs/plans/`
- Reference docs: `docs/reference/`
- Prompts: `docs/prompts/`
- Code: follow project structure

---

## 3. Documentation Rules

### File Naming
- Short, 2-4 core tokens (camelCase)
- Reference: stable names without dates (e.g., `implementationStatus.md`)
- Plan: `featureNamePlan.md` or `featureNamePlan_YYYY-MM-DD.md`
- Prompt: `docs/prompts/YYYY-MM-DD/short_name.md`

### Document Metadata
- Top of every document: `type`, `status`, `updated_at`
- Status values: `draft` → `in_progress` → `done` → `archived`
- Reference docs: update same file (no date-based duplication)
- Plans/prompts: new documents per task allowed (must update index.md)

### Versioning
- Use `status: archived` + `superseded_by` instead of deletion
- Brainstorm/comparison docs: mark `canonical: false`

---

## 4. Coding Rules

### Language
- Respond in the language the user uses (match user's message language)
- Code, paths, identifiers: keep in original language

### Code Quality
- Only modify what was requested. Do not clean up surrounding code
- Error handling: minimize silent fallbacks during development
- No speculative abstractions or future-proofing
- Modify one path at a time → verify → proceed to next

### Testing
- Verify existing tests pass after changes
- Consider unit tests for new logic

---

## 5. Work Safety Rules

- **Verify replacement works** before removing existing functionality
- **Confirm before destructive operations** (file deletion, schema changes)
- **Single-path modification** — never change multiple execution paths simultaneously
- Check all consumers before modifying shared state

---

## 6. Agent Behavior Rules

- **Plan before implementing** — present your plan and wait for user approval before writing code
- Introduce yourself by profile name first, then engine. No mixed expressions
- Do not claim ownership of other agents' messages
- Respond in the user's language
- Lead with conclusions, then reasoning

---

## 7. Current Status

### Completed
- (record here)

### In Progress
- (record here)

### Known Issues
- (record here)

---

## 8. Next Priorities

1. (record here)
"#)
}


/// Hide a project from the list without deleting any data.
#[tauri::command]
pub fn hide_project(key: String, state: State<DbState>) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    conn.execute(
        "UPDATE projects SET hidden = 1, updated_at = ?1 WHERE key = ?2",
        params![now_epoch(), key],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_project(key: String, state: State<DbState>) -> Result<Project, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    conn.query_row(
        "SELECT key, name, path, type, default_engine, workspace_root, source, updated_at
         FROM projects WHERE key = ?1",
        [&key],
        |row| {
            Ok(Project {
                key: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                project_type: row.get(3)?,
                default_engine: row.get(4)?,
                workspace_root: row.get(5)?,
                source: row.get(6)?,
                updated_at: row.get(7)?,
            })
        },
    )
    .map_err(|_| AppError::NotFound(format!("Project '{}' not found", key)))
}

/// Validate a project path before creation.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathValidation {
    pub valid: bool,
    pub exists: bool,
    pub is_dir: bool,
    pub normalized_path: String,
    pub error: Option<String>,
}

#[tauri::command]
pub fn validate_project_path(path: String) -> Result<PathValidation, AppError> {
    let trimmed = path.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        return Ok(PathValidation {
            valid: false, exists: false, is_dir: false,
            normalized_path: String::new(),
            error: Some("경로를 입력하세요".into()),
        });
    }

    let p = std::path::Path::new(trimmed);
    let exists = p.exists();
    let is_dir = p.is_dir();

    if !exists {
        return Ok(PathValidation {
            valid: false, exists: false, is_dir: false,
            normalized_path: trimmed.to_string(),
            error: Some("존재하지 않는 경로입니다".into()),
        });
    }
    if !is_dir {
        return Ok(PathValidation {
            valid: false, exists: true, is_dir: false,
            normalized_path: trimmed.to_string(),
            error: Some("디렉토리만 등록할 수 있습니다".into()),
        });
    }

    Ok(PathValidation {
        valid: true, exists: true, is_dir: true,
        normalized_path: trimmed.to_string(),
        error: None,
    })
}

/// Structured rawq status returned to frontend.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawqStatus {
    pub available: bool,
    pub indexed: bool,
    /// "ready" | "built" | "error" | "unavailable"
    pub status: String,
    pub message: String,
    pub files: Option<u64>,
    pub chunks: Option<u64>,
}

/// Get rawq status for a project path without triggering a build.
#[tauri::command]
pub fn get_rawq_status(project_path: String) -> Result<RawqStatus, AppError> {
    use crate::agents::rawq;

    // Check binary availability
    let bin_ok = rawq::is_available();
    if !bin_ok {
        return Ok(RawqStatus {
            available: false, indexed: false,
            status: "unavailable".into(), message: "rawq not found".into(),
            files: None, chunks: None,
        });
    }

    // Check index status via CLI
    match rawq::index_status(&project_path) {
        Ok(Some(info)) => Ok(RawqStatus {
            available: true, indexed: true,
            status: "ready".into(),
            message: format!("{} files, {} chunks", info.files, info.chunks),
            files: Some(info.files), chunks: Some(info.chunks),
        }),
        Ok(None) => Ok(RawqStatus {
            available: true, indexed: false,
            status: "ready".into(), message: "not indexed".into(),
            files: None, chunks: None,
        }),
        Err(e) => Ok(RawqStatus {
            available: true, indexed: false,
            status: "error".into(), message: format!("{}", e),
            files: None, chunks: None,
        }),
    }
}

/// Ensure rawq index exists, returning structured status.
/// NOTE: This is a blocking command. For non-blocking use `start_rawq_index`.
#[tauri::command]
pub fn ensure_rawq_index(project_path: String) -> Result<RawqStatus, AppError> {
    use crate::agents::rawq;

    match rawq::ensure_index(&project_path) {
        Ok(0) => {
            let (files, chunks) = rawq::index_status(&project_path)
                .ok()
                .flatten()
                .map(|i| (Some(i.files), Some(i.chunks)))
                .unwrap_or((None, None));
            Ok(RawqStatus {
                available: true, indexed: true,
                status: "ready".into(), message: "already indexed".into(),
                files, chunks,
            })
        }
        Ok(n) => Ok(RawqStatus {
            available: true, indexed: true,
            status: "built".into(), message: format!("indexed {} files", n),
            files: Some(n), chunks: None,
        }),
        Err(e) => {
            eprintln!("[ensure_rawq_index] {}", e);
            let available = !matches!(e, rawq::RawqError::NotFound(_));
            Ok(RawqStatus {
                available, indexed: false,
                status: if available { "error" } else { "unavailable" }.into(),
                message: format!("{}", e),
                files: None, chunks: None,
            })
        }
    }
}

/// Start rawq index build in background thread. Emits events:
/// - `rawq:indexing` — { projectPath, message }
/// - `rawq:indexed`  — RawqStatus (success)
/// - `rawq:error`    — RawqStatus (failure)
#[tauri::command]
pub fn start_rawq_index(
    project_path: String,
    app: tauri::AppHandle,
    indexing: State<RawqIndexing>,
) -> Result<(), AppError> {
    use crate::agents::rawq;
    use tauri::Emitter;

    // Duplicate guard — skip if already indexing this path
    {
        let mut set = indexing.0.lock().map_err(|_| AppError::Lock)?;
        if set.contains(&project_path) {
            eprintln!("[rawq] already indexing {}, skipping", project_path);
            return Ok(());
        }
        set.insert(project_path.clone());
    }
    let guard = indexing.0.clone();

    let _ = app.emit("rawq:indexing", serde_json::json!({
        "projectPath": &project_path,
        "message": "Building code index..."
    }));

    std::thread::spawn(move || {
        let result = match rawq::ensure_index(&project_path) {
            Ok(0) => {
                let (files, chunks) = rawq::index_status(&project_path)
                    .ok()
                    .flatten()
                    .map(|i| (Some(i.files), Some(i.chunks)))
                    .unwrap_or((None, None));
                RawqStatus {
                    available: true, indexed: true,
                    status: "ready".into(), message: "already indexed".into(),
                    files, chunks,
                }
            }
            Ok(n) => RawqStatus {
                available: true, indexed: true,
                status: "built".into(), message: format!("indexed {} files", n),
                files: Some(n), chunks: None,
            },
            Err(e) => {
                eprintln!("[start_rawq_index] {}", e);
                let available = !matches!(e, rawq::RawqError::NotFound(_));
                RawqStatus {
                    available, indexed: false,
                    status: if available { "error" } else { "unavailable" }.into(),
                    message: format!("{}", e),
                    files: None, chunks: None,
                }
            }
        };

        let event = if result.indexed { "rawq:indexed" } else { "rawq:error" };
        let _ = app.emit(event, &result);

        // Release guard
        if let Ok(mut set) = guard.lock() {
            set.remove(&project_path);
        }
    });

    Ok(())
}

/// Git status for a project path.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub dirty: bool,
    pub git_root: Option<String>,
}

#[tauri::command]
pub fn get_git_status(project_path: String) -> Result<GitStatus, AppError> {
    use std::process::Command;
    let path = std::path::Path::new(&project_path);
    if !path.exists() {
        return Ok(GitStatus { is_repo: false, branch: None, dirty: false, git_root: None });
    }

    // Check if git repo
    let is_repo = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&project_path)
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false);

    if !is_repo {
        return Ok(GitStatus { is_repo: false, branch: None, dirty: false, git_root: None });
    }

    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&project_path)
        .output()
        .ok()
        .and_then(|o| if o.status.success() {
            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
        } else { None });

    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&project_path)
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    let git_root = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&project_path)
        .output()
        .ok()
        .and_then(|o| if o.status.success() {
            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
        } else { None });

    Ok(GitStatus { is_repo, branch, dirty, git_root })
}
