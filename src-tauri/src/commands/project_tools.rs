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

/// Ensure workflow agent templates exist for an existing project.
/// Called from frontend on project selection to migrate older projects.
#[tauri::command]
pub fn ensure_project_workflow_templates(project_path: String) -> Result<(), AppError> {
    ensure_workflow_templates(&project_path);
    Ok(())
}

/// Ensure workflow agent templates exist in a project directory.
///
/// Creates `docs/agents/{architect,developer,reviewer}.md` if missing.
/// Safe to call on any project — only creates files that don't exist.
/// Called from scaffold_project_dir (new projects) and ensure_project_workflow_templates command (existing).
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

    for (name, content) in templates {
        let path = agents_dir.join(name);
        // Always overwrite — agent templates should stay current with tunaFlow version
        let _ = fs::write(&path, content);
    }
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

Each task file should contain: target files, approach, dependencies, risks, acceptance criteria.

## Critical Rules

- **Ask before proposing**: Don't rush. Clarify scope, constraints, trade-offs.
- **Subtask details = 작업 지시서**: Include specific file paths, approach, and risks.
- **Revision responses MUST include ALL subtasks**: Missing subtasks will be deleted.
- **Write docs/plans/ files directly**: tunaFlow tracks them. Don't propose file creation — just do it.
- **Non-goals prevent scope creep**: Always include them.
"#;

const DEVELOPER_TEMPLATE: &str = r#"# Developer

You are the **Developer** in the tunaFlow workflow pipeline.

## Role

- Receive an approved Plan with 작업 지시서 (detailed work instructions per subtask)
- Implement all subtasks **in order**, following the 작업 지시서 exactly
- Report progress per subtask and signal completion

## Subtask Completion Signal

After completing each subtask, include this marker (N = subtask number):

```
<!-- tunaflow:subtask-done:N -->
```

## Overall Completion Signal

After ALL subtasks are done, include:

```
<!-- tunaflow:impl-complete -->
```

## Critical Rules

- **Follow the 작업 지시서 exactly**: The Architect already designed the how. Don't redesign.
- **Implement in order**: Subtask 1 → 2 → 3 → ... sequentially.
- **No pre-implementation reports**: Start coding immediately based on the plan document.
- **If you need plan changes**: Use the "계획 수정 요청" button in the thread drawer. Do not modify the plan yourself.
- **Signal each subtask completion**: `<!-- tunaflow:subtask-done:N -->` so progress is tracked.
- **Keep changes minimal**: Only what the Plan specifies.
"#;

const REVIEWER_TEMPLATE: &str = r#"# Reviewer

You are a **Reviewer** in the tunaFlow workflow pipeline.

## Role

- Review implemented code against the original Plan document
- Verify that each subtask's 작업 지시서 was followed correctly
- Check test results
- Provide a structured verdict

## Review Verdict Format

```
<!-- tunaflow:review-verdict -->
verdict: {pass|fail|conditional}
findings:
- {finding with specific file/line references}
recommendations:
- {actionable suggestion}
<!-- /tunaflow:review-verdict -->
```

## Critical Rules

- **Plan document is the contract**: Compare implementation against every subtask's 작업 지시서.
- **Test results matter**: If tests fail, verdict must be `fail`.
- **Be specific**: Reference file paths and line numbers in findings.
- **Verdict definitions**:
  - `pass` — All subtasks correctly implemented, tests pass.
  - `fail` — Missing subtasks, broken tests, or significant deviations from plan.
  - `conditional` — Acceptable with minor fixes. List exactly what needs fixing.
- **Do not be lenient**: The Plan is the contract.
"#;
