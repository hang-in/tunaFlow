pub mod migrations;
pub mod models;
pub mod schema;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use crate::errors::AppError;

/// Tauri managed state: dual connections for read/write separation.
///
/// `write` — exclusive write connection (used by agent send, RT, CRUD mutations)
/// `read`  — read-only connection (used by list queries, UI reads)
///
/// Both connections share the same SQLite WAL-mode database,
/// so reads never block writes and vice versa.
///
/// Fields use `Arc<Mutex<>>` so background threads can clone the Arc
/// without lifetime issues with Tauri's State wrapper.
#[derive(Clone)]
pub struct DbState {
    pub write: Arc<Mutex<Connection>>,
    pub read: Arc<Mutex<Connection>>,
}

impl DbState {
    /// Convenience: lock the write connection (most commands use this).
    /// Existing code uses `state.0.lock()` — we provide backward compat below.
    pub fn write_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
        self.write.lock().map_err(|_| AppError::Lock)
    }

    /// Lock the read connection (for list/query commands).
    pub fn read_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
        self.read.lock().map_err(|_| AppError::Lock)
    }
}

pub fn init(db_path: PathBuf) -> Result<(Connection, Connection), AppError> {
    // Write connection — runs migrations, enables WAL + foreign keys
    let write_conn = Connection::open(&db_path)?;
    write_conn.execute_batch("PRAGMA journal_mode = WAL;")?;
    write_conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrations::run(&write_conn)?;

    // Read connection — separate handle, read-only pragmas
    let read_conn = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    read_conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    Ok((write_conn, read_conn))
}
