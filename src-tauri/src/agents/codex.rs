use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;

use crate::agents::claude::{RunInput, RunOutput};
use crate::errors::AppError;

/// Resolve the codex binary/script path.
///
/// On Windows, prefer the node script directly (avoids `.cmd` wrapper issues).
/// Returns `(command, script_arg)`:
///   - Windows with node_modules: ("node", Some("path/to/codex.js"))
///   - Otherwise: ("codex" or resolved path, None)
fn resolve_codex() -> (String, Option<String>) {
    #[cfg(target_os = "windows")]
    {
        // Prefer direct node invocation (tunadish pattern)
        if let Ok(appdata) = std::env::var("APPDATA") {
            let entry = PathBuf::from(&appdata)
                .join("npm")
                .join("node_modules")
                .join("@openai")
                .join("codex")
                .join("bin")
                .join("codex.js");
            if entry.exists() {
                let node = which_or("node", "node");
                return (node, Some(entry.to_string_lossy().to_string()));
            }
        }
        // Fallback to .cmd via cmd /C
        if let Ok(appdata) = std::env::var("APPDATA") {
            let candidate = PathBuf::from(&appdata).join("npm").join("codex.cmd");
            if candidate.exists() {
                return (candidate.to_string_lossy().to_string(), None);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // fnm/nvm: check node version manager paths first
        if let Ok(home) = std::env::var("HOME") {
            let fnm_base = PathBuf::from(&home).join(".local/share/fnm/node-versions");
            if fnm_base.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&fnm_base) {
                    let mut versions: Vec<PathBuf> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.path().join("installation/bin/codex"))
                        .filter(|p| p.exists())
                        .collect();
                    versions.sort();
                    if let Some(candidate) = versions.last() {
                        return (candidate.to_string_lossy().to_string(), None);
                    }
                }
            }
            let nvm_base = PathBuf::from(&home).join(".nvm/versions/node");
            if nvm_base.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&nvm_base) {
                    let mut versions: Vec<PathBuf> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.path().join("bin/codex"))
                        .filter(|p| p.exists())
                        .collect();
                    versions.sort();
                    if let Some(candidate) = versions.last() {
                        return (candidate.to_string_lossy().to_string(), None);
                    }
                }
            }
        }
        // Standard paths
        for prefix in &["/usr/local/bin", "/usr/bin", "/opt/homebrew/bin"] {
            let candidate = PathBuf::from(prefix).join("codex");
            if candidate.exists() {
                return (candidate.to_string_lossy().to_string(), None);
            }
        }
    }

    ("codex".to_string(), None)
}

#[cfg(target_os = "windows")]
fn which_or(name: &str, fallback: &str) -> String {
    std::env::var("PATH")
        .ok()
        .and_then(|path| {
            path.split(';').find_map(|dir| {
                let candidate = PathBuf::from(dir).join(format!("{}.exe", name));
                if candidate.exists() {
                    Some(candidate.to_string_lossy().to_string())
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| fallback.to_string())
}

use super::claude::resolve_cwd;

/// Execute `codex exec` as a one-shot non-interactive subprocess.
///
/// Prompt is delivered via **stdin** (tunadish pattern: `exec --json ... -`).
/// Cost and token fields are unavailable; returned as 0.
pub fn run(input: RunInput) -> Result<RunOutput, AppError> {
    let (codex_cmd, codex_script) = resolve_codex();

    let mut cmd = Command::new(&codex_cmd);
    if let Some(ref script) = codex_script {
        cmd.arg(script);
    }

    cmd.arg("exec")
        .arg("--json")
        .arg("--skip-git-repo-check")
        .arg("--color=never");

    if let Some(model) = &input.model {
        cmd.arg("--model").arg(model);
    }

    // `-` = read prompt from stdin (tunadish pattern)
    cmd.arg("-");

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(resolve_cwd(input.project_path.as_deref()));

    let mut child = cmd.spawn().map_err(|e| {
        AppError::Agent(format!("Failed to spawn codex ({}): {}", codex_cmd, e))
    })?;

    // Write prompt to stdin, then close
    if let Some(mut stdin) = child.stdin.take() {
        let prompt_bytes = input.prompt.as_bytes().to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&prompt_bytes);
        });
    }

    // Drain stderr in background
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Agent("Failed to capture codex stderr".into()))?;
    let stderr_handle = thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr_pipe.read_to_string(&mut buf);
        buf
    });

    // Read stdout
    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Agent("Failed to capture codex stdout".into()))?;
    let mut stdout_raw = String::new();
    stdout_pipe
        .read_to_string(&mut stdout_raw)
        .map_err(|e| AppError::Agent(format!("Failed to read codex stdout: {}", e)))?;

    let exit_status = child.wait()?;
    let stderr_content = stderr_handle.join().unwrap_or_default();

    if !exit_status.success() {
        let detail = if !stderr_content.trim().is_empty() {
            stderr_content.trim().to_string()
        } else if !stdout_raw.trim().is_empty() {
            stdout_raw.trim().to_string()
        } else {
            format!("exit code {:?}", exit_status.code())
        };
        return Err(AppError::Agent(format!("codex failed: {}", detail)));
    }

    // Parse JSONL event stream from `codex exec --json`.
    // Events are newline-delimited JSON objects. We extract:
    //   - item.completed + item.type="agent_message" → item.text (user-facing content)
    //   - turn.completed → usage.input_tokens / output_tokens / total_cost (optional)
    // All other events (thread.started, turn.started, etc.) are ignored.
    let mut agent_texts: Vec<String> = Vec::new();
    let mut input_tokens: i64 = 0;
    let mut output_tokens: i64 = 0;
    let mut total_cost: f64 = 0.0;

    for line in stdout_raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Try to parse as JSON; skip non-JSON lines (e.g. plain text output)
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            // Not valid JSON — treat as plain text fallback
            agent_texts.push(line.to_string());
            continue;
        };

        let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match event_type {
            "item.completed" => {
                // Extract agent message text
                if let Some(item) = event.get("item") {
                    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if item_type == "agent_message" {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            if !text.is_empty() {
                                agent_texts.push(text.to_string());
                            }
                        }
                    }
                }
            }
            "turn.completed" => {
                // Extract usage stats if available
                if let Some(usage) = event.get("usage") {
                    if let Some(v) = usage.get("input_tokens").and_then(|v| v.as_i64()) {
                        input_tokens += v;
                    }
                    if let Some(v) = usage.get("output_tokens").and_then(|v| v.as_i64()) {
                        output_tokens += v;
                    }
                    if let Some(v) = usage.get("total_cost").and_then(|v| v.as_f64()) {
                        total_cost += v;
                    }
                }
            }
            _ => {
                // thread.started, turn.started, etc. — debug only
                eprintln!("[codex:event] {}", event_type);
            }
        }
    }

    let content = agent_texts.join("\n\n").trim().to_string();

    if content.is_empty() && !stderr_content.trim().is_empty() {
        return Err(AppError::Agent(format!(
            "codex produced no output: {}",
            stderr_content.trim()
        )));
    }

    if content.is_empty() {
        return Err(AppError::Agent(
            "codex returned no agent_message events".to_string(),
        ));
    }

    Ok(RunOutput {
        content,
        cost_usd: total_cost,
        input_tokens,
        output_tokens,
        session_id: None,
    })
}

/// Streaming variant of `run()` — reads JSONL events line-by-line and emits
/// partial content via callbacks as each `item.completed` event arrives.
///
/// `on_progress` — called for non-content events (thread.started, turn.started)
/// `on_chunk` — called with accumulated content so far when a new agent_message arrives
pub fn stream_run<F1, F2>(
    input: RunInput,
    mut on_progress: F1,
    mut on_chunk: F2,
) -> Result<RunOutput, AppError>
where
    F1: FnMut(&str),
    F2: FnMut(&str),
{
    let (codex_cmd, codex_script) = resolve_codex();

    let mut cmd = Command::new(&codex_cmd);
    if let Some(ref script) = codex_script {
        cmd.arg(script);
    }

    cmd.arg("exec")
        .arg("--json")
        .arg("--skip-git-repo-check")
        .arg("--color=never");

    if let Some(model) = &input.model {
        cmd.arg("--model").arg(model);
    }

    cmd.arg("-");
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(resolve_cwd(input.project_path.as_deref()));

    let mut child = cmd.spawn().map_err(|e| {
        AppError::Agent(format!("Failed to spawn codex ({}): {}", codex_cmd, e))
    })?;

    // Write prompt to stdin
    if let Some(mut stdin) = child.stdin.take() {
        let prompt_bytes = input.prompt.as_bytes().to_vec();
        thread::spawn(move || { let _ = stdin.write_all(&prompt_bytes); });
    }

    // Drain stderr in background
    let mut stderr_pipe = child.stderr.take()
        .ok_or_else(|| AppError::Agent("Failed to capture codex stderr".into()))?;
    let stderr_handle = thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr_pipe.read_to_string(&mut buf);
        buf
    });

    // Read stdout line-by-line for streaming
    let stdout_pipe = child.stdout.take()
        .ok_or_else(|| AppError::Agent("Failed to capture codex stdout".into()))?;
    let reader = BufReader::new(stdout_pipe);

    let mut agent_texts: Vec<String> = Vec::new();
    let mut input_tokens: i64 = 0;
    let mut output_tokens: i64 = 0;
    let mut total_cost: f64 = 0.0;

    for line_result in reader.lines() {
        let Ok(line) = line_result else { break; };
        let line = line.trim().to_string();
        if line.is_empty() { continue; }

        let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) else {
            agent_texts.push(line);
            let accumulated = agent_texts.join("\n\n");
            on_chunk(&accumulated);
            continue;
        };

        let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match event_type {
            "item.started" => {
                if let Some(item) = event.get("item") {
                    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match item_type {
                        "command_execution" => {
                            let cmd = item.get("command").and_then(|v| v.as_str()).unwrap_or("bash");
                            let step = serde_json::json!({
                                "type": "command",
                                "name": "Bash",
                                "input": cmd.chars().take(120).collect::<String>(),
                                "status": "running"
                            });
                            on_progress(&format!("__STEP__:{}", step));
                        }
                        "file_change" => {
                            let file = item.get("file").and_then(|v| v.as_str()).unwrap_or("file");
                            let step = serde_json::json!({
                                "type": "file_change",
                                "name": "Edit",
                                "input": file,
                                "status": "running"
                            });
                            on_progress(&format!("__STEP__:{}", step));
                        }
                        "reasoning" => {
                            let step = serde_json::json!({
                                "type": "thinking",
                                "name": "Reasoning",
                                "input": "",
                                "status": "running"
                            });
                            on_progress(&format!("__STEP__:{}", step));
                        }
                        _ => {
                            on_progress(event_type);
                        }
                    }
                }
            }
            "item.completed" => {
                if let Some(item) = event.get("item") {
                    let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match item_type {
                        "agent_message" => {
                            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                                if !text.is_empty() {
                                    agent_texts.push(text.to_string());
                                    let accumulated = agent_texts.join("\n\n");
                                    on_chunk(&accumulated);
                                }
                            }
                        }
                        "command_execution" => {
                            let cmd = item.get("command").and_then(|v| v.as_str()).unwrap_or("bash");
                            let status_str = item.get("status").and_then(|v| v.as_str()).unwrap_or("done");
                            let step = serde_json::json!({
                                "type": "command",
                                "name": "Bash",
                                "input": cmd.chars().take(120).collect::<String>(),
                                "status": if status_str == "failed" { "error" } else { "done" }
                            });
                            on_progress(&format!("__STEP__:{}", step));
                        }
                        "file_change" => {
                            let file = item.get("file").and_then(|v| v.as_str()).unwrap_or("file");
                            let step = serde_json::json!({
                                "type": "file_change",
                                "name": "Edit",
                                "input": file,
                                "status": "done"
                            });
                            on_progress(&format!("__STEP__:{}", step));
                        }
                        _ => {}
                    }
                }
            }
            "turn.completed" => {
                if let Some(usage) = event.get("usage") {
                    if let Some(v) = usage.get("input_tokens").and_then(|v| v.as_i64()) { input_tokens += v; }
                    if let Some(v) = usage.get("output_tokens").and_then(|v| v.as_i64()) { output_tokens += v; }
                    if let Some(v) = usage.get("total_cost").and_then(|v| v.as_f64()) { total_cost += v; }
                }
            }
            _ => {
                on_progress(event_type);
            }
        }
    }

    let exit_status = child.wait()?;
    let stderr_content = stderr_handle.join().unwrap_or_default();

    if !exit_status.success() {
        let detail = if !stderr_content.trim().is_empty() {
            stderr_content.trim().to_string()
        } else {
            format!("exit code {:?}", exit_status.code())
        };
        return Err(AppError::Agent(format!("codex failed: {}", detail)));
    }

    let content = agent_texts.join("\n\n").trim().to_string();

    Ok(RunOutput {
        content,
        cost_usd: total_cost,
        input_tokens,
        output_tokens,
        session_id: None,
    })
}
