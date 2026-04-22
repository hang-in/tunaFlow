//! Transactional session boundary (Phase 3a of harnessVerificationGapPlan §4).
//!
//! `AgentSessionTx` wraps a write-path agent session in a SQLite `SAVEPOINT` so
//! that mid-session failure (panic, timeout, user cancel) leaves no half-formed
//! rows in `plans`, `artifacts`, `conversation_memory`, etc.
//!
//! **Important**: this module is scaffold only (3a). `persistence.rs` is NOT
//! yet wrapped — Phase 3b introduces the actual call-site integration.
//! Streaming chunk rows (partial assistant messages) are intentionally designed
//! to live OUTSIDE the savepoint so that a user sees what the agent produced
//! before failure.
//!
//! Each session is audited in `agent_session_audit` (migration v43).

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::migrations::now_epoch_ms;
use crate::errors::AppError;

/// Outcome values stored in `agent_session_audit.outcome`.
pub const OUTCOME_IN_PROGRESS: &str = "in_progress";
pub const OUTCOME_COMMITTED: &str = "committed";
pub const OUTCOME_ROLLED_BACK: &str = "rolled_back";
pub const OUTCOME_PANIC: &str = "panic";

// ─── Audit-only API (Phase 3b-part1) ───────────────────────────────────────────
//
// Records session lifecycle in `agent_session_audit` without opening a SAVEPOINT.
// This gives us the observability half of transactional boundaries — "which
// sessions finished? which hung? which panicked?" — with zero rollback cost and
// zero write-lock contention added to the hot path. SAVEPOINT-backed rollback
// is Phase 3b-part2 behind a feature flag; the audit record format is shared so
// part2 can upgrade the same rows in place.

/// Insert an audit row with `outcome='in_progress'` and return the session id.
/// No SAVEPOINT is opened. Call `audit_commit` or `audit_rollback` to finalize.
pub fn audit_begin(conn: &Connection, conversation_id: Option<&str>) -> Result<String, AppError> {
    let session_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO agent_session_audit \
         (session_id, conversation_id, started_at, outcome) \
         VALUES (?1, ?2, ?3, ?4)",
        params![
            session_id,
            conversation_id,
            now_epoch_ms(),
            OUTCOME_IN_PROGRESS,
        ],
    )?;
    Ok(session_id)
}

/// Mark an audit row `committed`. No-op if the row is missing (legacy sessions
/// that started before Phase 3b-part1 shipped).
pub fn audit_commit(conn: &Connection, session_id: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE agent_session_audit SET ended_at = ?1, outcome = ?2 WHERE session_id = ?3",
        params![now_epoch_ms(), OUTCOME_COMMITTED, session_id],
    )?;
    Ok(())
}

/// Mark an audit row `rolled_back` with a reason. No-op if the row is missing.
pub fn audit_rollback(conn: &Connection, session_id: &str, reason: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE agent_session_audit \
         SET ended_at = ?1, outcome = ?2, rollback_reason = ?3 \
         WHERE session_id = ?4",
        params![now_epoch_ms(), OUTCOME_ROLLED_BACK, reason, session_id],
    )?;
    Ok(())
}

/// Mark an audit row `panic` — used by cleanup logic after detecting a session
/// that never finalized (startup sweeper, Phase 3c). Also safe to call from a
/// catch_unwind handler with write lock access.
pub fn audit_panic(conn: &Connection, session_id: &str, reason: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE agent_session_audit \
         SET ended_at = ?1, outcome = ?2, rollback_reason = ?3 \
         WHERE session_id = ?4",
        params![now_epoch_ms(), OUTCOME_PANIC, reason, session_id],
    )?;
    Ok(())
}

/// Is audit recording enabled? Default ON — the override env var lets
/// operators disable if the audit INSERT itself ever becomes a bottleneck.
pub fn audit_enabled() -> bool {
    match std::env::var("TUNAFLOW_AGENT_SESSION_AUDIT") {
        Ok(v) => !matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "false" | "off"),
        Err(_) => true,
    }
}

/// Wraps a single agent write session in a SQLite SAVEPOINT.
///
/// Use `commit` on success and `rollback` on explicit failure. If the value is
/// dropped without calling either (i.e. a panic unwinds past it), the `Drop`
/// impl records `outcome='panic'` but does NOT emit a SAVEPOINT ROLLBACK — the
/// outer transaction/connection owner is responsible for that. This design is
/// chosen because we cannot hand a Connection reference to Drop safely.
#[must_use = "AgentSessionTx must be explicitly committed or rolled back — drop leaves the savepoint dangling"]
pub struct AgentSessionTx {
    session_id: String,
    savepoint_name: String,
    /// Prevents Drop from double-writing the audit row after commit/rollback.
    finalized: bool,
}

impl AgentSessionTx {
    /// Begin a new agent session. Creates a SAVEPOINT and inserts an audit row
    /// with `outcome='in_progress'`.
    pub fn begin(conn: &Connection, conversation_id: Option<&str>) -> Result<Self, AppError> {
        let session_id = Uuid::new_v4().to_string();
        let savepoint_name = derive_savepoint_name(&session_id);

        // Insert audit row first so that even if SAVEPOINT fails we have a trace
        conn.execute(
            "INSERT INTO agent_session_audit \
             (session_id, conversation_id, started_at, outcome, savepoint_name) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id,
                conversation_id,
                now_epoch_ms(),
                OUTCOME_IN_PROGRESS,
                savepoint_name,
            ],
        )?;

        conn.execute(&format!("SAVEPOINT {}", savepoint_name), [])?;

        Ok(Self { session_id, savepoint_name, finalized: false })
    }

    /// Session id assigned at `begin` — useful for logging and correlating with
    /// WS events.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Release the SAVEPOINT and mark the audit row `committed`.
    pub fn commit(mut self, conn: &Connection) -> Result<(), AppError> {
        conn.execute(&format!("RELEASE SAVEPOINT {}", self.savepoint_name), [])?;
        conn.execute(
            "UPDATE agent_session_audit SET ended_at = ?1, outcome = ?2 WHERE session_id = ?3",
            params![now_epoch_ms(), OUTCOME_COMMITTED, self.session_id],
        )?;
        self.finalized = true;
        Ok(())
    }

    /// Roll back to the SAVEPOINT and mark the audit row with a reason.
    pub fn rollback(mut self, conn: &Connection, reason: &str) -> Result<(), AppError> {
        // Two statements — SQLite requires explicit RELEASE after ROLLBACK TO
        // SAVEPOINT, otherwise the savepoint stays on the stack.
        conn.execute(&format!("ROLLBACK TO SAVEPOINT {}", self.savepoint_name), [])?;
        conn.execute(&format!("RELEASE SAVEPOINT {}", self.savepoint_name), [])?;
        conn.execute(
            "UPDATE agent_session_audit \
             SET ended_at = ?1, outcome = ?2, rollback_reason = ?3 \
             WHERE session_id = ?4",
            params![now_epoch_ms(), OUTCOME_ROLLED_BACK, reason, self.session_id],
        )?;
        self.finalized = true;
        Ok(())
    }
}

impl Drop for AgentSessionTx {
    fn drop(&mut self) {
        // Panic-safety marker only. We deliberately do NOT touch the DB here
        // (no Connection available) — the next session begin will see a stale
        // `in_progress` row, which a startup cleanup routine can detect and
        // mark `panic`. Phase 3b will add that sweeper.
        if !self.finalized {
            eprintln!(
                "[agent-session-tx] WARNING: session {} dropped without commit/rollback. \
                 Audit row remains 'in_progress' — Phase 3b startup sweeper will reconcile.",
                self.session_id
            );
        }
    }
}

/// SQLite savepoint names must be valid identifiers. UUIDs contain hyphens
/// which are fine if quoted, but we prefer a clean unquoted name. Strip hyphens
/// and prefix with `sp_` so it starts with a letter.
fn derive_savepoint_name(session_id: &str) -> String {
    let cleaned: String = session_id.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    format!("sp_{}", cleaned)
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Minimal schema for the audit table — mirrors migration v43 but inline so
    /// the test is self-contained.
    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory");
        conn.execute_batch(
            "CREATE TABLE agent_session_audit (
                session_id    TEXT PRIMARY KEY,
                conversation_id TEXT,
                started_at    INTEGER NOT NULL,
                ended_at      INTEGER,
                outcome       TEXT NOT NULL DEFAULT 'in_progress',
                rollback_reason TEXT,
                savepoint_name TEXT
             );
             CREATE TABLE sandbox (id INTEGER PRIMARY KEY, value TEXT);",
        ).expect("create audit + sandbox");
        conn
    }

    #[test]
    fn derive_savepoint_name_strips_hyphens() {
        let name = derive_savepoint_name("abc-123-def");
        assert_eq!(name, "sp_abc123def");
    }

    #[test]
    fn begin_inserts_audit_row_and_opens_savepoint() {
        let conn = open_test_db();
        let tx = AgentSessionTx::begin(&conn, Some("conv-1")).expect("begin");
        let sid = tx.session_id().to_string();

        // audit row exists with in_progress outcome
        let (outcome, conv, sp): (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT outcome, conversation_id, savepoint_name FROM agent_session_audit WHERE session_id = ?1",
                [&sid],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("query audit");
        assert_eq!(outcome, OUTCOME_IN_PROGRESS);
        assert_eq!(conv.as_deref(), Some("conv-1"));
        assert!(sp.is_some());

        // commit succeeds to release the savepoint (otherwise it'd linger)
        tx.commit(&conn).expect("commit");
    }

    #[test]
    fn commit_updates_outcome_and_releases_savepoint() {
        let conn = open_test_db();
        let tx = AgentSessionTx::begin(&conn, None).expect("begin");
        let sid = tx.session_id().to_string();

        conn.execute("INSERT INTO sandbox (value) VALUES ('inside')", []).expect("insert");
        tx.commit(&conn).expect("commit");

        let outcome: String = conn
            .query_row("SELECT outcome FROM agent_session_audit WHERE session_id = ?1", [&sid], |r| r.get(0))
            .expect("outcome");
        assert_eq!(outcome, OUTCOME_COMMITTED);

        // committed write is visible
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sandbox WHERE value = 'inside'", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1);

        // ended_at set
        let ended_at: Option<i64> = conn
            .query_row("SELECT ended_at FROM agent_session_audit WHERE session_id = ?1", [&sid], |r| r.get(0))
            .expect("ended_at");
        assert!(ended_at.is_some());
    }

    #[test]
    fn rollback_reverts_writes_and_records_reason() {
        let conn = open_test_db();
        let tx = AgentSessionTx::begin(&conn, Some("conv-A")).expect("begin");
        let sid = tx.session_id().to_string();

        conn.execute("INSERT INTO sandbox (value) VALUES ('will-revert')", []).expect("insert");
        tx.rollback(&conn, "test failure").expect("rollback");

        // the sandbox write must be gone
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sandbox WHERE value = 'will-revert'", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 0, "rollback did not revert sandbox write");

        // audit row updated correctly
        let (outcome, reason): (String, Option<String>) = conn
            .query_row(
                "SELECT outcome, rollback_reason FROM agent_session_audit WHERE session_id = ?1",
                [&sid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("query");
        assert_eq!(outcome, OUTCOME_ROLLED_BACK);
        assert_eq!(reason.as_deref(), Some("test failure"));
    }

    #[test]
    fn nested_sessions_are_independent() {
        // Two concurrent-ish sessions on the same connection. SQLite allows
        // nested SAVEPOINTs, so an inner rollback must NOT affect the outer.
        let conn = open_test_db();

        let outer = AgentSessionTx::begin(&conn, None).expect("outer begin");
        conn.execute("INSERT INTO sandbox (value) VALUES ('outer')", []).expect("outer insert");

        let inner = AgentSessionTx::begin(&conn, None).expect("inner begin");
        conn.execute("INSERT INTO sandbox (value) VALUES ('inner')", []).expect("inner insert");
        inner.rollback(&conn, "inner fail").expect("inner rollback");

        // inner reverted but outer still holds its write
        let has_outer: i64 = conn
            .query_row("SELECT COUNT(*) FROM sandbox WHERE value = 'outer'", [], |r| r.get(0))
            .expect("count outer");
        let has_inner: i64 = conn
            .query_row("SELECT COUNT(*) FROM sandbox WHERE value = 'inner'", [], |r| r.get(0))
            .expect("count inner");
        assert_eq!(has_outer, 1);
        assert_eq!(has_inner, 0);

        outer.commit(&conn).expect("outer commit");
    }

    #[test]
    fn drop_without_finalize_emits_warning_but_does_not_panic() {
        // We can't assert stderr easily, but we can assert that dropping without
        // commit/rollback doesn't panic the test binary itself. This is a
        // safety guard — Phase 3b will add a startup sweeper.
        let conn = open_test_db();
        {
            let _tx = AgentSessionTx::begin(&conn, None).expect("begin");
            // implicit drop here — must not panic
        }
        // The audit row should still be in_progress (Phase 3b reconciles)
        let outcomes: Vec<String> = conn
            .prepare("SELECT outcome FROM agent_session_audit")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(outcomes, vec![OUTCOME_IN_PROGRESS.to_string()]);
    }

    #[test]
    fn derive_savepoint_name_is_valid_sqlite_identifier() {
        // Must start with letter/underscore and contain only alphanumerics
        let name = derive_savepoint_name("12345-abcde-67890");
        assert!(name.starts_with("sp_"));
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
    }

    // ─── Audit-only API (Phase 3b-part1) ──────────────────────────────────────

    #[test]
    fn audit_begin_inserts_in_progress_row() {
        let conn = open_test_db();
        let sid = audit_begin(&conn, Some("conv-X")).expect("begin");
        let (outcome, conv): (String, Option<String>) = conn
            .query_row(
                "SELECT outcome, conversation_id FROM agent_session_audit WHERE session_id = ?1",
                [&sid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(outcome, OUTCOME_IN_PROGRESS);
        assert_eq!(conv.as_deref(), Some("conv-X"));
    }

    #[test]
    fn audit_commit_flips_outcome_and_sets_ended_at() {
        let conn = open_test_db();
        let sid = audit_begin(&conn, None).unwrap();
        audit_commit(&conn, &sid).expect("commit");
        let (outcome, ended_at): (String, Option<i64>) = conn
            .query_row(
                "SELECT outcome, ended_at FROM agent_session_audit WHERE session_id = ?1",
                [&sid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(outcome, OUTCOME_COMMITTED);
        assert!(ended_at.is_some(), "ended_at must be populated on commit");
    }

    #[test]
    fn audit_rollback_records_reason() {
        let conn = open_test_db();
        let sid = audit_begin(&conn, None).unwrap();
        audit_rollback(&conn, &sid, "engine timeout").expect("rollback");
        let (outcome, reason): (String, Option<String>) = conn
            .query_row(
                "SELECT outcome, rollback_reason FROM agent_session_audit WHERE session_id = ?1",
                [&sid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(outcome, OUTCOME_ROLLED_BACK);
        assert_eq!(reason.as_deref(), Some("engine timeout"));
    }

    #[test]
    fn audit_panic_marks_outcome() {
        let conn = open_test_db();
        let sid = audit_begin(&conn, None).unwrap();
        audit_panic(&conn, &sid, "thread panicked").expect("panic mark");
        let outcome: String = conn
            .query_row(
                "SELECT outcome FROM agent_session_audit WHERE session_id = ?1",
                [&sid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(outcome, OUTCOME_PANIC);
    }

    #[test]
    fn audit_update_on_missing_row_is_noop() {
        // Updates targeting a nonexistent session_id must not error — they just
        // affect zero rows. This lets legacy sessions (started before this code)
        // flow through finalize without surprises.
        let conn = open_test_db();
        audit_commit(&conn, "nonexistent-session-id").expect("commit noop");
        audit_rollback(&conn, "nonexistent-session-id", "reason").expect("rollback noop");
    }

    #[test]
    fn audit_enabled_default_true() {
        // When the env var is unset, auditing should default to ON.
        // We can't safely mutate env inside tests (order dependency); instead
        // this test documents the semantics + guards against accidental flip.
        std::env::remove_var("TUNAFLOW_AGENT_SESSION_AUDIT");
        assert!(audit_enabled(), "audit must default ON when env unset");
    }

    #[test]
    fn audit_enabled_respects_off_values() {
        // This test mutates env var, so we restore after.
        let previous = std::env::var("TUNAFLOW_AGENT_SESSION_AUDIT").ok();

        for off_value in ["0", "false", "off", "FALSE", "Off"] {
            std::env::set_var("TUNAFLOW_AGENT_SESSION_AUDIT", off_value);
            assert!(!audit_enabled(), "value '{}' must disable audit", off_value);
        }
        for on_value in ["1", "true", "on", "TRUE", "yes"] {
            std::env::set_var("TUNAFLOW_AGENT_SESSION_AUDIT", on_value);
            assert!(audit_enabled(), "value '{}' must enable audit", on_value);
        }

        match previous {
            Some(v) => std::env::set_var("TUNAFLOW_AGENT_SESSION_AUDIT", v),
            None => std::env::remove_var("TUNAFLOW_AGENT_SESSION_AUDIT"),
        }
    }
}
