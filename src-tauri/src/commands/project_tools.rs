use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::State;

use crate::errors::AppError;
use super::projects::RawqIndexing;

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
        // 사용자 가시 메시지에 다음 단계 액션을 포함. RuntimeStatusBar 에서
        // 120px 로 truncate 되므로 핵심만 (Settings → Runtime 의 풀 메시지가 SSOT).
        // 자세한 진단은 backend stderr 에 한 번만 기록하여 release 빌드에서
        // `Console.app` 으로 추적 가능하게 한다.
        let detail = rawq::resolve_diagnostics();
        eprintln!("[get_rawq_status] rawq sidecar unavailable — {}", detail);
        return Ok(RawqStatus {
            available: false, indexed: false,
            status: "unavailable".into(),
            message: "rawq sidecar 없음 — INSTALL.md 의 'rawq 인식 안 됨' 섹션 참조".into(),
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

/// Register a cancel flag for `project_path` and insert into the active set.
/// Returns `Some(flag)` on success, `None` if a build is already in progress
/// (duplicate guard). Holds a single lock for both operations so a parallel
/// `start_*`/`rebuild_*` call observes a consistent state.
fn register_indexing(
    indexing: &RawqIndexing,
    project_path: &str,
) -> Option<Arc<AtomicBool>> {
    let mut active = indexing.active.lock();
    if active.contains(project_path) {
        return None;
    }
    active.insert(project_path.to_string());

    let flag = Arc::new(AtomicBool::new(false));
    indexing.cancels.lock().insert(project_path.to_string(), flag.clone());
    Some(flag)
}

/// Start rawq index build in background thread. Emits events:
/// - `rawq:indexing` — { projectPath, message }
/// - `rawq:indexed`  — RawqStatus (success)
/// - `rawq:error`    — RawqStatus (failure)
/// - `rawq:cancelled` — { projectPath } (user dismissed before completion)
#[tauri::command]
pub fn start_rawq_index(
    project_path: String,
    app: tauri::AppHandle,
    indexing: State<RawqIndexing>,
) -> Result<(), AppError> {
    use crate::agents::rawq;
    use tauri::Emitter;

    // Duplicate guard + cancel flag registration (single lock scope).
    let Some(cancel) = register_indexing(&indexing, &project_path) else {
        eprintln!("[rawq] already indexing {}, skipping", project_path);
        return Ok(());
    };
    let active = indexing.active.clone();
    let cancels = indexing.cancels.clone();

    let _ = app.emit("rawq:indexing", serde_json::json!({
        "projectPath": &project_path,
        "message": "Building code index..."
    }));

    std::thread::spawn(move || {
        let result = match rawq::ensure_index_cancellable(&project_path, Some(cancel.clone())) {
            Ok(0) => {
                let (files, chunks) = rawq::index_status(&project_path)
                    .ok()
                    .flatten()
                    .map(|i| (Some(i.files), Some(i.chunks)))
                    .unwrap_or((None, None));
                Some(RawqStatus {
                    available: true, indexed: true,
                    status: "ready".into(), message: "already indexed".into(),
                    files, chunks,
                })
            }
            Ok(n) => Some(RawqStatus {
                available: true, indexed: true,
                status: "built".into(), message: format!("indexed {} files", n),
                files: Some(n), chunks: None,
            }),
            Err(rawq::RawqError::Cancelled) => {
                eprintln!("[start_rawq_index] cancelled for {}", project_path);
                let _ = app.emit(
                    "rawq:cancelled",
                    serde_json::json!({ "projectPath": &project_path }),
                );
                None
            }
            Err(e) => {
                eprintln!("[start_rawq_index] {}", e);
                let available = !matches!(e, rawq::RawqError::NotFound(_));
                Some(RawqStatus {
                    available, indexed: false,
                    status: if available { "error" } else { "unavailable" }.into(),
                    message: format!("{}", e),
                    files: None, chunks: None,
                })
            }
        };

        if let Some(status) = result {
            let event = if status.indexed { "rawq:indexed" } else { "rawq:error" };
            let _ = app.emit(event, &status);
        }

        // Release both guards
        active.lock().remove(&project_path);
        cancels.lock().remove(&project_path);
    });

    Ok(())
}

/// Rebuild rawq index — drops the existing index and rebuilds with the current
/// hardcoded exclude patterns. #180 hotfix 후 레거시 오염분 정리용.
/// Emits the same events as start_rawq_index (rawq:indexing / rawq:indexed / rawq:error / rawq:cancelled).
#[tauri::command]
pub fn rebuild_rawq_index(
    project_path: String,
    app: tauri::AppHandle,
    indexing: State<RawqIndexing>,
) -> Result<(), AppError> {
    use crate::agents::rawq;
    use tauri::Emitter;

    let Some(cancel) = register_indexing(&indexing, &project_path) else {
        eprintln!("[rawq] already indexing {}, skipping rebuild", project_path);
        return Ok(());
    };
    let active = indexing.active.clone();
    let cancels = indexing.cancels.clone();

    let _ = app.emit("rawq:indexing", serde_json::json!({
        "projectPath": &project_path,
        "message": "Rebuilding code index (removing legacy data)..."
    }));

    std::thread::spawn(move || {
        let result = match rawq::rebuild_index_cancellable(&project_path, Some(cancel.clone())) {
            Ok(n) => Some(RawqStatus {
                available: true, indexed: true,
                status: "built".into(), message: format!("rebuilt: indexed {} files", n),
                files: Some(n), chunks: None,
            }),
            Err(rawq::RawqError::Cancelled) => {
                eprintln!("[rebuild_rawq_index] cancelled for {}", project_path);
                let _ = app.emit(
                    "rawq:cancelled",
                    serde_json::json!({ "projectPath": &project_path }),
                );
                None
            }
            Err(e) => {
                eprintln!("[rebuild_rawq_index] {}", e);
                let available = !matches!(e, rawq::RawqError::NotFound(_));
                Some(RawqStatus {
                    available, indexed: false,
                    status: if available { "error" } else { "unavailable" }.into(),
                    message: format!("{}", e),
                    files: None, chunks: None,
                })
            }
        };

        if let Some(status) = result {
            let event = if status.indexed { "rawq:indexed" } else { "rawq:error" };
            let _ = app.emit(event, &status);
        }

        active.lock().remove(&project_path);
        cancels.lock().remove(&project_path);
    });

    Ok(())
}

/// Cancel an in-flight rawq index build for `project_path`.
///
/// Idempotent — calling on a path that is not currently indexing is a no-op.
/// Returns `true` if a cancel flag was actually set, `false` otherwise. The
/// background thread observes the flag within ~100 ms (next poll tick),
/// kills the rawq subprocess, and emits `rawq:cancelled`.
#[tauri::command]
pub fn cancel_rawq_index(
    project_path: String,
    indexing: State<RawqIndexing>,
) -> Result<bool, AppError> {
    let cancels = indexing.cancels.lock();
    if let Some(flag) = cancels.get(&project_path) {
        flag.store(true, Ordering::Relaxed);
        eprintln!("[rawq] cancel requested for {}", project_path);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Git status for a project path.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub dirty: bool,
    pub git_root: Option<String>,
    pub added: u32,
    pub modified: u32,
    pub untracked: u32,
}

#[tauri::command]
pub fn get_git_status(project_path: String) -> Result<GitStatus, AppError> {
    use std::process::Command;
    use crate::no_console::NoConsole;
    let path = std::path::Path::new(&project_path);
    if !path.exists() {
        return Ok(GitStatus { is_repo: false, branch: None, dirty: false, git_root: None, added: 0, modified: 0, untracked: 0 });
    }

    // Check if git repo
    let is_repo = Command::new("git")
        .no_console()
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&project_path)
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false);

    if !is_repo {
        return Ok(GitStatus { is_repo: false, branch: None, dirty: false, git_root: None, added: 0, modified: 0, untracked: 0 });
    }

    let branch = Command::new("git")
        .no_console()
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&project_path)
        .output()
        .ok()
        .and_then(|o| if o.status.success() {
            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
        } else { None });

    let porcelain = Command::new("git")
        .no_console()
        .args(["status", "--porcelain"])
        .current_dir(&project_path)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let dirty = !porcelain.trim().is_empty();
    let mut added = 0u32;
    let mut modified = 0u32;
    let mut untracked = 0u32;
    for line in porcelain.lines() {
        if line.len() < 2 { continue; }
        let code = &line[..2];
        match code {
            "??" => untracked += 1,
            s if s.starts_with('A') || s.ends_with('A') => added += 1,
            s if s.contains('M') || s.contains('R') || s.contains('C') => modified += 1,
            s if s.contains('D') => modified += 1, // deletions count as modifications
            _ => modified += 1,
        }
    }

    let git_root = Command::new("git")
        .no_console()
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&project_path)
        .output()
        .ok()
        .and_then(|o| if o.status.success() {
            Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
        } else { None });

    Ok(GitStatus { is_repo, branch, dirty, git_root, added, modified, untracked })
}

/// Ensure workflow agent templates exist for an existing project.
/// Called from frontend on project selection to migrate older projects.
#[tauri::command]
pub fn ensure_project_workflow_templates(project_path: String) -> Result<(), AppError> {
    ensure_workflow_templates(&project_path);
    Ok(())
}

/// Sentinel marker that delimits the user-customizable region inside a scaffold
/// template. Anything between `BEGIN user-customize` and `END user-customize`
/// is preserved across `ensure_workflow_templates` refreshes (issue #254).
pub const SENTINEL_BEGIN: &str = "<!-- BEGIN user-customize -->";
pub const SENTINEL_END: &str = "<!-- END user-customize -->";

/// Migration suffix appended when a sentinel-less file is replaced. The
/// original content is preserved at `{file}.pre-sentinel` so the user can
/// recover their customizations into the new sentinel block.
const PRE_SENTINEL_SUFFIX: &str = ".pre-sentinel";

/// Extract the body between `SENTINEL_BEGIN` and `SENTINEL_END` (exclusive).
/// Returns `None` if either marker is missing or out of order.
fn extract_user_customize(content: &str) -> Option<&str> {
    let begin = content.find(SENTINEL_BEGIN)?;
    let body_start = begin + SENTINEL_BEGIN.len();
    // The end marker must come *after* the begin marker — search the suffix.
    let end_rel = content[body_start..].find(SENTINEL_END)?;
    Some(&content[body_start..body_start + end_rel])
}

/// Replace the sentinel body of `template` with `body`. Returns `None` if
/// the template itself does not contain the sentinel pair (defensive — every
/// shipped template ships with empty sentinels).
fn inject_user_customize(template: &str, body: &str) -> Option<String> {
    let begin = template.find(SENTINEL_BEGIN)?;
    let body_start = begin + SENTINEL_BEGIN.len();
    let end_rel = template[body_start..].find(SENTINEL_END)?;
    let end_abs = body_start + end_rel;
    let mut out = String::with_capacity(template.len() + body.len());
    out.push_str(&template[..body_start]);
    out.push_str(body);
    out.push_str(&template[end_abs..]);
    Some(out)
}

/// Refresh a single agent doc applying the sentinel preservation policy.
///
/// Returns silently on any I/O failure — scaffold is best-effort and we never
/// want a transient FS error to block project open. The single hard rule is:
/// **never overwrite a sentinel-less user file unless the `.pre-sentinel`
/// backup write succeeded first**.
fn refresh_agent_doc_with_sentinel(
    path: &std::path::Path,
    template: &str,
    file_label: &str,
) {
    use std::fs;

    // Case 1 — file does not exist. First-time scaffold; nothing to preserve.
    if !path.exists() {
        if let Err(e) = fs::write(path, template) {
            eprintln!("[scaffold] failed to create {}: {}", path.display(), e);
        } else {
            eprintln!("[scaffold] created {}", path.display());
        }
        return;
    }

    // Case 2/3 — file exists. Decide based on sentinel presence.
    let existing = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[scaffold] failed to read {} (refresh skipped): {}", path.display(), e);
            return;
        }
    };

    if let Some(body) = extract_user_customize(&existing) {
        // Case 2 — sentinel present. Preserve body, refresh shell.
        match inject_user_customize(template, body) {
            Some(new_content) => {
                if new_content == existing {
                    // Nothing to update — quiet no-op.
                    return;
                }
                if let Err(e) = fs::write(path, &new_content) {
                    eprintln!(
                        "[scaffold] failed to refresh {} (write error: {}). User content untouched.",
                        path.display(), e
                    );
                } else {
                    eprintln!(
                        "[scaffold] preserved user-customize section in {}",
                        file_label
                    );
                }
            }
            None => {
                // Defensive: shipped template should always have sentinels.
                // If it doesn't, do nothing rather than risk losing user data.
                eprintln!(
                    "[scaffold] template for {} missing sentinel markers — refresh skipped",
                    file_label
                );
            }
        }
        return;
    }

    // Case 3 — pre-sentinel legacy file. Back up first, then write template.
    let backup_path = {
        let mut s = path.as_os_str().to_owned();
        s.push(PRE_SENTINEL_SUFFIX);
        std::path::PathBuf::from(s)
    };

    // If a backup already exists from a prior migration attempt, do not
    // clobber it — keep the oldest copy of the user's customization.
    if !backup_path.exists() {
        if let Err(e) = fs::write(&backup_path, &existing) {
            eprintln!(
                "[scaffold] ABORT refresh of {}: backup write to {} failed: {}. \
                 User customization preserved on disk; tunaFlow will not overwrite.",
                file_label, backup_path.display(), e
            );
            return;
        }
    }

    if let Err(e) = fs::write(path, template) {
        eprintln!(
            "[scaffold] migration write failed for {} after backup at {}: {}",
            file_label, backup_path.display(), e
        );
        return;
    }

    eprintln!(
        "[scaffold] migrated {} → {} + new template",
        file_label,
        backup_path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| backup_path.display().to_string())
    );
}

/// Ensure workflow agent templates exist in a project directory.
///
/// Refresh policy (issue #254):
///   1. Missing file → write the latest template.
///   2. File with `BEGIN user-customize` / `END user-customize` sentinel →
///      preserve the body between markers, refresh everything else.
///   3. Pre-sentinel file (legacy) → back up to `*.md.pre-sentinel` first,
///      then write the new template. **If the backup write fails, abort
///      this file's refresh entirely** (never overwrite user customization
///      without a recoverable backup).
///
/// Safe to call on any project. Called from scaffold_project_dir (new
/// projects) and ensure_project_workflow_templates command (existing).
pub fn ensure_workflow_templates(project_path: &str) {
    use std::fs;
    use std::path::Path;

    let root = Path::new(project_path);
    if !root.is_dir() { return; }

    let agents_dir = root.join("docs/agents");
    let _ = fs::create_dir_all(&agents_dir);

    let templates: &[(&str, &str)] = &[
        ("architect.md", ARCHITECT_TEMPLATE),
        ("developer.md", DEVELOPER_TEMPLATE),
        ("reviewer.md", REVIEWER_TEMPLATE),
    ];

    for (name, template) in templates {
        let path = agents_dir.join(name);
        refresh_agent_doc_with_sentinel(&path, template, name);
    }

    // Ensure .claude/settings.local.json with default permissions
    let claude_dir = root.join(".claude");
    let _ = fs::create_dir_all(&claude_dir);
    let settings_path = claude_dir.join("settings.local.json");
    if !settings_path.exists() {
        let default_settings = r#"{
  "permissions": {
    "allow": [
      "Bash(npm install*)",
      "Bash(npm run*)",
      "Bash(npm ci*)",
      "Bash(npx *)",
      "Bash(cargo build*)",
      "Bash(cargo test*)",
      "Bash(cargo check*)",
      "Bash(cargo run*)",
      "Bash(git *)",
      "Bash(mkdir *)",
      "Bash(ls *)",
      "Bash(cat *)",
      "Bash(node *)"
    ]
  }
}"#;
        let _ = fs::write(&settings_path, default_settings);
    }
}

/// Read the project's .claude/settings.local.json permissions
#[tauri::command]
pub fn get_project_cli_permissions(project_path: String) -> Result<Vec<String>, AppError> {
    let path = std::path::Path::new(&project_path).join(".claude/settings.local.json");
    if !path.exists() { return Ok(vec![]); }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| AppError::Agent(format!("read: {}", e)))?;
    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::Agent(format!("parse: {}", e)))?;
    let allow = json.get("permissions")
        .and_then(|p| p.get("allow"))
        .and_then(|a| a.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    Ok(allow)
}

/// Update the project's .claude/settings.local.json permissions
#[tauri::command]
pub fn set_project_cli_permissions(project_path: String, permissions: Vec<String>) -> Result<(), AppError> {
    let claude_dir = std::path::Path::new(&project_path).join(".claude");
    let _ = std::fs::create_dir_all(&claude_dir);
    let path = claude_dir.join("settings.local.json");

    // Read existing or create new
    let mut json: serde_json::Value = if path.exists() {
        let content = std::fs::read_to_string(&path).unwrap_or_else(|_| "{}".into());
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // Update permissions.allow
    json["permissions"]["allow"] = serde_json::json!(permissions);

    let content = serde_json::to_string_pretty(&json)
        .map_err(|e| AppError::Agent(format!("serialize: {}", e)))?;
    std::fs::write(&path, content)
        .map_err(|e| AppError::Agent(format!("write: {}", e)))?;
    Ok(())
}

const ARCHITECT_TEMPLATE: &str = r#"# Architect

You are the **Architect** in the tunaFlow workflow pipeline.

## Role

- Design plans: **what** to do (Plan) and **how** to do it (작업 지시서)
- Iterate with the user through Q&A before proposing
- Modify plans when revision requests include review opinions

## Workflow Stages

1. **Chat**: Discuss requirements → propose plan (plan-proposal marker)
2. **Plan (drafting)**: Plan promoted → write docs/plans/ files (main plan + per-subtask task docs)
3. **Subtask (review)**: User reviews 작업 지시서 → may request revisions via slider chat

## Plan Proposal Format (Chat stage)

```
<!-- tunaflow:plan-proposal -->
## Plan Proposal: {title}

### Description
{what and why}

### Expected Outcome
{success criteria}

### Subtasks
1. {task title} — {detailed work instruction: files to modify, approach, risks}
2. {task title} — {detailed work instruction}

### Constraints
- {constraint}

### Non-goals
- {explicitly excluded}
<!-- /tunaflow:plan-proposal -->
```

## Document Writing (after promotion)

After the plan is promoted, write documents directly in `docs/plans/`:

- `{slug}.md` — Main plan document (description, outcome, subtask summary, version)
- `{slug}-task-01.md` — Subtask 1 work instruction (detailed how)
- `{slug}-task-02.md` — Subtask 2 work instruction
- Continue for each subtask

**`{slug}` is not something you invent.** The ContextPack `## Active Plan` section
includes `> **Plan slug (canonical):** `<value>`` — use that value verbatim.
Do not abbreviate, truncate, or re-slugify the plan title yourself. tunaFlow
(Reviewer context loader, result/review report writers) reads file names back
using this exact slug; any deviation — including a stray trailing `-` — means
your task files will be invisible to downstream agents.

Each task file MUST contain:
1. **Changed files** — exact paths verified against the codebase (new files: state explicitly)
2. **Change description** — what to add/modify/remove and why
3. **Dependencies** — which tasks must complete first (depends_on)
4. **Verification** — one or more **executable shell commands** that prove the task is done. Examples:
   - `npx tsc --noEmit` (type check)
   - `npx vitest run src/tests/foo.test.ts` (specific test)
   - `curl -s http://localhost:3000/api/health | jq .status` (API check)
   - Do NOT write vague criteria like "works" or "compiles"
5. **Risks** — potential side effects (use graph data if available)

When subtasks can run independently, assign the same `parallel_group` and specify `depends_on` for ordering.

### Result Report — DO NOT include as a subtask

**Do NOT** add a final subtask such as "write result.md", "결과 문서 작성", "summarize results into docs/plans/{slug}-result.md", or any equivalent. The result report (`docs/plans/{slug}-result.md`) is **auto-generated by tunaFlow** after `<!-- tunaflow:impl-complete -->` is signaled. The Reviewer must never read it. Including a result-writing task corrupts the report format and triggers repeated review failures.

This applies to **plan proposals** (chat stage) and **task files** (drafting stage) equally. Do not include result.md / 결과 문서 / summary report writing as a subtask under any name.

## Tool Requests

When you need to explore the codebase before designing:
- `<!-- tunaflow:tool-request:docs:QUERY -->` — Search library/framework documentation
- `<!-- tunaflow:tool-request:rawq:QUERY -->` — Search project codebase
- `<!-- tunaflow:tool-request:graph:PATTERN TARGET -->` — Query code graph (callers_of, tests_for, etc.)

Tiered message inspection (when `recent_turns` truncated a message you need to verify):
- `<!-- tunaflow:tool-request:probe_message:MESSAGE_ID -->` — ~1 KB metadata probe (length + head/tail previews). Confirms DB has full content before you pay for the body.
- `<!-- tunaflow:tool-request:fetch_slice:MESSAGE_ID:OFFSET:LEN -->` — Read a `[offset, offset+len)` char slice. LEN capped at 16 000.
- `<!-- tunaflow:tool-request:full_message:MESSAGE_ID -->` — Entire content with no truncation. Heaviest — prefer probe/slice unless you really need the whole thing.

tunaFlow will execute the request and provide results in the next turn.
Include markers at the END of your response, after your main content.

## Critical Rules

- **NEVER write code or implement features**: You are the Architect, not the Developer. You design plans and write 작업 지시서 documents only. If asked to discuss a subtask, discuss the design — do not create source code files.
- **NEVER add a result-report subtask**: `docs/plans/{slug}-result.md` is auto-generated by tunaFlow. Do not include "write result.md", "결과 문서 작성", or any equivalent as a subtask in plan proposals or task files.
- **Do NOT guess file paths**: Verify they exist using tool-request:rawq before including them.
- **Ask before proposing**: Don't rush. Clarify scope, constraints, trade-offs.
- **Subtask details = 작업 지시서**: Include specific file paths, approach, and risks.
- **Revision responses MUST include ALL subtasks**: Missing subtasks will be deleted.
- **Write docs/plans/ files directly**: tunaFlow tracks them. Don't propose file creation — just do it.
- **Non-goals prevent scope creep**: Always include them.
- **Discussion = discussion only**: When a user opens a subtask discussion, respond with analysis, questions, suggestions — not implementation.
- **Do NOT guess past work**: If the user asks about a past plan, completed task, or historical context that is not in your current context, use tool-request markers FIRST (`tool-request:plans`, `tool-request:memory`, `tool-request:rawq`) to retrieve the information. Never present uncertain information as fact. Say "I'll look that up" and emit the marker — do NOT answer and then verify after.

## Custom Rules

<!-- BEGIN user-customize -->
<!-- This section is preserved across tunaFlow scaffold refreshes. Add your
     project-specific Architect rules here. tunaFlow will never overwrite
     content between the BEGIN/END user-customize markers. -->

<!-- END user-customize -->
"#;

const DEVELOPER_TEMPLATE: &str = r#"# Developer

You are the **Developer** in the tunaFlow workflow pipeline.

## Role

- Receive an approved Plan with 작업 지시서 (detailed work instructions per subtask)
- Implement all subtasks **in order**, following the 작업 지시서 exactly
- Handle rework when review findings are provided

## Implementation Procedure

For each subtask:
1. Read the task file (`docs/plans/{slug}-task-NN.md`)
2. Implement changes to the files listed in **Changed files** only
3. Run every command in the **Verification** section and report results
4. Signal completion with `<!-- tunaflow:subtask-done:N -->`

After ALL subtasks:
5. Signal `<!-- tunaflow:impl-complete -->`

**IMPORTANT**: These markers are for the chat message ONLY. Do NOT write them into files.

## Verification — MANDATORY

Before signaling subtask-done or impl-complete, run each Verification command from the task file and report:

```
Verification results for Task N:
✅ `npx tsc --noEmit` — exit 0
✅ `npx vitest run src/tests/foo.test.ts` — 3 passed
❌ `curl ...` — connection refused (server not running, expected in dev)
```

- Run **only** the commands listed in the task's Verification section
- Do NOT run the full project test suite unless the task says to
- If a command fails for an expected reason (e.g. no server in dev), explain why
- Do NOT claim a verification passed if you did not actually run it

## Result Report — DO NOT WRITE

tunaFlow **automatically generates** the result report (`docs/plans/{slug}-result.md`).

**You must NOT**:
- Create or modify `*-result.md` files
- Include `<!-- tunaflow:impl-complete -->` markers in any file
- Write verification results into files

## Tool Requests

When you need information during implementation:
- `<!-- tunaflow:tool-request:docs:QUERY -->` — Search library/framework documentation
- `<!-- tunaflow:tool-request:rawq:QUERY -->` — Search project codebase
- `<!-- tunaflow:tool-request:graph:callers_of TARGET -->` — Find what calls a function

Tiered message inspection (when a message appeared cut in `recent_turns`):
- `<!-- tunaflow:tool-request:probe_message:MESSAGE_ID -->` — metadata + head/tail (~1 KB)
- `<!-- tunaflow:tool-request:fetch_slice:MESSAGE_ID:OFFSET:LEN -->` — slice (LEN ≤ 16 000)
- `<!-- tunaflow:tool-request:full_message:MESSAGE_ID -->` — full content (heavy)

Include markers at the END of your response, after your main content.

## Rework

When you receive a rework request with review findings:
1. Read each finding carefully — **only fix the specified subtasks**
2. If "대상 서브태스크" is specified, do NOT modify other tasks' code
3. Check "이전 시도 이력" to avoid repeating past mistakes
4. Re-run Verification commands and report results
5. Signal completion with `<!-- tunaflow:impl-complete -->`

## Critical Rules

- **Follow the 작업 지시서 exactly**: The Architect already designed the how. Don't redesign.
- **Changed files only**: Do NOT modify files outside the task's 'Changed files' list.
- **Verification is not optional**: Every task has Verification commands — run them and report.
- **Markers in chat only**: Never write tunaflow markers into files.
- **If the plan needs changes, say so**: Don't silently deviate.

## Custom Rules

<!-- BEGIN user-customize -->
<!-- This section is preserved across tunaFlow scaffold refreshes. Add your
     project-specific Developer rules here. tunaFlow will never overwrite
     content between the BEGIN/END user-customize markers. -->

<!-- END user-customize -->
"#;

const REVIEWER_TEMPLATE: &str = r#"# Reviewer

You are a **Reviewer** in the tunaFlow workflow pipeline.

## Role

- Review implemented code **by reading code only** — do NOT run build, test, or shell commands
- The Developer already ran Verification commands and reported results
- Provide a structured verdict based on a 3-point checklist

## Review Procedure

For each subtask, read the task file (`docs/plans/{slug}-task-NN.md`) and check:

1. **Changed files**: Are the files listed in 'Changed files' actually modified? Do changes match 'Change description'?
2. **Verification results**: Did the Developer report Verification results? Did they pass?
3. **Code defects**: Does the changed code contain runtime errors, logic bugs, or security vulnerabilities?

**Pass** if all three checks are satisfied for every subtask.

## Review Verdict Format (MANDATORY)

Your response MUST end with this exact verdict block. Do NOT put it inside a code fence.

<!-- tunaflow:review-verdict -->
verdict: {pass|fail|conditional}
failed_subtask_ids: [N, M]
findings:
- {file:line — concrete defect description}
recommendations:
- {actionable improvement suggestion}
<!-- /tunaflow:review-verdict -->

**failed_subtask_ids**: fail 또는 conditional인 경우, 문제가 있는 서브태스크 번호(1-based)를 반드시 포함.

## What is NOT a fail reason

- Code style or structure preferences (different approach but correct result)
- Missing tests not required by the task's Verification section
- Pre-existing issues in files the Developer did not modify
- "A better approach exists" opinions → put in recommendations, not findings
- Result report quality, content, structure, OR existence — it is auto-generated
  by tunaFlow, not the Developer's work. Do not read or judge `*-result.md`.

## Re-review Rules

When reviewing after rework:
- Focus on whether previous findings were fixed
- Verify the same issues don't persist
- New findings only for concrete defects within the Plan scope
- Do NOT re-run or second-guess Verification results the Developer reported as passing

## Critical Rules

- **Read code only**: Do NOT run any shell commands, builds, or tests.
- **Task file is the contract**: Compare implementation against each task's Changed files and Verification.
- **Be specific**: Every finding MUST include file path, line number, and concrete defect description.
- **Result report is auto-generated**: Never judge `*-result.md` quality.
- **Do NOT read `*-result.md` from disk**: Even with sed/cat/nl/read tools,
  accessing the result report file is the same policy violation as judging
  it. The result report is auto-generated and not part of the review contract.
- **Findings vs Recommendations**: Only actual defects go in findings. Everything else goes in recommendations.

## Custom Rules

<!-- BEGIN user-customize -->
<!-- This section is preserved across tunaFlow scaffold refreshes. Add your
     project-specific Reviewer rules here. tunaFlow will never overwrite
     content between the BEGIN/END user-customize markers. -->

<!-- END user-customize -->
"#;


// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Guard against silent drift — scaffold templates ship features that
    // live git files depend on. If these lines are removed from the const,
    // every project selection overwrites the tracked doc with a stale copy.

    #[test]
    fn architect_template_includes_slug_canonical_rule() {
        assert!(ARCHITECT_TEMPLATE.contains("Plan slug (canonical)"),
            "architect template must preserve the canonical-slug rule");
        assert!(ARCHITECT_TEMPLATE.contains("is not something you invent"),
            "architect template must warn against re-slugifying");
    }

    #[test]
    fn architect_template_includes_tiered_message_tools() {
        assert!(ARCHITECT_TEMPLATE.contains("probe_message:MESSAGE_ID"),
            "architect template must expose probe_message tool-request");
        assert!(ARCHITECT_TEMPLATE.contains("fetch_slice:MESSAGE_ID"),
            "architect template must expose fetch_slice tool-request");
        assert!(ARCHITECT_TEMPLATE.contains("full_message:MESSAGE_ID"),
            "architect template must expose full_message tool-request");
    }

    #[test]
    fn developer_template_includes_tiered_message_tools() {
        assert!(DEVELOPER_TEMPLATE.contains("probe_message:MESSAGE_ID"),
            "developer template must expose probe_message tool-request");
        assert!(DEVELOPER_TEMPLATE.contains("fetch_slice:MESSAGE_ID"),
            "developer template must expose fetch_slice tool-request");
        assert!(DEVELOPER_TEMPLATE.contains("full_message:MESSAGE_ID"),
            "developer template must expose full_message tool-request");
    }

    #[test]
    fn reviewer_template_not_empty() {
        assert!(!REVIEWER_TEMPLATE.is_empty());
        assert!(REVIEWER_TEMPLATE.contains("Reviewer"));
    }

    /// Issue #254 — block Architect from injecting a result.md writing task.
    /// Both the dedicated section and the Critical Rules line must stay present.
    #[test]
    fn architect_template_blocks_result_task_inject() {
        assert!(
            ARCHITECT_TEMPLATE.contains("Result Report — DO NOT include as a subtask"),
            "architect template must declare the dedicated 'do not include result.md' section (issue #254)"
        );
        assert!(
            ARCHITECT_TEMPLATE.contains("NEVER add a result-report subtask"),
            "architect template Critical Rules must explicitly forbid result-report subtasks (issue #254)"
        );
        assert!(
            ARCHITECT_TEMPLATE.contains("auto-generated by tunaFlow"),
            "architect template must explain that result.md is auto-generated"
        );
    }

    /// Issue #254 — every shipped agent template must carry the sentinel
    /// markers so that `ensure_workflow_templates` can preserve user edits.
    #[test]
    fn all_agent_templates_carry_user_customize_sentinel() {
        for (label, tmpl) in [
            ("ARCHITECT_TEMPLATE", ARCHITECT_TEMPLATE),
            ("DEVELOPER_TEMPLATE", DEVELOPER_TEMPLATE),
            ("REVIEWER_TEMPLATE", REVIEWER_TEMPLATE),
        ] {
            assert!(
                tmpl.contains(SENTINEL_BEGIN),
                "{} missing BEGIN user-customize marker", label
            );
            assert!(
                tmpl.contains(SENTINEL_END),
                "{} missing END user-customize marker", label
            );
            // BEGIN must precede END.
            let begin = tmpl.find(SENTINEL_BEGIN).unwrap();
            let end = tmpl.find(SENTINEL_END).unwrap();
            assert!(begin < end, "{} has END before BEGIN", label);
        }
    }

    #[test]
    fn extract_user_customize_returns_body_between_markers() {
        let doc = format!(
            "header\n\n## Custom\n\n{}\nuser line A\nuser line B\n{}\nfooter\n",
            SENTINEL_BEGIN, SENTINEL_END
        );
        let body = extract_user_customize(&doc).expect("sentinel pair must parse");
        assert!(body.contains("user line A"));
        assert!(body.contains("user line B"));
        assert!(!body.contains("header"));
        assert!(!body.contains("footer"));
    }

    #[test]
    fn extract_user_customize_returns_none_without_markers() {
        assert!(extract_user_customize("plain doc, no markers").is_none());
        let only_begin = format!("foo {} bar", SENTINEL_BEGIN);
        assert!(extract_user_customize(&only_begin).is_none());
    }

    #[test]
    fn inject_user_customize_round_trips() {
        let template = format!(
            "shell head\n{}\nDEFAULT EMPTY\n{}\nshell tail\n",
            SENTINEL_BEGIN, SENTINEL_END
        );
        let preserved = "\nuser-line-1\nuser-line-2\n";
        let merged = inject_user_customize(&template, preserved).expect("inject must succeed");
        assert!(merged.contains("shell head"));
        assert!(merged.contains("shell tail"));
        assert!(merged.contains("user-line-1"));
        assert!(merged.contains("user-line-2"));
        assert!(!merged.contains("DEFAULT EMPTY"));
    }

    #[test]
    fn refresh_preserves_sentinel_body() {
        let dir = tempfile::tempdir().unwrap();
        let agents = dir.path().join("docs/agents");
        std::fs::create_dir_all(&agents).unwrap();
        let path = agents.join("architect.md");

        // Seed the file with the shipped template + a user customization.
        let body = "\nMY PROJECT RULE: never add a result task.\n";
        let seeded = inject_user_customize(ARCHITECT_TEMPLATE, body).unwrap();
        std::fs::write(&path, &seeded).unwrap();

        ensure_workflow_templates(dir.path().to_str().unwrap());

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(
            after.contains("MY PROJECT RULE: never add a result task."),
            "sentinel body must survive a scaffold refresh"
        );
        assert!(
            !path.with_extension("md.pre-sentinel").exists() &&
            !agents.join("architect.md.pre-sentinel").exists(),
            "no migration backup should be created when sentinels are present"
        );
    }

    #[test]
    fn refresh_migrates_pre_sentinel_legacy_file_with_backup() {
        let dir = tempfile::tempdir().unwrap();
        let agents = dir.path().join("docs/agents");
        std::fs::create_dir_all(&agents).unwrap();
        let path = agents.join("architect.md");

        // Legacy file without sentinel markers.
        let legacy = "# Architect (legacy)\n\nUser rules without sentinel.\n";
        std::fs::write(&path, legacy).unwrap();

        ensure_workflow_templates(dir.path().to_str().unwrap());

        let backup = agents.join("architect.md.pre-sentinel");
        assert!(backup.exists(), "legacy file must be backed up before overwrite");
        let backup_contents = std::fs::read_to_string(&backup).unwrap();
        assert_eq!(backup_contents, legacy, "backup must be exact copy of legacy file");

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(
            after.contains(SENTINEL_BEGIN) && after.contains(SENTINEL_END),
            "new file must contain sentinel markers"
        );
        assert!(
            after.contains("# Architect"),
            "new file must be the shipped template"
        );
    }

    #[test]
    fn refresh_creates_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        ensure_workflow_templates(dir.path().to_str().unwrap());
        let path = dir.path().join("docs/agents/architect.md");
        assert!(path.exists(), "missing agent doc must be created");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains(SENTINEL_BEGIN));
    }

    #[test]
    fn refresh_does_not_overwrite_existing_backup() {
        let dir = tempfile::tempdir().unwrap();
        let agents = dir.path().join("docs/agents");
        std::fs::create_dir_all(&agents).unwrap();
        let path = agents.join("architect.md");
        let backup = agents.join("architect.md.pre-sentinel");

        std::fs::write(&path, "legacy v2\n").unwrap();
        std::fs::write(&backup, "legacy v1 (older, must be kept)\n").unwrap();

        ensure_workflow_templates(dir.path().to_str().unwrap());

        let backup_after = std::fs::read_to_string(&backup).unwrap();
        assert_eq!(
            backup_after, "legacy v1 (older, must be kept)\n",
            "existing backup must not be clobbered"
        );
    }
}
