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

    // Spawn reader thread — emits pty:output events
    let sid = session_id;
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let raw = &buf[..n];
                    let text = String::from_utf8_lossy(raw).to_string();

                    // Raw output → xterm.js (Terminal tab debug view)
                    let _ = app.emit(
                        "pty:output",
                        PtyOutputPayload {
                            session_id: sid,
                            data: text,
                        },
                    );

                    // ANSI-stripped + TUI-chrome-filtered text → Chat message streaming
                    let stripped_bytes = strip_ansi_escapes::strip(raw);
                    let stripped = String::from_utf8_lossy(&stripped_bytes)
                        .replace('\r', "");
                    // Filter out TUI chrome: box-drawing, UI hints, progress spinners
                    let filtered: String = stripped.lines()
                        .filter(|line| {
                            let t = line.trim();
                            if t.is_empty() { return true; }
                            // Box-drawing borders
                            if t.chars().all(|c| "━╭╮╰╯│─┌┐└┘├┤┬┴┼╶╴╷╵─ ".contains(c)) { return false; }
                            // UI hint lines (ctrl+X to Y)
                            if t.contains("ctrl+") && t.contains("to") { return false; }
                            // Progress spinners / cursor movement artifacts
                            if t.len() <= 2 && !t.chars().next().unwrap_or(' ').is_alphanumeric() { return false; }
                            true
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !filtered.trim().is_empty() {
                        let _ = app.emit(
                            "pty:text",
                            PtyOutputPayload {
                                session_id: sid,
                                data: filtered,
                            },
                        );
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
    let active_ids: Vec<u32> = sessions.keys().copied().collect();
    eprintln!("[pty] write to session {}, active sessions: {:?}", session_id, active_ids);
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| AppError::NotFound(format!("PTY session {} not found (active: {:?})", session_id, active_ids)))?;
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
    state: State<'_, PtyState>,
) -> Result<(), AppError> {
    // portable-pty resize requires the master, but we only stored writer/reader.
    // For now, log and skip — resize support requires storing the master handle.
    eprintln!("[pty] resize session {} to {}x{} (not yet implemented)", session_id, cols, rows);
    Ok(())
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
