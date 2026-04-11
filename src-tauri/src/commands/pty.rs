use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;

use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::errors::AppError;

/// Payload emitted for each PTY output chunk.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyOutputPayload {
    pub session_id: u32,
    pub data: String,
}

/// Payload emitted when PTY process exits.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyExitPayload {
    pub session_id: u32,
    pub exit_code: Option<i32>,
}

/// Managed state for PTY sessions.
pub struct PtyState {
    next_id: Mutex<u32>,
    sessions: Mutex<HashMap<u32, PtySession>>,
}

struct PtySession {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    _child: Box<dyn portable_pty::Child + Send>,
}

impl PtyState {
    pub fn new() -> Self {
        Self {
            next_id: Mutex::new(1),
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

/// Spawn a PTY process and start streaming output via events.
#[tauri::command]
pub fn pty_spawn(
    file: String,
    args: Vec<String>,
    cwd: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
    env: Option<std::collections::HashMap<String, String>>,
    app: AppHandle,
    state: State<'_, PtyState>,
) -> Result<u32, AppError> {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: rows.unwrap_or(24),
            cols: cols.unwrap_or(80),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| AppError::Agent(format!("Failed to open PTY: {}", e)))?;

    let mut cmd = CommandBuilder::new(&file);
    for arg in &args {
        cmd.arg(arg);
    }
    if let Some(ref dir) = cwd {
        cmd.cwd(dir);
    }
    // Set environment variables (e.g., TERM=dumb, NO_COLOR=1)
    if let Some(ref env_map) = env {
        for (k, v) in env_map {
            cmd.env(k, v);
        }
    }

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| AppError::Agent(format!("Failed to spawn {}: {}", file, e)))?;

    // Drop slave — we only need master for I/O
    drop(pair.slave);

    let writer = Arc::new(Mutex::new(
        pair.master
            .take_writer()
            .map_err(|e| AppError::Agent(format!("Failed to get PTY writer: {}", e)))?,
    ));

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| AppError::Agent(format!("Failed to get PTY reader: {}", e)))?;

    // Assign session ID
    let session_id = {
        let mut id = state.next_id.lock();
        let current = *id;
        *id += 1;
        current
    };

    // Store session
    {
        let mut sessions = state.sessions.lock();
        sessions.insert(
            session_id,
            PtySession {
                writer: Arc::clone(&writer),
                _child: child,
            },
        );
    }

    // Spawn reader thread — emits pty:output (raw) + pty:text (screen snapshot) events
    let sid = session_id;
    let pty_cols = cols.unwrap_or(80) as usize;
    let pty_rows = rows.unwrap_or(24) as usize;
    std::thread::spawn(move || {
        use alacritty_terminal::term::{Config as TermConfig, Term, test::TermSize};
        use alacritty_terminal::event::VoidListener;
        use alacritty_terminal::vte::ansi;
        use alacritty_terminal::index::{Line, Column};

        // Noop timeout for Processor
        #[derive(Default)]
        struct NoTimeout;
        impl ansi::Timeout for NoTimeout {
            fn set_timeout(&mut self, _: std::time::Duration) {}
            fn clear_timeout(&mut self) {}
            fn pending_timeout(&self) -> bool { false }
        }

        // Create virtual terminal buffer
        let size = TermSize::new(pty_cols, pty_rows);
        let mut term = Term::new(TermConfig::default(), &size, VoidListener);
        let mut parser = ansi::Processor::<NoTimeout>::new();
        let mut prev_screen_text = String::new();

        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let raw = &buf[..n];
                    let text = String::from_utf8_lossy(raw).to_string();

                    // Raw output → xterm.js (debug terminal panel)
                    let _ = app.emit("pty:output", PtyOutputPayload {
                        session_id: sid, data: text,
                    });

                    // Feed bytes into virtual terminal buffer
                    parser.advance(&mut term, raw);

                    // Read VISIBLE screen from VTE grid (for completion detection)
                    let grid = term.grid();
                    let mut screen_lines: Vec<String> = Vec::new();
                    for row_idx in 0..pty_rows {
                        let row = &grid[Line(row_idx as i32)];
                        let line: String = (0..pty_cols)
                            .map(|col| row[Column(col)].c)
                            .collect::<String>()
                            .trim_end()
                            .to_string();
                        screen_lines.push(line);
                    }
                    while screen_lines.last().map_or(false, |l| l.is_empty()) {
                        screen_lines.pop();
                    }
                    let screen_text = screen_lines.join("\n");

                    // Emit TWO events:
                    // 1. pty:screen — VTE screen snapshot (for completion detection)
                    if screen_text != prev_screen_text && !screen_text.trim().is_empty() {
                        prev_screen_text = screen_text.clone();
                        let _ = app.emit("pty:screen", PtyOutputPayload {
                            session_id: sid, data: screen_text,
                        });
                    }

                    // 2. pty:text — ANSI-stripped text (for response accumulation)
                    let stripped_bytes = strip_ansi_escapes::strip(raw);
                    let stripped = String::from_utf8_lossy(&stripped_bytes)
                        .replace('\r', "")
                        .to_string();
                    if !stripped.trim().is_empty() {
                        let _ = app.emit("pty:text", PtyOutputPayload {
                            session_id: sid, data: stripped,
                        });
                    }
                }
                Err(e) => {
                    eprintln!("[pty] read error for session {}: {}", sid, e);
                    break;
                }
            }
        }

        // Process ended
        let _ = app.emit(
            "pty:exit",
            PtyExitPayload {
                session_id: sid,
                exit_code: None, // portable-pty doesn't easily give exit code here
            },
        );
    });

    let active_count = state.sessions.lock().len();
    eprintln!("[pty] spawned session {} — {} {:?} (cwd: {:?}), active sessions: {}", session_id, file, args, cwd, active_count);
    Ok(session_id)
}

/// Write data to a PTY session's stdin.
#[tauri::command]
pub fn pty_write(
    session_id: u32,
    data: String,
    state: State<'_, PtyState>,
) -> Result<(), AppError> {
    let sessions = state.sessions.lock();
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| AppError::NotFound(format!("PTY session {} not found", session_id)))?;
    let mut writer = session.writer.lock();
    writer
        .write_all(data.as_bytes())
        .map_err(|e| AppError::Agent(format!("PTY write error: {}", e)))?;
    writer
        .flush()
        .map_err(|e| AppError::Agent(format!("PTY flush error: {}", e)))?;
    Ok(())
}

/// Resize a PTY session.
#[tauri::command]
pub fn pty_resize(
    session_id: u32,
    cols: u16,
    rows: u16,
    _state: State<'_, PtyState>,
) -> Result<(), AppError> {
    // portable-pty resize requires the master, but we only stored writer/reader.
    // For now, log and skip — resize support requires storing the master handle.
    // Resize not yet implemented — requires storing master handle
    let _ = (session_id, cols, rows);
    Ok(())
}

/// Kill all PTY sessions.
#[tauri::command]
pub fn pty_kill_all(
    state: State<'_, PtyState>,
) -> Result<usize, AppError> {
    let mut sessions = state.sessions.lock();
    let count = sessions.len();
    for (id, mut session) in sessions.drain() {
        let _ = session._child.kill();
        eprintln!("[pty] killed session {} (kill_all)", id);
    }
    Ok(count)
}

/// List JSONL files in the Claude projects directory for a given project path.
/// Used to snapshot before PTY spawn — new files after spawn = PTY session's JSONL.
#[tauri::command]
pub fn pty_list_jsonl_files(
    project_path: String,
) -> Result<Vec<String>, AppError> {
    let encoded = project_path.replace('/', "-");
    let claude_dir = dirs::home_dir()
        .ok_or_else(|| AppError::Agent("no home dir".into()))?
        .join(".claude/projects")
        .join(&encoded);

    if !claude_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in std::fs::read_dir(&claude_dir).map_err(|e| AppError::Agent(e.to_string()))? {
        let entry = entry.map_err(|e| AppError::Agent(e.to_string()))?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "jsonl") && !path.to_string_lossy().contains("subagents") {
            files.push(path.to_string_lossy().to_string());
        }
    }
    Ok(files)
}

/// Find the latest JSONL file for a project and return the last assistant message.
/// Claude Code writes conversation logs to ~/.claude/projects/{encoded-cwd}/{sessionId}.jsonl
///
/// If `jsonl_path` is provided, read that specific file (PTY session tracking).
/// Otherwise, fall back to the most recently modified .jsonl file.
#[tauri::command]
pub fn pty_poll_jsonl(
    project_path: String,
    after_line: Option<usize>,
    jsonl_path: Option<String>,
) -> Result<Option<PtyJsonlResult>, AppError> {
    use std::io::BufRead;

    let jsonl_path: std::path::PathBuf = if let Some(ref explicit) = jsonl_path {
        let p = std::path::PathBuf::from(explicit);
        if !p.exists() {
            return Ok(None);
        }
        p
    } else {
        // Fallback: find most recently modified .jsonl file
        let encoded = project_path.replace('/', "-");
        let claude_dir = dirs::home_dir()
            .ok_or_else(|| AppError::Agent("no home dir".into()))?
            .join(".claude/projects")
            .join(&encoded);

        if !claude_dir.exists() {
            return Ok(None);
        }

        let mut latest: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
        for entry in std::fs::read_dir(&claude_dir).map_err(|e| AppError::Agent(e.to_string()))? {
            let entry = entry.map_err(|e| AppError::Agent(e.to_string()))?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "jsonl") && !path.to_string_lossy().contains("subagents") {
                if let Ok(meta) = path.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if latest.as_ref().map_or(true, |(t, _)| modified > *t) {
                            latest = Some((modified, path));
                        }
                    }
                }
            }
        }

        match latest {
            Some((_, p)) => p,
            None => return Ok(None),
        }
    };

    // Read lines after `after_line` index, collect ALL assistant messages.
    // Claude Code JSONL interleaves: user → assistant(tool_use) → user(tool_result) → assistant(text)
    // Tool results are recorded as "user" type, so we simply collect all assistant messages
    // after the baseline without clearing on user messages.
    let file = std::fs::File::open(&jsonl_path).map_err(|e| AppError::Agent(e.to_string()))?;
    let reader = std::io::BufReader::new(file);
    let skip = after_line.unwrap_or(0);
    let mut total_lines = 0usize;
    // Collect both assistant messages and user messages (which contain tool_result)
    let mut all_messages: Vec<serde_json::Value> = Vec::new();

    for (idx, line) in reader.lines().enumerate() {
        total_lines = idx + 1;
        if idx < skip { continue; }
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            match value.get("type").and_then(|t| t.as_str()) {
                Some("assistant") | Some("user") | Some("human") => {
                    all_messages.push(value);
                }
                _ => {}
            }
        }
    }

    // Filter: only keep if there are assistant messages
    let assistant_messages: Vec<&serde_json::Value> = all_messages.iter()
        .filter(|v| v.get("type").and_then(|t| t.as_str()) == Some("assistant"))
        .collect();

    if assistant_messages.is_empty() {
        return Ok(None);
    }

    // Extract tool steps from all messages, matching tool_use → tool_result by ID.
    // Messages interleave: assistant(tool_use) → user(tool_result) → assistant(tool_use) → ... → assistant(text)
    let mut tool_steps: Vec<PtyToolStep> = Vec::new();
    let mut final_text_parts: Vec<String> = Vec::new();
    let mut final_tool_uses: Vec<String> = Vec::new();
    let mut model: Option<String> = None;
    // Map tool_use_id → index in tool_steps for attaching output later
    let mut pending_outputs: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for msg_value in &all_messages {
        let msg_type = msg_value["type"].as_str().unwrap_or("");
        let message = &msg_value["message"];

        if msg_type == "assistant" {
            if model.is_none() {
                model = message["model"].as_str().map(|s| s.to_string());
            }

            if let Some(content) = message["content"].as_array() {
                for item in content {
                    match item["type"].as_str() {
                        Some("text") => {
                            if let Some(t) = item["text"].as_str() {
                                final_text_parts.push(t.to_string());
                            }
                        }
                        Some("tool_use") => {
                            let name = item["name"].as_str().unwrap_or("unknown").to_string();
                            let id = item["id"].as_str().unwrap_or("").to_string();
                            let input_summary = summarize_tool_input(&name, &item["input"]);
                            final_tool_uses.push(name.clone());
                            let idx = tool_steps.len();
                            if !id.is_empty() {
                                pending_outputs.insert(id.clone(), idx);
                            }
                            tool_steps.push(PtyToolStep {
                                step_type: "tool_use".to_string(),
                                name,
                                tool_use_id: if id.is_empty() { None } else { Some(id) },
                                input: input_summary,
                                output: None,
                                status: "done".to_string(),
                            });
                        }
                        Some("thinking") => {
                            let thinking_text = item["thinking"].as_str().unwrap_or("");
                            let summary = if thinking_text.len() > 120 {
                                format!("{}...", &thinking_text[..thinking_text.floor_char_boundary(120)])
                            } else {
                                thinking_text.to_string()
                            };
                            tool_steps.push(PtyToolStep {
                                step_type: "thinking".to_string(),
                                name: "thinking".to_string(),
                                tool_use_id: None,
                                input: summary,
                                output: None,
                                status: "done".to_string(),
                            });
                        }
                        _ => {}
                    }
                }
            }
        } else if msg_type == "user" || msg_type == "human" {
            // Attach tool_result outputs to corresponding tool_use steps
            if let Some(content) = message["content"].as_array() {
                for item in content {
                    if item["type"].as_str() == Some("tool_result") {
                        let tool_use_id = item["tool_use_id"].as_str().unwrap_or("");
                        if let Some(&step_idx) = pending_outputs.get(tool_use_id) {
                            let output = extract_tool_result_content(&item["content"]);
                            if !output.is_empty() {
                                // Truncate to 500 chars for display
                                let truncated = if output.len() > 500 {
                                    format!("{}…", &output[..output.floor_char_boundary(500)])
                                } else {
                                    output
                                };
                                if let Some(step) = tool_steps.get_mut(step_idx) {
                                    step.output = Some(truncated);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Check if the last assistant message has text content (= final response arrived)
    let last_has_text = assistant_messages.last()
        .and_then(|v| v["message"]["content"].as_array())
        .map(|arr| arr.iter().any(|item| item["type"].as_str() == Some("text") && item["text"].as_str().map_or(false, |t| !t.is_empty())))
        .unwrap_or(false);

    // If only tool_use messages (no final text), mark as still running
    let is_complete = last_has_text && !final_text_parts.is_empty();

    Ok(Some(PtyJsonlResult {
        text: final_text_parts.join("\n\n"),
        tool_uses: final_tool_uses,
        tool_steps,
        model,
        total_lines,
        is_complete,
    }))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyToolStep {
    pub step_type: String,
    pub name: String,
    pub tool_use_id: Option<String>,
    pub input: String,
    pub output: Option<String>,
    pub status: String,
}

/// Extract a human-readable summary from tool_use input.
fn summarize_tool_input(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "Read" => input["file_path"].as_str()
            .map(|p| shorten_path(p))
            .unwrap_or_default(),
        "Write" => input["file_path"].as_str()
            .map(|p| shorten_path(p))
            .unwrap_or_default(),
        "Edit" => input["file_path"].as_str()
            .map(|p| shorten_path(p))
            .unwrap_or_default(),
        "Glob" => input["pattern"].as_str()
            .unwrap_or("")
            .to_string(),
        "Grep" => {
            let pattern = input["pattern"].as_str().unwrap_or("");
            let path = input["path"].as_str().map(|p| shorten_path(p)).unwrap_or_default();
            if path.is_empty() { pattern.to_string() }
            else { format!("{} in {}", pattern, path) }
        }
        "Bash" => input["command"].as_str()
            .map(|c| c.chars().take(60).collect::<String>())
            .unwrap_or_default(),
        _ => {
            // Generic: try common field names
            for key in &["file_path", "path", "command", "query", "pattern", "url"] {
                if let Some(v) = input[*key].as_str() {
                    return v.chars().take(80).collect();
                }
            }
            String::new()
        }
    }
}

/// Extract text content from a tool_result content field.
/// Content can be a string or an array of content blocks.
fn extract_tool_result_content(content: &serde_json::Value) -> String {
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    if let Some(arr) = content.as_array() {
        let parts: Vec<String> = arr.iter().filter_map(|item| {
            if item["type"].as_str() == Some("text") {
                item["text"].as_str().map(|s| s.to_string())
            } else {
                None
            }
        }).collect();
        return parts.join("\n");
    }
    String::new()
}

fn shorten_path(path: &str) -> String {
    // Show last 2-3 components: /a/b/c/d/e.ts → c/d/e.ts
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 3 {
        parts.join("/")
    } else {
        parts[parts.len()-3..].join("/")
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyJsonlResult {
    pub text: String,
    pub tool_uses: Vec<String>,
    pub tool_steps: Vec<PtyToolStep>,
    pub model: Option<String>,
    pub total_lines: usize,
    pub is_complete: bool,
}

/// Kill a PTY session.
#[tauri::command]
pub fn pty_kill(
    session_id: u32,
    state: State<'_, PtyState>,
) -> Result<(), AppError> {
    let mut sessions = state.sessions.lock();
    if let Some(mut session) = sessions.remove(&session_id) {
        let _ = session._child.kill();
        let remaining: Vec<u32> = sessions.keys().copied().collect();
        eprintln!("[pty] killed session {}, remaining: {:?}", session_id, remaining);
    } else {
        eprintln!("[pty] kill: session {} not found", session_id);
    }
    Ok(())
}

/// Update the ## tunaFlow Context section in a project's CLAUDE.md.
/// Called when plan changes, persona switches, or on PTY session start.
/// Creates the section if it doesn't exist; replaces it if it does.
#[tauri::command]
pub fn pty_update_claude_md(
    project_path: String,
    context_section: String,
) -> Result<(), AppError> {
    let claude_md = std::path::Path::new(&project_path).join("CLAUDE.md");

    let content = if claude_md.exists() {
        std::fs::read_to_string(&claude_md)
            .map_err(|e| AppError::Agent(format!("read CLAUDE.md: {}", e)))?
    } else {
        String::new()
    };

    let marker_start = "## tunaFlow Context";
    let marker_end = "## "; // Next h2 section

    let new_section = format!("{}\n\n{}\n", marker_start, context_section);

    let updated = if let Some(start_idx) = content.find(marker_start) {
        // Find the end of this section (next ## or EOF)
        let after_start = start_idx + marker_start.len();
        let end_idx = content[after_start..]
            .find(marker_end)
            .map(|i| after_start + i)
            .unwrap_or(content.len());
        format!("{}{}\n{}", &content[..start_idx], new_section, &content[end_idx..])
    } else {
        // Append at the end
        if content.is_empty() {
            new_section
        } else {
            format!("{}\n\n{}", content.trim_end(), new_section)
        }
    };

    std::fs::write(&claude_md, updated)
        .map_err(|e| AppError::Agent(format!("write CLAUDE.md: {}", e)))?;

    eprintln!("[pty] updated CLAUDE.md tunaFlow Context section ({} chars)", context_section.len());
    Ok(())
}

/// Build ContextPack for PTY mode — returns the assembled prompt sections
/// that should be injected into the PTY session (first message or delta).
#[tauri::command]
pub fn pty_build_context(
    conversation_id: String,
    prompt: String,
    project_path: Option<String>,
    active_skills: Vec<String>,
    cross_session_ids: Vec<String>,
    persona_fragment: Option<String>,
    context_mode: Option<String>,
    db: State<crate::db::DbState>,
) -> Result<PtyContextResult, AppError> {
    let conn = db.read.lock().map_err(|_| AppError::Lock)?;
    let (assembled, system_prompt, meta) = crate::commands::agents_helpers::send_common::build_normalized_prompt_with_budget(
        &conn,
        &conversation_id,
        &prompt,
        project_path.as_deref(),
        &active_skills,
        &cross_session_ids,
        persona_fragment.as_deref(),
        context_mode.as_deref(),
        None,
    );
    Ok(PtyContextResult {
        assembled_prompt: assembled,
        system_prompt,
        context_mode: meta.mode,
        context_length: meta.length,
        sections: meta.sections,
    })
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyContextResult {
    pub assembled_prompt: String,
    pub system_prompt: Option<String>,
    pub context_mode: String,
    pub context_length: usize,
    pub sections: Vec<String>,
}

// ─── Codex JSONL parser ──────────────────────────────────────────────────────

/// Poll a Codex JSONL session file for the last assistant response + tool steps.
/// Codex format: response_item with type=message|function_call|function_call_output
#[tauri::command]
pub fn pty_poll_codex(
    jsonl_path: String,
    after_line: Option<usize>,
) -> Result<Option<PtyJsonlResult>, AppError> {
    use std::io::BufRead;

    let path = std::path::PathBuf::from(&jsonl_path);
    if !path.exists() {
        return Ok(None);
    }

    let file = std::fs::File::open(&path).map_err(|e| AppError::Agent(e.to_string()))?;
    let reader = std::io::BufReader::new(file);
    let skip = after_line.unwrap_or(0);
    let mut total_lines = 0usize;

    let mut tool_steps: Vec<PtyToolStep> = Vec::new();
    let mut pending_calls: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut final_text = String::new();
    let model: Option<String> = None; // Codex doesn't expose model in JSONL response_items
    let mut is_complete = false;

    for (idx, line) in reader.lines().enumerate() {
        total_lines = idx + 1;
        if idx < skip { continue; }
        let line = match line { Ok(l) => l, Err(_) => continue };
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }

        let value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let line_type = value["type"].as_str().unwrap_or("");

        if line_type == "response_item" {
            let payload = &value["payload"];
            let item_type = payload["type"].as_str().unwrap_or("");

            match item_type {
                "message" => {
                    let role = payload["role"].as_str().unwrap_or("");
                    if role == "assistant" {
                        if let Some(content) = payload["content"].as_array() {
                            for item in content {
                                if item["type"].as_str() == Some("output_text") {
                                    if let Some(t) = item["text"].as_str() {
                                        final_text = t.to_string();
                                    }
                                }
                            }
                        }
                    }
                }
                "function_call" => {
                    let name = payload["name"].as_str().unwrap_or("unknown").to_string();
                    let call_id = payload["call_id"].as_str().unwrap_or("").to_string();
                    let args = payload["arguments"].as_str()
                        .map(|s| s.chars().take(80).collect::<String>())
                        .unwrap_or_default();
                    let idx = tool_steps.len();
                    if !call_id.is_empty() {
                        pending_calls.insert(call_id.clone(), idx);
                    }
                    tool_steps.push(PtyToolStep {
                        step_type: "tool_use".to_string(),
                        name,
                        tool_use_id: if call_id.is_empty() { None } else { Some(call_id) },
                        input: args,
                        output: None,
                        status: "done".to_string(),
                    });
                }
                "function_call_output" => {
                    let call_id = payload["call_id"].as_str().unwrap_or("");
                    if let Some(&step_idx) = pending_calls.get(call_id) {
                        let output = payload["output"].as_str().unwrap_or("").to_string();
                        let truncated = if output.len() > 500 {
                            format!("{}…", &output[..output.floor_char_boundary(500)])
                        } else {
                            output
                        };
                        if let Some(step) = tool_steps.get_mut(step_idx) {
                            step.output = Some(truncated);
                        }
                    }
                }
                _ => {}
            }
        } else if line_type == "event_msg" {
            if value["payload"]["type"].as_str() == Some("task_complete") {
                is_complete = true;
            }
        }
    }

    if final_text.is_empty() && tool_steps.is_empty() {
        return Ok(None);
    }

    let has_text = !final_text.is_empty();
    Ok(Some(PtyJsonlResult {
        text: final_text,
        tool_uses: tool_steps.iter().filter(|s| s.step_type == "tool_use").map(|s| s.name.clone()).collect(),
        tool_steps,
        model,
        total_lines,
        is_complete: is_complete || has_text,
    }))
}

// ─── Gemini JSON parser ──────────────────────────────────────────────────────

/// Poll a Gemini session JSON file for the last assistant response + tool steps.
/// Gemini format: single JSON with messages array, each having toolCalls.
#[tauri::command]
pub fn pty_poll_gemini(
    json_path: String,
    after_message_count: Option<usize>,
) -> Result<Option<PtyJsonlResult>, AppError> {
    let path = std::path::PathBuf::from(&json_path);
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| AppError::Agent(format!("read gemini session: {}", e)))?;
    let session: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| AppError::Agent(format!("parse gemini session: {}", e)))?;

    let messages = match session["messages"].as_array() {
        Some(m) => m,
        None => return Ok(None),
    };

    let skip = after_message_count.unwrap_or(0);
    let total_messages = messages.len();
    if total_messages <= skip {
        return Ok(None);
    }

    let mut tool_steps: Vec<PtyToolStep> = Vec::new();
    let mut final_text = String::new();
    let mut model: Option<String> = None;

    for msg in messages.iter().skip(skip) {
        let msg_type = msg["type"].as_str().unwrap_or("");
        if msg_type != "gemini" { continue; }

        if model.is_none() {
            model = msg["model"].as_str().map(|s| s.to_string());
        }

        // Extract thinking
        if let Some(thoughts) = msg["thoughts"].as_str() {
            if !thoughts.is_empty() {
                let summary = if thoughts.len() > 120 {
                    format!("{}...", &thoughts[..thoughts.floor_char_boundary(120)])
                } else {
                    thoughts.to_string()
                };
                tool_steps.push(PtyToolStep {
                    step_type: "thinking".to_string(),
                    name: "thinking".to_string(),
                    tool_use_id: None,
                    input: summary,
                    output: None,
                    status: "done".to_string(),
                });
            }
        }

        // Extract tool calls
        if let Some(tool_calls) = msg["toolCalls"].as_array() {
            for tc in tool_calls {
                let name = tc["name"].as_str().unwrap_or("unknown").to_string();
                let input_summary = {
                    let input = &tc["input"];
                    // Try common field names for summary
                    if let Some(v) = input["file_path"].as_str().or(input["path"].as_str()) {
                        shorten_path(v)
                    } else if let Some(v) = input["command"].as_str() {
                        v.chars().take(60).collect()
                    } else if let Some(v) = input["query"].as_str().or(input["pattern"].as_str()) {
                        v.to_string()
                    } else {
                        String::new()
                    }
                };
                let output = tc["output"].as_str().map(|s| {
                    if s.len() > 500 { format!("{}…", &s[..s.floor_char_boundary(500)]) }
                    else { s.to_string() }
                });
                tool_steps.push(PtyToolStep {
                    step_type: "tool_use".to_string(),
                    name,
                    tool_use_id: None,
                    input: input_summary,
                    output,
                    status: "done".to_string(),
                });
            }
        }

        // Extract text content
        if let Some(text) = msg["content"].as_str() {
            if !text.is_empty() {
                final_text = text.to_string();
            }
        }
    }

    if final_text.is_empty() && tool_steps.is_empty() {
        return Ok(None);
    }

    let has_text = !final_text.is_empty();
    Ok(Some(PtyJsonlResult {
        text: final_text,
        tool_uses: tool_steps.iter().filter(|s| s.step_type == "tool_use").map(|s| s.name.clone()).collect(),
        tool_steps,
        model,
        total_lines: total_messages,
        is_complete: has_text,
    }))
}

/// List Codex JSONL session files. Codex stores sessions globally by date.
#[tauri::command]
pub fn pty_list_codex_files(
    project_path: String,
) -> Result<Vec<String>, AppError> {
    // Codex stores sessions at ~/.codex/sessions/{y}/{m}/{d}/*.jsonl
    // Filter by cwd matching project_path in session_meta
    let codex_dir = dirs::home_dir()
        .ok_or_else(|| AppError::Agent("no home dir".into()))?
        .join(".codex/sessions");

    if !codex_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    // Walk date directories
    for year_entry in std::fs::read_dir(&codex_dir).map_err(|e| AppError::Agent(e.to_string()))? {
        let year_entry = year_entry.map_err(|e| AppError::Agent(e.to_string()))?;
        if !year_entry.path().is_dir() { continue; }
        for month_entry in std::fs::read_dir(year_entry.path()).into_iter().flatten().flatten() {
            if !month_entry.path().is_dir() { continue; }
            for day_entry in std::fs::read_dir(month_entry.path()).into_iter().flatten().flatten() {
                if !day_entry.path().is_dir() { continue; }
                for file_entry in std::fs::read_dir(day_entry.path()).into_iter().flatten().flatten() {
                    let path = file_entry.path();
                    if path.extension().map_or(false, |e| e == "jsonl") {
                        // Quick check: read first line for session_meta cwd
                        if let Ok(first_line) = std::fs::read_to_string(&path).map(|s| s.lines().next().unwrap_or("").to_string()) {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&first_line) {
                                let cwd = v["payload"]["cwd"].as_str().unwrap_or("");
                                if cwd.contains(&project_path) || project_path.contains(cwd) {
                                    files.push(path.to_string_lossy().to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(files)
}

/// List Gemini session JSON files for a project.
#[tauri::command]
pub fn pty_list_gemini_files(
    project_path: String,
) -> Result<Vec<String>, AppError> {
    // Gemini stores sessions at ~/.gemini/tmp/{project-name}/chats/session-*.json
    // Project name = last component of project_path
    let project_name = std::path::Path::new(&project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let gemini_dir = dirs::home_dir()
        .ok_or_else(|| AppError::Agent("no home dir".into()))?
        .join(".gemini/tmp")
        .join(&project_name)
        .join("chats");

    if !gemini_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in std::fs::read_dir(&gemini_dir).map_err(|e| AppError::Agent(e.to_string()))? {
        let entry = entry.map_err(|e| AppError::Agent(e.to_string()))?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "json") {
            files.push(path.to_string_lossy().to_string());
        }
    }
    Ok(files)
}
