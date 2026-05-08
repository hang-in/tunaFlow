use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::thread;

use serde::Deserialize;

use crate::agents::claude::{RunInput, RunOutput};
use crate::errors::AppError;
use crate::no_console::NoConsole;

/// Resolve the gemini binary/script path via shared resolve module.
fn resolve_gemini() -> (String, Option<String>) {
    use super::resolve::{NpmCliConfig, resolve_npm_cli};
    let resolved = resolve_npm_cli(&NpmCliConfig {
        bin_name: "gemini",
        npm_package: "@google/gemini-cli",
        npm_entry: "dist/index.js",
    });
    (resolved.command, resolved.script_arg)
}

use super::claude::resolve_cwd;

/// Execute `gemini -p <prompt>` as a one-shot non-interactive subprocess.
///
/// Cost and token fields are unavailable; returned as 0.
pub fn run(input: RunInput) -> Result<RunOutput, AppError> {
    let (gemini_cmd, gemini_script) = resolve_gemini();

    let mut cmd = Command::new(&gemini_cmd);
    cmd.no_console();
    if let Some(ref script) = gemini_script {
        cmd.arg("--no-warnings=DEP0040").arg(script);
    }

    cmd.arg("-p").arg(&input.prompt);

    if let Some(model) = &input.model {
        if model != "auto" {
            cmd.arg("--model").arg(model);
        }
    }

    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(resolve_cwd(input.project_path.as_deref()));

    let mut child = cmd.spawn().map_err(|e| {
        AppError::Agent(format!("Failed to spawn gemini ({}): {}", gemini_cmd, e))
    })?;

    // Drain stderr in background
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Agent("Failed to capture gemini stderr".into()))?;
    let stderr_handle = thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr_pipe.read_to_string(&mut buf);
        buf
    });

    // Read stdout
    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Agent("Failed to capture gemini stdout".into()))?;
    let mut stdout_raw = String::new();
    stdout_pipe
        .read_to_string(&mut stdout_raw)
        .map_err(|e| AppError::Agent(format!("Failed to read gemini stdout: {}", e)))?;

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
        return Err(AppError::Agent(format!("gemini failed: {}", detail)));
    }

    let content = stdout_raw.trim().to_string();

    if content.is_empty() && !stderr_content.trim().is_empty() {
        return Err(AppError::Agent(format!(
            "gemini produced no output: {}",
            stderr_content.trim()
        )));
    }

    Ok(RunOutput {
        content,
        cost_usd: 0.0,
        input_tokens: 0,
        output_tokens: 0,
        session_id: None,
        last_rate_limit: None,
        fresh_fallback: false,
        window_rotated: None,
    })
}

// ─── Streaming JSON types (--output-format stream-json) ───────────────────

#[derive(Deserialize)]
struct GeminiStreamLine {
    #[serde(rename = "type")]
    line_type: String,
    // init event
    session_id: Option<String>,
    model: Option<String>,
    // message event
    role: Option<String>,
    content: Option<String>,
    // tool_use event
    tool_name: Option<String>,
    parameters: Option<serde_json::Value>,
    // tool_result event
    tool_id: Option<String>,
    output: Option<String>,
    // result event
    status: Option<String>,
    stats: Option<GeminiStats>,
}

#[derive(Deserialize)]
struct GeminiStats {
    #[allow(dead_code)]
    total_tokens: Option<i64>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    #[serde(default)]
    tool_calls: i64,
}

/// Execute `gemini -p` with `--output-format stream-json`.
///
/// Two callbacks:
/// - `on_progress`: called for init/status events (progress log)
/// - `on_chunk`: called when assistant text content arrives (streaming)
///
/// Returns the final `RunOutput` when the `result` line arrives.
pub fn stream_run<F, G, C>(input: RunInput, mut on_progress: G, mut on_chunk: F, is_cancelled: C) -> Result<RunOutput, AppError>
where
    F: FnMut(String),
    G: FnMut(String),
    C: Fn() -> bool,
{
    let (gemini_cmd, gemini_script) = resolve_gemini();

    let mut cmd = Command::new(&gemini_cmd);
    cmd.no_console();
    if let Some(ref script) = gemini_script {
        cmd.arg("--no-warnings=DEP0040").arg(script);
    }

    cmd.arg("-p").arg(&input.prompt)
        .arg("--output-format").arg("stream-json")
        .arg("-y"); // auto-approve for non-interactive

    if let Some(model) = &input.model {
        if model != "auto" {
            cmd.arg("--model").arg(model);
        }
    }

    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(super::claude::resolve_cwd(input.project_path.as_deref()));

    let mut child = cmd.spawn().map_err(|e| {
        AppError::Agent(format!("Failed to spawn gemini ({}): {}", gemini_cmd, e))
    })?;

    // Drain stderr in background
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Agent("Failed to capture gemini stderr".into()))?;
    let stderr_handle = thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr_pipe.read_to_string(&mut buf);
        buf
    });

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Agent("Failed to capture gemini stdout".into()))?;

    let reader = BufReader::new(stdout);
    let mut accumulated_content = String::new();
    let mut session_id: Option<String> = None;
    let mut total_in: i64 = 0;
    let mut total_out: i64 = 0;
    let mut got_result = false;

    on_progress("Gemini starting...".into());

    for raw in reader.lines() {
        if is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::Agent("cancelled by user".into()));
        }
        let line = raw?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed: GeminiStreamLine = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        match parsed.line_type.as_str() {
            "init" => {
                session_id = parsed.session_id;
                let model_name = parsed.model.unwrap_or_else(|| "gemini".into());
                on_progress(format!("🚀 Model: {}", model_name));
            }
            "message" => {
                if parsed.role.as_deref() == Some("assistant") {
                    if let Some(delta) = parsed.content {
                        if !delta.is_empty() {
                            accumulated_content.push_str(&delta);
                            on_chunk(accumulated_content.clone());
                        }
                    }
                } else if parsed.role.as_deref() == Some("tool") {
                    // Tool result — structured step (done)
                    if let Some(text) = &parsed.content {
                        let summary = if text.len() > 120 {
                            let mut end = 120;
                            while end > 0 && !text.is_char_boundary(end) { end -= 1; }
                            format!("{}…", &text[..end])
                        } else { text.clone() };
                        let step = serde_json::json!({
                            "type": "tool_result",
                            "name": "Tool",
                            "input": summary,
                            "status": "done"
                        });
                        on_progress(format!("__STEP__:{}", step));
                    }
                }
            }
            "tool_use" => {
                let name = parsed.tool_name.as_deref().unwrap_or("Tool");
                let input_summary = parsed.parameters.as_ref().map(|v| {
                    let s = v.to_string();
                    if s.len() > 120 {
                        let mut end = 120;
                        while end > 0 && !s.is_char_boundary(end) { end -= 1; }
                        format!("{}…", &s[..end])
                    } else { s }
                }).unwrap_or_default();
                let step = serde_json::json!({
                    "type": "tool_use",
                    "name": name,
                    "input": input_summary,
                    "status": "running"
                });
                on_progress(format!("__STEP__:{}", step));
            }
            "tool_result" => {
                let name = parsed.tool_id.as_deref().unwrap_or("Tool");
                let output = parsed.output.as_deref().unwrap_or("");
                let summary = if output.len() > 120 {
                    let mut end = 120;
                    while end > 0 && !output.is_char_boundary(end) { end -= 1; }
                    format!("{}…", &output[..end])
                } else { output.to_string() };
                let status = parsed.status.as_deref().unwrap_or("success");
                let step = serde_json::json!({
                    "type": "tool_result",
                    "name": name,
                    "input": summary,
                    "status": if status == "error" { "error" } else { "done" }
                });
                on_progress(format!("__STEP__:{}", step));
            }
            "result" => {
                got_result = true;
                if let Some(stats) = &parsed.stats {
                    total_in = stats.input_tokens.unwrap_or(0);
                    total_out = stats.output_tokens.unwrap_or(0);
                    if stats.tool_calls > 0 {
                        on_progress(format!("🔧 {} tool calls completed", stats.tool_calls));
                    }
                }
                if parsed.status.as_deref() == Some("error") {
                    let _ = child.wait();
                    return Err(AppError::Agent("gemini reported error".into()));
                }
            }
            _ => {}
        }
    }

    child.wait()?;
    let stderr_content = stderr_handle.join().unwrap_or_default();

    if !got_result && accumulated_content.is_empty() {
        let detail = if !stderr_content.trim().is_empty() {
            stderr_content.trim().to_string()
        } else {
            "no output received".to_string()
        };
        return Err(AppError::Agent(format!("gemini stream failed: {}", detail)));
    }

    Ok(RunOutput {
        content: accumulated_content,
        cost_usd: 0.0,
        input_tokens: total_in,
        output_tokens: total_out,
        session_id,
        last_rate_limit: None,
        fresh_fallback: false,
        window_rotated: None,
    })
}
