use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use rusqlite::params;
use serde::Deserialize;
use tauri::State;

use crate::db::{migrations::now_epoch, models::Project, DbState};
use crate::errors::AppError;

/// State for rawq indexing.
///
/// - `active` — set of project paths currently being indexed (duplicate
///   guard). Membership is held only for the duration of one build.
/// - `cancels` — per-path cancel flags shared with the rawq subprocess
///   poll loop in `agents::rawq::ensure_index_cancellable`. Stored under
///   the same lock as `active` so insert/remove stays atomic relative to
///   the duplicate guard.
///
/// See `docs/plans/rawqIndexCancelChannelPlan_2026-04-25.md` for the
/// invariants the cancel channel preserves (INV-1/2/3).
pub struct RawqIndexing {
    pub active: Arc<parking_lot::Mutex<HashSet<String>>>,
    pub cancels: Arc<parking_lot::Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl RawqIndexing {
    pub fn new() -> Self {
        Self {
            active: Arc::new(parking_lot::Mutex::new(HashSet::new())),
            cancels: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        }
    }
}

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

/// 최근 열었던 프로젝트 목록 — `last_opened_at DESC` 순.
///
/// `globalSettingsAndRecentProjectsPlan_2026-04-29.md` Task 02 의 G3.
/// `list_projects` 와 동일한 hidden 필터 + Project shape 를 유지하지만,
/// `last_opened_at = 0` (한 번도 열지 않은 프로젝트) 은 결과에서 제외 —
/// "최근" 의 정의에 부합. 정렬은 last_opened_at DESC, tie-break 으로
/// updated_at DESC.
///
/// `limit` 은 UI 측 (5 권장) 에서 제어. 0 또는 음수면 기본값 5 적용.
#[tauri::command]
pub fn list_recent_projects(
    limit: Option<i64>,
    state: State<DbState>,
) -> Result<Vec<Project>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let lim = limit.filter(|n| *n > 0).unwrap_or(5);
    let mut stmt = conn.prepare(
        "SELECT key, name, path, type, default_engine, workspace_root, source, updated_at
         FROM projects
         WHERE COALESCE(hidden, 0) = 0 AND COALESCE(last_opened_at, 0) > 0
         ORDER BY last_opened_at DESC, updated_at DESC
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![lim], |row| {
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

/// 프로젝트 선택 시 `last_opened_at` 을 현재 시각(ms) 으로 갱신.
///
/// `selectProject` 가 호출하므로 idempotent 해야 한다 — 동일 프로젝트
/// 재선택 시 단순히 timestamp 만 갱신. 알 수 없는 key 는 no-op (UPDATE rows = 0).
#[tauri::command]
pub fn touch_project_opened_at(key: String, state: State<DbState>) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    conn.execute(
        "UPDATE projects SET last_opened_at = ?1 WHERE key = ?2",
        params![crate::db::migrations::now_epoch_ms(), key],
    )?;
    Ok(())
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
        let content = generate_claude_md(project_name, Some(project_path));
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

    // docs/agentSessionHistory.md — agent session log (only if not present)
    let session_history = root.join("docs/agentSessionHistory.md");
    if !session_history.exists() {
        let _ = fs::write(
            &session_history,
            "# Agent Session History\n\nRecord key decisions, completed work, and context from each session.\nUpdate at the end of each session.\n\n---\n\n## Session Log\n\n<!-- Newest entries first -->\n",
        );
        eprintln!("[scaffold] created {}", session_history.display());
    }

    // Workflow agent templates — always ensure these exist
    super::project_tools::ensure_workflow_templates(project_path);
}

/// Update the §1 Project Overview section of an existing CLAUDE.md with auto-detected stack info.
/// Replaces everything between "## 1. Project Overview" and the next "---" separator.
/// If the section already contains auto-detected info, it's refreshed.
/// If the section was manually edited (no "Auto-detected" marker), it's left alone.
#[tauri::command]
pub fn refresh_project_stack_info(project_path: String, project_name: String) -> Result<bool, AppError> {
    use std::path::Path;

    let claude_md_path = Path::new(&project_path).join("CLAUDE.md");
    if !claude_md_path.is_file() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(&claude_md_path)
        .map_err(|e| AppError::Agent(format!("read CLAUDE.md: {}", e)))?;

    // Find §1 section
    let section_start = match content.find("## 1. Project Overview") {
        Some(pos) => pos,
        None => return Ok(false), // No §1 section
    };
    let section_end = content[section_start + 10..].find("\n---").map(|p| section_start + 10 + p);
    let section_end = match section_end {
        Some(pos) => pos,
        None => return Ok(false),
    };

    let existing_section = &content[section_start..section_end];

    // Only update if it contains the auto-detected marker or the placeholder
    if !existing_section.contains("Auto-detected") && !existing_section.contains("Describe project purpose") {
        eprintln!("[scaffold] §1 was manually edited — skipping stack refresh");
        return Ok(false);
    }

    let info = detect_project_info(&project_path);
    if info.detected_stack.is_empty() {
        return Ok(false); // Nothing to detect
    }

    let mut lines = vec![
        "## 1. Project Overview".to_string(),
        String::new(),
        format!("- Name: {}", project_name),
        "- Status: active".to_string(),
    ];
    if let Some(lang) = &info.language { lines.push(format!("- Language: {}", lang)); }
    if let Some(fw) = &info.framework { lines.push(format!("- Framework: {}", fw)); }
    if let Some(tc) = &info.test_command { lines.push(format!("- Test: `{}`", tc)); }
    if let Some(tcc) = &info.type_check_command { lines.push(format!("- Type check: `{}`", tcc)); }
    if let Some(bc) = &info.build_command { lines.push(format!("- Build: `{}`", bc)); }
    lines.push(format!("- Stack: {}", info.detected_stack.join(", ")));
    lines.push(String::new());
    lines.push("> Auto-detected by tunaFlow. Verify and adjust if needed.".into());

    let new_section = lines.join("\n");
    let updated = format!("{}{}\n{}", &content[..section_start], new_section, &content[section_end..]);

    std::fs::write(&claude_md_path, updated)
        .map_err(|e| AppError::Agent(format!("write CLAUDE.md: {}", e)))?;
    eprintln!("[scaffold] refreshed §1 Project Overview with stack: {:?}", info.detected_stack);

    Ok(true)
}

/// Detected project information from manifest files.
pub struct ProjectInfo {
    pub framework: Option<String>,
    pub language: Option<String>,
    pub test_command: Option<String>,
    pub type_check_command: Option<String>,
    pub build_command: Option<String>,
    pub detected_stack: Vec<String>,
}

/// Detect project info from manifest files (package.json, Cargo.toml, pyproject.toml).
pub fn detect_project_info(project_path: &str) -> ProjectInfo {
    let root = std::path::Path::new(project_path);
    let mut info = ProjectInfo {
        framework: None, language: None, test_command: None,
        type_check_command: None, build_command: None, detected_stack: Vec::new(),
    };

    // ── package.json ──
    let pkg_json = root.join("package.json");
    if pkg_json.is_file() {
        if let Ok(text) = std::fs::read_to_string(&pkg_json) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                info.language = Some("TypeScript/JavaScript".into());

                // Scripts
                if let Some(scripts) = val.get("scripts").and_then(|v| v.as_object()) {
                    if let Some(test) = scripts.get("test").and_then(|v| v.as_str()) {
                        info.test_command = Some(format!("npm test (→ {})", test));
                    }
                    if let Some(build) = scripts.get("build").and_then(|v| v.as_str()) {
                        info.build_command = Some(format!("npm run build (→ {})", build));
                    }
                    // Detect type check from scripts
                    for key in ["typecheck", "type-check", "lint:types"] {
                        if let Some(cmd) = scripts.get(key).and_then(|v| v.as_str()) {
                            info.type_check_command = Some(format!("npm run {} (→ {})", key, cmd));
                            break;
                        }
                    }
                }

                // Framework detection from dependencies
                let deps_iter = ["dependencies", "devDependencies"].iter()
                    .filter_map(|s| val.get(s).and_then(|v| v.as_object()))
                    .flat_map(|obj| obj.keys().map(|k| k.to_lowercase()));
                let dep_set: std::collections::HashSet<String> = deps_iter.collect();

                // Framework priority: most specific first
                let framework = if dep_set.contains("nuxt") { "Nuxt 3" }
                    else if dep_set.contains("next") { "Next.js" }
                    else if dep_set.contains("svelte") || dep_set.contains("@sveltejs/kit") { "SvelteKit" }
                    else if dep_set.contains("vue") { "Vue 3" }
                    else if dep_set.contains("react") { "React" }
                    else if dep_set.contains("express") { "Express" }
                    else if dep_set.contains("fastify") { "Fastify" }
                    else if dep_set.contains("hono") { "Hono" }
                    else { "" };
                if !framework.is_empty() {
                    info.framework = Some(framework.into());
                    info.detected_stack.push(framework.into());
                }

                // Test framework
                let test_fw = if dep_set.contains("vitest") { "vitest" }
                    else if dep_set.contains("jest") { "jest" }
                    else if dep_set.contains("mocha") { "mocha" }
                    else if dep_set.contains("ava") { "ava" }
                    else { "" };
                if !test_fw.is_empty() {
                    info.detected_stack.push(test_fw.into());
                    if info.test_command.is_none() {
                        info.test_command = Some(format!("npx {} run", test_fw));
                    }
                }

                // TypeScript check
                if dep_set.contains("typescript") {
                    info.detected_stack.push("TypeScript".into());
                    if info.type_check_command.is_none() {
                        // Nuxt uses vue-tsc, others use tsc
                        if dep_set.contains("nuxt") || dep_set.contains("vue-tsc") {
                            info.type_check_command = Some("npx vue-tsc --noEmit".into());
                        } else {
                            info.type_check_command = Some("npx tsc --noEmit".into());
                        }
                    }
                }

                // Other notable deps
                for dep in ["tailwindcss", "prisma", "drizzle-orm", "zustand", "pinia", "trpc"] {
                    if dep_set.contains(dep) { info.detected_stack.push(dep.into()); }
                }
            }
        }
    }

    // ── Cargo.toml ──
    let cargo_toml = root.join("Cargo.toml");
    if cargo_toml.is_file() {
        info.language = Some(info.language.map_or("Rust".into(), |l| format!("{} + Rust", l)));
        info.detected_stack.push("Rust".into());
        if info.test_command.is_none() {
            info.test_command = Some("cargo test".into());
        }
        if info.type_check_command.is_none() {
            info.type_check_command = Some("cargo check".into());
        }
    }

    // ── pyproject.toml / requirements.txt ──
    let has_python = root.join("pyproject.toml").is_file() || root.join("requirements.txt").is_file();
    if has_python {
        info.language = Some(info.language.map_or("Python".into(), |l| format!("{} + Python", l)));
        info.detected_stack.push("Python".into());
        if root.join("pyproject.toml").is_file() {
            if let Ok(text) = std::fs::read_to_string(root.join("pyproject.toml")) {
                if text.contains("pytest") {
                    info.detected_stack.push("pytest".into());
                    if info.test_command.is_none() {
                        info.test_command = Some("pytest".into());
                    }
                }
            }
        }
    }

    // ── Go ──
    if root.join("go.mod").is_file() {
        info.language = Some(info.language.map_or("Go".into(), |l| format!("{} + Go", l)));
        info.detected_stack.push("Go".into());
        if info.test_command.is_none() { info.test_command = Some("go test ./...".into()); }
    }

    info
}

/// Generate a comprehensive CLAUDE.md for a new project.
fn generate_claude_md(project_name: &str, project_path: Option<&str>) -> String {
    let info = project_path.map(|p| detect_project_info(p)).unwrap_or(ProjectInfo {
        framework: None, language: None, test_command: None,
        type_check_command: None, build_command: None, detected_stack: Vec::new(),
    });

    let stack_section = if info.detected_stack.is_empty() {
        "> Describe project purpose and tech stack here.".to_string()
    } else {
        let mut lines = Vec::new();
        if let Some(lang) = &info.language { lines.push(format!("- Language: {}", lang)); }
        if let Some(fw) = &info.framework { lines.push(format!("- Framework: {}", fw)); }
        if let Some(tc) = &info.test_command { lines.push(format!("- Test: `{}`", tc)); }
        if let Some(tcc) = &info.type_check_command { lines.push(format!("- Type check: `{}`", tcc)); }
        if let Some(bc) = &info.build_command { lines.push(format!("- Build: `{}`", bc)); }
        if !info.detected_stack.is_empty() {
            lines.push(format!("- Stack: {}", info.detected_stack.join(", ")));
        }
        lines.push(String::new());
        lines.push("> Auto-detected by tunaFlow. Verify and adjust if needed.".into());
        lines.join("\n")
    };

    format!(r#"# {project_name} — Agent Instructions

> This file defines project-level rules for all agents in tunaFlow.
> All agents (Claude, Gemini, Codex, OpenCode) must follow these rules.

---

## 1. Project Overview

- Name: {project_name}
- Status: initial setup

{stack_section}

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

### Command Execution Rules
- **NEVER run commands in background** (`&`, `nohup`, `disown`) — always run synchronously and wait for the result
- If a command takes a long time, wait for it to complete and report the full output
- Do NOT say "running in background" and return early — the result will be lost
- For long-running scripts, add progress output (e.g., `print()` per step) to show activity

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

---

## 9. Session History

> 세션 이력: `docs/agentSessionHistory.md`
> Record key decisions, completed features, and context at the end of each session.
> Read this file at the start of a new session to restore context.
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

