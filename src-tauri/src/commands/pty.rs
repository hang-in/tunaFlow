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
