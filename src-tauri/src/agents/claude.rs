use std::io::{BufRead, BufReader, Read, Write as IoWrite};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use serde::Deserialize;
use crate::errors::AppError;

/// Resolve working directory for CLI agent execution.
/// If a project path is provided, the agent runs inside that project (read+write).
/// Otherwise falls back to temp_dir (no project context).
pub fn resolve_cwd(project_path: Option<&str>) -> PathBuf {
    if let Some(p) = project_path {
        let path = PathBuf::from(p);
        if path.is_dir() {
            return path;
        }
    }
    std::env::temp_dir()
}

// ─── Streaming JSON types (--output-format stream-json) ───────────────────

/// One JSON line emitted by `claude --output-format stream-json`
#[derive(Deserialize)]
struct StreamLine {
    #[serde(rename = "type")]
    line_type: String,
    // assistant event
    message: Option<StreamAssistantMsg>,
    // result event
    result: Option<String>,
    is_error: Option<bool>,
    cost_usd: Option<f64>,
    total_cost_usd: Option<f64>,
    total_input_tokens: Option<i64>,
    total_output_tokens: Option<i64>,
    session_id: Option<String>,
}

#[derive(Deserialize)]
struct StreamAssistantMsg {
    content: Option<Vec<StreamContentBlock>>,
}

#[derive(Deserialize)]
struct StreamContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
    thinking: Option<String>,
    // tool_use fields
    name: Option<String>,
    input: Option<serde_json::Value>,
}

/// Extract thinking content from assistant message (for progress display).
fn extract_thinking(msg: &StreamAssistantMsg) -> Option<String> {
    msg.content
        .as_ref()
        .and_then(|blocks| {
            blocks.iter()
                .filter(|b| b.block_type == "thinking")
                .filter_map(|b| b.thinking.as_deref())
                .next()
                .map(|s| s.to_string())
        })
}

/// Extract tool_use invocations from assistant message (for progress display).
fn extract_tool_uses(msg: &StreamAssistantMsg) -> Vec<String> {
    msg.content
        .as_ref()
        .map(|blocks| {
            blocks.iter()
                .filter(|b| b.block_type == "tool_use")
                .filter_map(|b| {
                    let name = b.name.as_deref()?;
                    // Summarize input — show first ~80 chars of stringified input
                    let input_summary = b.input.as_ref().map(|v| {
                        let s = v.to_string();
                        if s.len() > 80 { format!("{}…", &s[..80]) } else { s }
                    }).unwrap_or_default();
                    Some(format!("🔧 {} {}", name, input_summary))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn extract_text(msg: &StreamAssistantMsg) -> String {
    msg.content
        .as_ref()
        .and_then(|blocks| {
            blocks
                .iter()
                .filter(|b| b.block_type == "text")
                .filter_map(|b| b.text.as_deref())
                .next()
        })
        .unwrap_or("")
        .to_string()
}

/// Shape of `claude -p --output-format json` stdout
#[derive(Debug, Deserialize)]
pub struct ClaudeJsonOutput {
    pub result: Option<String>,
    pub is_error: Option<bool>,
    pub cost_usd: Option<f64>,
    pub total_input_tokens: Option<i64>,
    pub total_output_tokens: Option<i64>,
    pub session_id: Option<String>,
}

pub struct RunInput {
    pub prompt: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    /// Session resume token from the previous CompletedEvent (session_id).
    /// None = new session. Some(token) = continue existing session via --resume.
    pub resume_token: Option<String>,
    /// Project directory — agent runs here (read+write own project).
    pub project_path: Option<String>,
}

pub struct RunOutput {
    pub content: String,
    pub cost_usd: f64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub session_id: Option<String>,
}

/// Execute `claude -p` with `--output-format stream-json`.
///
/// Two callbacks:
/// - `on_progress`: called for thinking/tool events (progress log, not final answer)
/// - `on_chunk`: called when assistant text content arrives (final answer streaming)
///
/// Returns the final `RunOutput` when the `result` line arrives.
/// Caller must NOT hold the DbState lock while calling this function.
pub fn stream_run<F, G, C>(input: RunInput, mut on_progress: G, mut on_chunk: F, is_cancelled: C) -> Result<RunOutput, AppError>
where
    F: FnMut(String),
    G: FnMut(String),
    C: Fn() -> bool,
{
    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg(&input.prompt)
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(resolve_cwd(input.project_path.as_deref()));

    if let Some(model) = &input.model {
        cmd.arg("--model").arg(model);
    }

    // Write system prompt to temp file to avoid Windows 32K command line limit.
    // --append-system-prompt-file reads from file instead of inline arg.
    let _prompt_file = if let Some(system_prompt) = &input.system_prompt {
        let mut tmp = tempfile::NamedTempFile::new()
            .map_err(|e| AppError::Agent(format!("Failed to create temp file: {}", e)))?;
        tmp.write_all(system_prompt.as_bytes())
            .map_err(|e| AppError::Agent(format!("Failed to write system prompt: {}", e)))?;
        tmp.flush()
            .map_err(|e| AppError::Agent(format!("Failed to flush system prompt: {}", e)))?;
        cmd.arg("--append-system-prompt-file").arg(tmp.path());
        Some(tmp) // keep alive until child exits
    } else {
        None
    };

    if let Some(token) = &input.resume_token {
        cmd.arg("--resume").arg(token);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Agent(format!("Failed to spawn claude: {}", e)))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Agent("Failed to capture stdout".into()))?;

    // Drain stderr in a background thread to prevent pipe-buffer deadlock
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Agent("Failed to capture stderr".into()))?;
    let stderr_handle = thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr_pipe.read_to_string(&mut buf);
        buf
    });

    let reader = BufReader::new(stdout);
    let mut final_output: Option<RunOutput> = None;
    let mut unparsed_lines: Vec<String> = Vec::new();

    for raw in reader.lines() {
        // Check cancel between each line of streaming output
        if is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::Agent("cancelled by user".into()));
        }
        let line = raw?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed: StreamLine = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                // Collect unparseable stdout lines (may contain plain-text errors)
                unparsed_lines.push(line);
                continue;
            }
        };

        match parsed.line_type.as_str() {
            "system" => {
                // Init event — report as progress
                on_progress("Agent initializing...".into());
            }
            "assistant" => {
                if let Some(msg) = &parsed.message {
                    // Thinking → progress (show full thinking content, not just last line)
                    if let Some(thinking) = extract_thinking(msg) {
                        for line in thinking.lines() {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                on_progress(format!("💭 {}", trimmed));
                            }
                        }
                    }
                    // Tool use → progress
                    for tool_line in extract_tool_uses(msg) {
                        on_progress(tool_line);
                    }
                    // Text → final answer chunk
                    let text = extract_text(msg);
                    if !text.is_empty() {
                        on_chunk(text);
                    }
                }
            }
            "result" => {
                if parsed.is_error.unwrap_or(false) {
                    let _ = child.wait();
                    return Err(AppError::Agent(format!(
                        "claude reported error: {}",
                        parsed.result.as_deref().unwrap_or("unknown")
                    )));
                }
                final_output = Some(RunOutput {
                    content: parsed.result.unwrap_or_default(),
                    cost_usd: parsed.total_cost_usd.or(parsed.cost_usd).unwrap_or(0.0),
                    input_tokens: parsed.total_input_tokens.unwrap_or(0),
                    output_tokens: parsed.total_output_tokens.unwrap_or(0),
                    session_id: parsed.session_id,
                });
            }
            _ => {}
        }
    }

    child.wait()?;
    let stderr_content = stderr_handle.join().unwrap_or_default();

    final_output.ok_or_else(|| {
        // Build a diagnostic message using stderr, then unparsed stdout lines as fallback
        let detail = if !stderr_content.trim().is_empty() {
            stderr_content.trim().to_string()
        } else if !unparsed_lines.is_empty() {
            unparsed_lines.join(" | ")
        } else {
            "no output received".to_string()
        };
        AppError::Agent(format!("claude stream failed: {}", detail))
    })
}

/// Execute `claude -p` as a one-shot subprocess and return the result.
///
/// Caller must NOT hold the DbState lock while calling this function,
/// since the subprocess can take an arbitrarily long time.
pub fn run(input: RunInput) -> Result<RunOutput, AppError> {
    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg(&input.prompt)
        .arg("--output-format")
        .arg("json")
        .current_dir(resolve_cwd(input.project_path.as_deref()));

    if let Some(model) = &input.model {
        cmd.arg("--model").arg(model);
    }

    let _prompt_file = if let Some(system_prompt) = &input.system_prompt {
        let mut tmp = tempfile::NamedTempFile::new()
            .map_err(|e| AppError::Agent(format!("Failed to create temp file: {}", e)))?;
        tmp.write_all(system_prompt.as_bytes())
            .map_err(|e| AppError::Agent(format!("Failed to write system prompt: {}", e)))?;
        tmp.flush()
            .map_err(|e| AppError::Agent(format!("Failed to flush system prompt: {}", e)))?;
        cmd.arg("--append-system-prompt-file").arg(tmp.path());
        Some(tmp)
    } else {
        None
    };

    if let Some(token) = &input.resume_token {
        cmd.arg("--resume").arg(token);
    }

    let output = cmd.output().map_err(|e| {
        AppError::Agent(format!("Failed to spawn claude: {}", e))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Agent(format!(
            "claude exited {:?}: {}",
            output.status.code(),
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: ClaudeJsonOutput = serde_json::from_str(stdout.trim()).map_err(|e| {
        AppError::Agent(format!(
            "Failed to parse claude output: {} | raw: {}",
            e,
            stdout.trim()
        ))
    })?;

    if parsed.is_error.unwrap_or(false) {
        return Err(AppError::Agent(format!(
            "claude reported error: {}",
            parsed.result.as_deref().unwrap_or("unknown")
        )));
    }

    Ok(RunOutput {
        content: parsed.result.unwrap_or_default(),
        cost_usd: parsed.cost_usd.unwrap_or(0.0),
        input_tokens: parsed.total_input_tokens.unwrap_or(0),
        output_tokens: parsed.total_output_tokens.unwrap_or(0),
        session_id: parsed.session_id,
    })
}
