use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::thread;

use crate::agents::claude::{RunInput, RunOutput};
use crate::errors::AppError;
use crate::no_console::NoConsole;

/// Resolve the codex binary/script path via shared resolve module.
fn resolve_codex() -> (String, Option<String>) {
    use super::resolve::{NpmCliConfig, resolve_npm_cli};
    let resolved = resolve_npm_cli(&NpmCliConfig {
        bin_name: "codex",
        npm_package: "@openai/codex",
        npm_entry: "bin/codex.js",
    });
    (resolved.command, resolved.script_arg)
}

use super::claude::resolve_cwd;

/// Codex `exec --json` 은 한 턴 안에서 `item.completed(agent_message)` 를
/// 여러 번 emit 할 수 있다 (reasoning 중간 답변 → 도구 호출 → 최종 답변).
/// 단순 `push` 하면 같은 문단이 두 번 찍히는 현상이 발생한다.
///
/// 정책:
/// - 새 텍스트가 마지막 메시지와 동일 → skip
/// - 새 텍스트가 마지막 메시지를 **prefix 로 포함** → 기존을 대체 (점진적 확장)
/// - 그 외 → append
fn push_agent_text_dedup(texts: &mut Vec<String>, incoming: &str) {
    let trimmed = incoming.trim();
    if trimmed.is_empty() { return; }
    if let Some(last) = texts.last() {
        let last_tr = last.trim();
        if last_tr == trimmed { return; }
        if trimmed.starts_with(last_tr) && trimmed.len() > last_tr.len() {
            *texts.last_mut().unwrap() = incoming.to_string();
            return;
        }
        // 사용자가 관찰한 "같은 문단 2회" 패턴: Codex 가 reasoning 후 같은 텍스트를
        // 다시 내뱉는 경우. prefix 관계는 아니어도 60% 이상 겹치면 중복으로 판단.
        if last_tr.len() >= 40 && trimmed.contains(last_tr) {
            *texts.last_mut().unwrap() = incoming.to_string();
            return;
        }
    }
    texts.push(incoming.to_string());
}

/// Execute `codex exec` as a one-shot non-interactive subprocess.
///
/// Prompt is delivered via **stdin** (tunadish pattern: `exec --json ... -`).
/// Cost and token fields are unavailable; returned as 0.
pub fn run(input: RunInput) -> Result<RunOutput, AppError> {
    let (codex_cmd, codex_script) = resolve_codex();

    let mut cmd = Command::new(&codex_cmd);
    cmd.no_console();
    if let Some(ref script) = codex_script {
        cmd.arg(script);
    }

    cmd.arg("exec")
        .arg("--json")
        .arg("--skip-git-repo-check")
        .arg("--color=never")
        .arg("--full-auto");

    if let Some(model) = &input.model {
        cmd.arg("--model").arg(model);
    }

    for img in &input.image_paths {
        cmd.arg("-i").arg(img);
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
            push_agent_text_dedup(&mut agent_texts, line);
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
                                push_agent_text_dedup(&mut agent_texts, text);
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
        last_rate_limit: None,
        fresh_fallback: false,
        window_rotated: None,
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
    cmd.no_console();
    if let Some(ref script) = codex_script {
        cmd.arg(script);
    }

    cmd.arg("exec")
        .arg("--json")
        .arg("--skip-git-repo-check")
        .arg("--color=never")
        .arg("--full-auto");

    if let Some(model) = &input.model {
        cmd.arg("--model").arg(model);
    }

    for img in &input.image_paths {
        cmd.arg("-i").arg(img);
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
            push_agent_text_dedup(&mut agent_texts, &line);
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
                                    push_agent_text_dedup(&mut agent_texts, text);
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
        last_rate_limit: None,
        fresh_fallback: false,
        window_rotated: None,
    })
}

#[cfg(test)]
mod tests {
    use super::push_agent_text_dedup;

    #[test]
    fn dedup_skips_exact_duplicate() {
        let mut v: Vec<String> = Vec::new();
        push_agent_text_dedup(&mut v, "hello world");
        push_agent_text_dedup(&mut v, "hello world");
        assert_eq!(v, vec!["hello world".to_string()]);
    }

    #[test]
    fn dedup_replaces_prefix_extension() {
        let mut v: Vec<String> = Vec::new();
        push_agent_text_dedup(&mut v, "Step 1 draft");
        push_agent_text_dedup(&mut v, "Step 1 draft\n\nStep 2 final");
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("Step 2 final"));
    }

    #[test]
    fn dedup_appends_when_distinct() {
        let mut v: Vec<String> = Vec::new();
        push_agent_text_dedup(&mut v, "first topic discussion");
        push_agent_text_dedup(&mut v, "totally different topic result");
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn dedup_handles_same_paragraph_repeated_midstream() {
        let para = "Architect Codex입니다. 요청하신 6개 이슈는 기존 범위를 넘어섭니다. 파일 경로를 먼저 검증하겠습니다.";
        let mut v: Vec<String> = Vec::new();
        push_agent_text_dedup(&mut v, para);
        push_agent_text_dedup(&mut v, para);
        assert_eq!(v, vec![para.to_string()]);
    }

    #[test]
    fn dedup_ignores_whitespace_diff() {
        let mut v: Vec<String> = Vec::new();
        push_agent_text_dedup(&mut v, "same content");
        push_agent_text_dedup(&mut v, "  same content  ");
        assert_eq!(v.len(), 1);
    }
}
