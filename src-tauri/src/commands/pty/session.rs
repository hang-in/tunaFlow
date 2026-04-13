//! PTY session lifecycle — spawn, write, resize, kill.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;

use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tauri::{AppHandle, Emitter, State};

use crate::errors::AppError;
use super::{PtyOutputPayload, PtyExitPayload};

/// Managed state for PTY sessions.
pub struct PtyState {
    next_id: Mutex<u32>,
    sessions: Mutex<HashMap<u32, PtySession>>,
}

pub(super) struct PtySession {
    /// Write queue sender — serializes writes to PTY stdin in FIFO order.
    pub write_tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    pub _child: Box<dyn portable_pty::Child + Send>,
    /// PTY master — kept alive for resize (SIGWINCH).
    pub master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    /// Latest VTE screen text — shared with reader thread for on-demand access.
    pub screen: Arc<Mutex<String>>,
    /// Current terminal dimensions — shared with reader thread for VTE scan.
    pub dims: Arc<Mutex<(usize, usize)>>,
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
    if let Some(ref env_map) = env {
        for (k, v) in env_map {
            cmd.env(k, v);
        }
    }

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| AppError::Agent(format!("Failed to spawn {}: {}", file, e)))?;

    drop(pair.slave);

    let raw_writer = Arc::new(Mutex::new(
        pair.master
            .take_writer()
            .map_err(|e| AppError::Agent(format!("Failed to get PTY writer: {}", e)))?,
    ));

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| AppError::Agent(format!("Failed to get PTY reader: {}", e)))?;

    let master: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>> =
        Arc::new(Mutex::new(pair.master));

    let session_id = {
        let mut id = state.next_id.lock();
        let current = *id;
        *id += 1;
        current
    };

    // Write queue — serializes writes to PTY stdin in FIFO order
    let (write_tx, mut write_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
    {
        let writer_for_queue = Arc::clone(&raw_writer);
        let write_sid = session_id;
        tauri::async_runtime::spawn(async move {
            while let Some(data) = write_rx.recv().await {
                let mut w = writer_for_queue.lock();
                // Chunk large writes to avoid blocking the PTY pipe buffer.
                // The PTY kernel buffer on macOS is ~65 KB; writing more than that at once
                // would block until the slave (Claude CLI) reads data. Chunking prevents
                // starving the write queue and ensures \r (Enter) is delivered promptly.
                const CHUNK: usize = 4096;
                let mut offset = 0;
                let mut write_ok = true;
                while offset < data.len() {
                    let end = (offset + CHUNK).min(data.len());
                    if let Err(e) = w.write_all(&data[offset..end]) {
                        eprintln!("[pty] write error for session {} (offset {}): {}", write_sid, offset, e);
                        write_ok = false;
                        break;
                    }
                    offset = end;
                }
                if write_ok {
                    let _ = w.flush();
                } else {
                    // Write failed mid-way. Log and continue — subsequent small writes
                    // (like \r) may still succeed and should not be silently dropped.
                    eprintln!("[pty] write queue continuing after partial error for session {}", write_sid);
                }
            }
            eprintln!("[pty] write queue closed for session {}", write_sid);
        });
    }

    let shared_screen = Arc::new(Mutex::new(String::new()));
    let screen_for_reader = Arc::clone(&shared_screen);

    let init_cols = cols.unwrap_or(80) as usize;
    let init_rows = rows.unwrap_or(50) as usize;
    let shared_dims = Arc::new(Mutex::new((init_cols, init_rows)));
    let dims_for_reader = Arc::clone(&shared_dims);

    {
        let mut sessions = state.sessions.lock();
        sessions.insert(
            session_id,
            PtySession {
                write_tx,
                _child: child,
                master: Arc::clone(&master),
                screen: shared_screen,
                dims: shared_dims,
            },
        );
    }

    // Spawn reader thread — emits pty:output (raw) + pty:screen events
    let sid = session_id;
    std::thread::spawn(move || {
        use alacritty_terminal::term::{Config as TermConfig, Term, test::TermSize};
        use alacritty_terminal::event::VoidListener;
        use alacritty_terminal::vte::ansi;
        use alacritty_terminal::index::{Line, Column};
        use alacritty_terminal::grid::Dimensions;

        #[derive(Default)]
        struct NoTimeout;
        impl ansi::Timeout for NoTimeout {
            fn set_timeout(&mut self, _: std::time::Duration) {}
            fn clear_timeout(&mut self) {}
            fn pending_timeout(&self) -> bool { false }
        }

        let (init_c, init_r) = *dims_for_reader.lock();
        let size = TermSize::new(init_c, init_r);
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

                    let _ = app.emit("pty:output", PtyOutputPayload {
                        session_id: sid, data: text,
                    });

                    parser.advance(&mut term, raw);

                    let (cur_cols, cur_rows) = *dims_for_reader.lock();
                    if cur_cols != term.columns() || cur_rows != term.screen_lines() {
                        term.resize(TermSize::new(cur_cols, cur_rows));
                    }

                    let grid = term.grid();
                    let mut screen_lines: Vec<String> = Vec::new();
                    for row_idx in 0..cur_rows {
                        let row = &grid[Line(row_idx as i32)];
                        let line: String = (0..cur_cols)
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

                    if screen_text != prev_screen_text && !screen_text.trim().is_empty() {
                        prev_screen_text = screen_text.clone();
                        *screen_for_reader.lock() = screen_text.clone();
                        let _ = app.emit("pty:screen", PtyOutputPayload {
                            session_id: sid, data: screen_text,
                        });
                    }
                }
                Err(e) => {
                    eprintln!("[pty] read error for session {}: {}", sid, e);
                    break;
                }
            }
        }

        let _ = app.emit(
            "pty:exit",
            PtyExitPayload {
                session_id: sid,
                exit_code: None,
            },
        );
    });

    let active_count = state.sessions.lock().len();
    eprintln!("[pty] spawned session {} — {} {:?} (cwd: {:?}), active sessions: {}", session_id, file, args, cwd, active_count);
    Ok(session_id)
}

/// Write data to a PTY session's stdin via the write queue (FIFO ordered).
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
    session.write_tx
        .send(data.into_bytes())
        .map_err(|e| AppError::Agent(format!("PTY write queue send error: {}", e)))?;
    Ok(())
}

/// Get the current VTE screen text for a PTY session (on-demand).
#[tauri::command]
pub fn pty_get_screen(
    session_id: u32,
    state: State<'_, PtyState>,
) -> Result<String, AppError> {
    let arc = {
        let sessions = state.sessions.lock();
        Arc::clone(&sessions
            .get(&session_id)
            .ok_or_else(|| AppError::NotFound(format!("PTY session {} not found", session_id)))?
            .screen)
    };
    let guard = arc.lock();
    Ok(guard.clone())
}

/// Check if a PTY session's child process is alive.
#[tauri::command]
pub fn pty_is_alive(
    session_id: u32,
    state: State<'_, PtyState>,
) -> bool {
    let sessions = state.sessions.lock();
    sessions.contains_key(&session_id)
}

/// Resize a PTY session (sends SIGWINCH to the child process).
#[tauri::command]
pub fn pty_resize(
    session_id: u32,
    cols: u16,
    rows: u16,
    state: State<'_, PtyState>,
) -> Result<(), AppError> {
    let sessions = state.sessions.lock();
    let session = sessions.get(&session_id).ok_or(AppError::NotFound("pty session not found".into()))?;

    *session.dims.lock() = (cols as usize, rows as usize);

    let new_size = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
    session.master.lock().resize(new_size)
        .map_err(|e| AppError::Agent(format!("pty resize failed: {e}")))?;

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
