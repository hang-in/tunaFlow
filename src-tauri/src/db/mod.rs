pub mod migrations;
pub mod models;
pub mod schema;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use crate::errors::AppError;

/// Tighten file permissions to owner-only (0600) on Unix. No-op on other OSes.
/// Missing files are silently skipped. Logs but does not fail on permission
/// errors — we'd rather run with loose perms than refuse to start.
#[cfg(unix)]
fn chmod_owner_only(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if !path.exists() { return; }
    let perms = std::fs::Permissions::from_mode(0o600);
    if let Err(e) = std::fs::set_permissions(path, perms) {
        eprintln!("[db-sec] chmod 600 failed for {:?}: {}", path, e);
    }
}

#[cfg(not(unix))]
fn chmod_owner_only(_path: &Path) {
    // Windows: file ACLs are user-scoped by default under the user profile;
    // we don't attempt to further restrict here. Document in threat model.
}

/// Apply 0600 perms to the primary DB file + its WAL/SHM/bak siblings. Called
/// on app init (catches legacy 644 files) and after backup copy (catches new
/// `.bak` which otherwise inherits umask default).
pub(crate) fn secure_db_files(db_path: &Path) {
    chmod_owner_only(db_path);
    for ext in ["db-wal", "db-shm", "db.bak"] {
        chmod_owner_only(&db_path.with_extension(ext));
    }
    // Also tighten the parent directory to 0700 so other OS users cannot list
    // contents. Only in dev (we control `~/.tunaflow/`); release uses
    // OS-managed `Application Support/<bundle-id>/` which is already user-scoped.
    #[cfg(unix)]
    if let Some(parent) = db_path.parent() {
        use std::os::unix::fs::PermissionsExt;
        if parent.exists() {
            if let Ok(meta) = std::fs::metadata(parent) {
                let mut perms = meta.permissions();
                perms.set_mode(0o700);
                if let Err(e) = std::fs::set_permissions(parent, perms) {
                    eprintln!("[db-sec] chmod 700 failed for dir {:?}: {}", parent, e);
                }
            }
        }
        // Also the grandparent (~/.tunaflow/) which holds `db/`, `skills/`, etc.
        if let Some(root) = parent.parent() {
            let is_tunaflow_root = root.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == ".tunaflow")
                .unwrap_or(false);
            if is_tunaflow_root && root.exists() {
                if let Ok(meta) = std::fs::metadata(root) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o700);
                    let _ = std::fs::set_permissions(root, perms);
                }
            }
        }
    }
}

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
    // Register sqlite-vec as auto-extension (process-global, before any Connection::open)
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }

    // Back up the database before running migrations (best-effort, non-fatal).
    // This creates <db>.bak so users can recover from a bad migration.
    if db_path.exists() {
        let bak_path = db_path.with_extension("db.bak");
        if let Err(e) = std::fs::copy(&db_path, &bak_path) {
            eprintln!("[db] backup failed (non-fatal): {}", e);
        }
    }

    // Tighten perms on legacy-mode (0644) files before opening. Covers DB + WAL + SHM + bak.
    secure_db_files(&db_path);

    // Write connection — runs migrations, enables WAL + foreign keys.
    // v45 부터 migrations::run 은 `&mut Connection` 을 요구 (transaction API 사용).
    let mut write_conn = Connection::open(&db_path)?;
    write_conn.execute_batch("PRAGMA journal_mode = WAL;")?;
    write_conn.execute_batch("PRAGMA synchronous = NORMAL;")?;
    write_conn.execute_batch("PRAGMA busy_timeout = 5000;")?;
    write_conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrations::run(&mut write_conn)?;

    // Read connection — separate handle, read-only pragmas
    let read_conn = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    read_conn.execute_batch("PRAGMA busy_timeout = 5000;")?;
    read_conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    // Re-tighten perms after WAL file creation (opening in WAL mode creates the
    // `-wal` and `-shm` siblings with umask default, usually 0644).
    secure_db_files(&db_path);

    Ok((write_conn, read_conn))
}
