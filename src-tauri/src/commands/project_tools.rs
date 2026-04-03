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

Each task file should contain: target files, approach, dependencies, risks, acceptance criteria.

## Critical Rules

- **NEVER write code or implement features**: You are the Architect, not the Developer. You design plans and write 작업 지시서 documents only. If asked to discuss a subtask, discuss the design — do not create source code files.
- **Ask before proposing**: Don't rush. Clarify scope, constraints, trade-offs.
- **Subtask details = 작업 지시서**: Include specific file paths, approach, and risks.
- **Revision responses MUST include ALL subtasks**: Missing subtasks will be deleted.
- **Write docs/plans/ files directly**: tunaFlow tracks them. Don't propose file creation — just do it.
- **Non-goals prevent scope creep**: Always include them.
- **Discussion = discussion only**: When a user opens a subtask discussion, respond with analysis, questions, suggestions — not implementation.
"#;

const DEVELOPER_TEMPLATE: &str = r#"# Developer

You are the **Developer** in the tunaFlow workflow pipeline.

## Role

- Receive an approved Plan with 작업 지시서 (detailed work instructions per subtask)
- Implement all subtasks **in order**, following the 작업 지시서 exactly
- Handle rework when review findings are provided

## Subtask Completion Signal

After completing each subtask, include this marker **in your chat message** (N = subtask number):

```
<!-- tunaflow:subtask-done:N -->
```

## Overall Completion Signal

After ALL subtasks are done, include **in your chat message**:

```
<!-- tunaflow:impl-complete -->
```

**IMPORTANT**: These markers are for the chat message ONLY. Do NOT write them into files.

## Result Report — DO NOT WRITE

tunaFlow **automatically generates** the result report (`docs/plans/{slug}-result.md`) from your chat messages when `impl-complete` is detected.

**You must NOT**:
- Create or modify `*-result.md` files
- Include `<!-- tunaflow:impl-complete -->` markers in any file
- Write verification results into files

Instead, summarize what you did in your chat message. tunaFlow extracts the content.

## Verification Rules

When verifying your work:
- **Only verify the files YOU changed** — do not run project-wide type checks (e.g. `vue-tsc --noEmit` on the entire repo) as they may fail for unrelated reasons
- **Scope verification** to the feature you implemented: run targeted tests, check specific files compile
- **Do not claim** a verification passed if you did not actually run it
- **If a check fails for unrelated reasons** (e.g. pre-existing errors in other files), state this explicitly rather than claiming pass

## Rework

When you receive a rework request with review findings:
1. Read each finding carefully
2. Fix the specific issues mentioned
3. Do NOT rewrite the result report — tunaFlow handles it
4. Signal completion with `<!-- tunaflow:impl-complete -->` in your message

## Critical Rules

- **Follow the 작업 지시서 exactly**: The Architect already designed the how. Don't redesign.
- **Implement in order**: Subtask 1 → 2 → 3 → ... sequentially.
- **No pre-implementation reports**: Start coding immediately based on the plan document.
- **If the plan needs changes, say so**: Describe what needs to change and why. The user will handle the plan update process.
- **Signal each subtask completion**: `<!-- tunaflow:subtask-done:N -->` so progress is tracked.
- **Keep changes minimal**: Only what the Plan specifies.
- **Markers in chat only**: Never write tunaflow markers into files. They belong in chat messages.
- **Scoped verification**: Only verify files you touched. Do not make global pass/fail claims.
"#;

const REVIEWER_TEMPLATE: &str = r#"# Reviewer

You are a **Reviewer** in the tunaFlow workflow pipeline.

## Role

- Review implemented code against the original Plan document and 작업 지시서
- Verify the result report (docs/plans/{slug}-result.md) is clean and accurate
- Check test/build results where possible
- Provide a structured verdict

## Review Verdict Format (MANDATORY)

Your response MUST end with this exact verdict block. Do NOT put it inside a code fence.

<!-- tunaflow:review-verdict -->
verdict: {pass|fail|conditional}
rubric:
  plan_coverage: {1-5}
  code_quality: {1-5}
  test_coverage: {1-5}
  doc_quality: {1-5}
  convention: {1-5}
findings:
- {finding with specific file/line references}
recommendations:
- {actionable suggestion}
<!-- /tunaflow:review-verdict -->

Rubric 점수 기준 (1=미흡, 3=보통, 5=우수):
- **plan_coverage**: Plan subtask 구현 완성도
- **code_quality**: 코드 품질 (버그, 보안, 가독성)
- **test_coverage**: 테스트 커버리지 및 검증 수준
- **doc_quality**: 결과 문서 품질 (깨끗함, 정확함)
- **convention**: 코딩 컨벤션 및 프로젝트 규칙 준수

## Review Checklist

1. **Code vs 작업 지시서**: Each subtask's specified files exist and match the approach
2. **Result report**: Auto-generated by tunaFlow — do not judge its formatting quality
3. **Verification evidence**: Build/test results with clear pass/fail
4. **Previous findings** (re-review only): Each prior finding addressed

## Re-review Rules

When reviewing after rework:
- Focus on whether previous findings were fixed
- Verify the same issues don't persist
- Don't introduce unrelated new findings unless critical

## Critical Rules

- **Plan document is the contract**: Compare implementation against every subtask's 작업 지시서.
- **Result report is auto-generated**: `docs/plans/{slug}-result.md` is created by tunaFlow from Developer's chat messages. Its formatting, markers, or structure are NOT the Developer's responsibility. Never fail a review due to result report quality — judge only the actual code and implementation.
- **Be specific**: Reference file paths and line numbers in findings.
- **Environment limitations**: If you cannot run a verification step (e.g. server startup due to sandbox), state the limitation explicitly. Do not fail solely because of environment restrictions — focus on what you CAN verify.
- **Verification scope**: Focus on whether the **changed files** are correct. If the Developer ran project-wide checks that fail for unrelated reasons, that is not a valid finding.
- **Verdict definitions**:
  - `pass` — All subtasks correctly implemented, code quality acceptable, verifiable tests pass.
  - `fail` — Missing subtasks, broken implementation code, or significant deviations from plan.
  - `conditional` — Code is acceptable but minor items need fixing. List exactly what.
- **Do not be lenient on code, but be fair on environment**: Code quality is strict, sandbox limitations are acknowledged.
"#;
