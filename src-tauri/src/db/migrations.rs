use rusqlite::{Connection, params};
use crate::errors::AppError;
use super::schema;

/// Check whether a column already exists on a table (PRAGMA table_info).
/// Returns true if the column is present, false otherwise.
fn column_exists(conn: &Connection, table: &str, column: &str) -> bool {
    let sql = format!("PRAGMA table_info({})", table);
    let Ok(mut stmt) = conn.prepare(&sql) else { return false };
    stmt.query_map([], |row| row.get::<_, String>(1))
        .map(|rows| rows.filter_map(|r| r.ok()).any(|name| name == column))
        .unwrap_or(false)
}

/// Idempotent ADD COLUMN: skips if column already exists, propagates real errors.
fn add_column_if_missing(conn: &Connection, table: &str, column: &str, col_def: &str) -> Result<(), AppError> {
    if column_exists(conn, table, column) {
        return Ok(());
    }
    let sql = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, col_def);
    conn.execute(&sql, [])?;
    Ok(())
}

pub fn run(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(schema::CREATE_SCHEMA_VERSION)?;
    let current = current_version(conn)?;
    if current < 1 {
        apply_v1(conn)?;
    }
    if current < 2 {
        apply_v2(conn)?;
    }
    if current < 3 {
        apply_v3(conn)?;
    }
    if current < 4 {
        apply_v4(conn)?;
    }
    if current < 5 {
        apply_v5(conn)?;
    }
    if current < 6 {
        apply_v6(conn)?;
    }
    if current < 7 {
        apply_v7(conn)?;
    }
    if current < 8 {
        apply_v8(conn)?;
    }
    if current < 9 {
        apply_v9(conn)?;
    }
    if current < 10 {
        apply_v10(conn)?;
    }
    if current < 11 {
        apply_v11(conn)?;
    }
    if current < 12 {
        apply_v12(conn)?;
    }
    if current < 13 {
        apply_v13(conn)?;
    }
    if current < 14 {
        apply_v14(conn)?;
    }
    if current < 15 {
        apply_v15(conn)?;
    }
    if current < 16 {
        apply_v16(conn)?;
    }
    if current < 17 {
        apply_v17(conn)?;
    }
    if current < 18 {
        apply_v18(conn)?;
    }
    if current < 19 {
        apply_v19(conn)?;
    }
    if current < 20 {
        apply_v20(conn)?;
    }
    Ok(())
}

fn current_version(conn: &Connection) -> Result<i64, AppError> {
    let v: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    Ok(v)
}

fn apply_v1(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(schema::V1_SCHEMA)?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (1, ?1)",
        [now_epoch()],
    )?;
    Ok(())
}

fn apply_v2(conn: &Connection) -> Result<(), AppError> {
    // V2 adds resume_token columns — idempotent to survive partial prior runs
    add_column_if_missing(conn, "conversations", "resume_token", "TEXT")?;
    add_column_if_missing(conn, "conversations", "resume_token_engine", "TEXT")?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (2, ?1)",
        [now_epoch()],
    )?;
    Ok(())
}

fn apply_v3(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(schema::V3_SCHEMA)?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (3, ?1)",
        [now_epoch()],
    )?;
    Ok(())
}

fn apply_v4(conn: &Connection) -> Result<(), AppError> {
    // V4 adds subtask_id to artifacts — idempotent to survive partial prior runs
    add_column_if_missing(
        conn, "artifacts", "subtask_id",
        "TEXT REFERENCES plan_subtasks(id) ON DELETE SET NULL",
    )?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_artifacts_subtask_id ON artifacts(subtask_id);",
    )?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (4, ?1)",
        [now_epoch()],
    )?;
    Ok(())
}

fn apply_v5(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(schema::V5_SCHEMA)?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (5, ?1)",
        [now_epoch()],
    )?;
    Ok(())
}

fn apply_v6(conn: &Connection) -> Result<(), AppError> {
    // V6 extends trace_log with OTel span columns — idempotent per column
    add_column_if_missing(conn, "trace_log", "trace_id", "TEXT")?;
    add_column_if_missing(conn, "trace_log", "span_id", "TEXT")?;
    add_column_if_missing(conn, "trace_log", "parent_span_id", "TEXT")?;
    add_column_if_missing(conn, "trace_log", "operation", "TEXT")?;
    add_column_if_missing(conn, "trace_log", "engine", "TEXT")?;
    add_column_if_missing(conn, "trace_log", "duration_ms", "INTEGER")?;
    add_column_if_missing(conn, "trace_log", "status", "TEXT DEFAULT 'ok'")?;
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_trace_log_trace_id ON trace_log(trace_id);",
    )?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (6, ?1)",
        [now_epoch()],
    )?;
    Ok(())
}

fn apply_v7(conn: &Connection) -> Result<(), AppError> {
    add_column_if_missing(conn, "plan_subtasks", "owner_agent", "TEXT")?;
    add_column_if_missing(conn, "plan_subtasks", "last_updated_by", "TEXT")?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (7, ?1)",
        [now_epoch()],
    )?;
    Ok(())
}

fn apply_v8(conn: &Connection) -> Result<(), AppError> {
    add_column_if_missing(conn, "branches", "mode", "TEXT DEFAULT 'chat'")?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (8, ?1)",
        [now_epoch()],
    )?;
    Ok(())
}

fn apply_v9(conn: &Connection) -> Result<(), AppError> {
    add_column_if_missing(conn, "branches", "subtask_id", "TEXT REFERENCES plan_subtasks(id) ON DELETE SET NULL")?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (9, ?1)",
        [now_epoch()],
    )?;
    Ok(())
}

fn apply_v12(conn: &Connection) -> Result<(), AppError> {
    add_column_if_missing(conn, "conversations", "rt_config", "TEXT")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (12, ?1)", [now_epoch()])?;
    Ok(())
}

fn apply_v11(conn: &Connection) -> Result<(), AppError> {
    add_column_if_missing(conn, "trace_log", "context_mode", "TEXT")?;
    add_column_if_missing(conn, "trace_log", "context_sections", "TEXT")?;
    add_column_if_missing(conn, "trace_log", "context_length", "INTEGER")?;
    add_column_if_missing(conn, "trace_log", "context_hash", "TEXT")?;
    add_column_if_missing(conn, "trace_log", "context_truncated", "INTEGER DEFAULT 0")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (11, ?1)", [now_epoch()])?;
    Ok(())
}

fn apply_v10(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(schema::V10_SCHEMA)?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (10, ?1)",
        [now_epoch()],
    )?;
    Ok(())
}

fn apply_v13(conn: &Connection) -> Result<(), AppError> {
    add_column_if_missing(conn, "projects", "hidden", "INTEGER NOT NULL DEFAULT 0")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (13, ?1)", [now_epoch()])?;
    Ok(())
}

/// Fix branches with shadow conversation IDs (branch:xxx) as conversation_id.
/// These should point to the root conversation instead.
fn apply_v14(conn: &Connection) -> Result<(), AppError> {
    // Step 1: Fix conversation_id for branches pointing to shadow convs.
    // For each such branch, walk up the conversations.parent_id chain to find root.
    let mut stmt = conn.prepare(
        "SELECT b.id, b.conversation_id FROM branches b WHERE b.conversation_id LIKE 'branch:%'"
    )?;
    let broken: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    for (branch_id, shadow_conv_id) in &broken {
        // Walk parent_id chain to find root conversation
        let mut current = shadow_conv_id.clone();
        for _ in 0..10 { // max depth guard
            let parent: Option<String> = conn
                .query_row("SELECT parent_id FROM conversations WHERE id = ?1", [&current], |row| row.get(0))
                .ok()
                .flatten();
            match parent {
                Some(p) if p.starts_with("branch:") => current = p,
                Some(p) => { current = p; break; }
                None => break,
            }
        }
        if !current.starts_with("branch:") {
            conn.execute("UPDATE branches SET conversation_id = ?1 WHERE id = ?2", params![current, branch_id])?;
        }
    }

    // Step 2: Best-effort parent_branch_id backfill.
    // For branches that were created from a shadow conv (now fixed),
    // if checkpoint_id exists in messages of a shadow conv, set parent_branch_id.
    let mut stmt2 = conn.prepare(
        "SELECT b.id, b.checkpoint_id FROM branches b
         WHERE b.parent_branch_id IS NULL AND b.checkpoint_id IS NOT NULL"
    )?;
    let candidates: Vec<(String, String)> = stmt2
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
        .filter_map(|r| r.ok())
        .collect();

    for (branch_id, checkpoint_id) in &candidates {
        // Find which shadow conversation contains this checkpoint message
        let parent: Option<String> = conn
            .query_row(
                "SELECT REPLACE(m.conversation_id, 'branch:', '') FROM messages m
                 WHERE m.id = ?1 AND m.conversation_id LIKE 'branch:%'",
                [checkpoint_id],
                |row| row.get(0),
            )
            .ok();
        if let Some(parent_branch_id) = parent {
            // Verify this parent branch actually exists
            let exists: bool = conn
                .query_row("SELECT COUNT(*) FROM branches WHERE id = ?1", [&parent_branch_id], |row| row.get::<_, i64>(0))
                .unwrap_or(0) > 0;
            if exists {
                conn.execute("UPDATE branches SET parent_branch_id = ?1 WHERE id = ?2", params![parent_branch_id, branch_id])?;
            }
        }
    }

    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (14, ?1)", [now_epoch()])?;
    Ok(())
}

/// Token/cost usage_status for trace_log + conversations
fn apply_v16(conn: &Connection) -> Result<(), AppError> {
    // trace_log: distinguish exact / unavailable / unknown
    add_column_if_missing(conn, "trace_log", "usage_status", "TEXT DEFAULT 'exact'")?;
    // conversations: same distinction for aggregated totals
    add_column_if_missing(conn, "conversations", "usage_status", "TEXT DEFAULT 'exact'")?;

    // Backfill: engines that don't provide usage data → 'unavailable'
    conn.execute_batch("
        UPDATE trace_log SET usage_status = 'unavailable'
        WHERE (engine = 'opencode' OR (engine = 'gemini' AND input_tokens = 0 AND output_tokens = 0))
          AND usage_status = 'exact';
    ")?;

    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (16, ?1)", [now_epoch()])?;
    Ok(())
}

/// FTS5 triggers for messages_fts + initial population
fn apply_v15(conn: &Connection) -> Result<(), AppError> {
    // Create triggers for keeping FTS in sync
    conn.execute_batch("
        CREATE TRIGGER IF NOT EXISTS messages_fts_insert AFTER INSERT ON messages BEGIN
            INSERT INTO messages_fts(rowid, content) VALUES (NEW.rowid, NEW.content);
        END;

        CREATE TRIGGER IF NOT EXISTS messages_fts_delete AFTER DELETE ON messages BEGIN
            INSERT INTO messages_fts(messages_fts, rowid, content) VALUES('delete', OLD.rowid, OLD.content);
        END;

        CREATE TRIGGER IF NOT EXISTS messages_fts_update AFTER UPDATE OF content ON messages BEGIN
            INSERT INTO messages_fts(messages_fts, rowid, content) VALUES('delete', OLD.rowid, OLD.content);
            INSERT INTO messages_fts(rowid, content) VALUES (NEW.rowid, NEW.content);
        END;
    ")?;

    // Populate FTS with existing messages
    conn.execute_batch("INSERT INTO messages_fts(rowid, content) SELECT rowid, content FROM messages;")?;

    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (15, ?1)", [now_epoch()])?;
    Ok(())
}

fn apply_v17(conn: &Connection) -> Result<(), AppError> {
    // Compressed conversation memory — structured summaries of older messages.
    // Stored per conversation/branch, regenerated as conversations grow.
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS conversation_memory (
            id            TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            summary       TEXT NOT NULL,
            source_count  INTEGER NOT NULL DEFAULT 0,
            created_at    INTEGER NOT NULL,
            updated_at    INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id)
        );
        CREATE INDEX IF NOT EXISTS idx_conv_memory_conv ON conversation_memory(conversation_id);
    ")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (17, ?1)", [now_epoch()])?;
    Ok(())
}

/// Workflow pipeline Phase A: plan phases, events, engine assignment
fn apply_v18(conn: &Connection) -> Result<(), AppError> {
    // Extend plans table with orchestration columns
    add_column_if_missing(conn, "plans", "phase", "TEXT NOT NULL DEFAULT 'drafting'")?;
    add_column_if_missing(conn, "plans", "architect_engine", "TEXT")?;
    add_column_if_missing(conn, "plans", "developer_engine", "TEXT")?;
    add_column_if_missing(conn, "plans", "reviewer_engines", "TEXT")?;
    add_column_if_missing(conn, "plans", "implementation_branch_id", "TEXT REFERENCES branches(id)")?;
    add_column_if_missing(conn, "plans", "review_branch_id", "TEXT REFERENCES branches(id)")?;

    // Plan events — history log for phase transitions
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS plan_events (
            id            TEXT PRIMARY KEY,
            plan_id       TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
            event_type    TEXT NOT NULL,
            actor         TEXT,
            detail        TEXT,
            created_at    INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_plan_events_plan_id ON plan_events(plan_id);
    ")?;

    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (18, ?1)", [now_epoch()])?;
    Ok(())
}

/// Semantic versioning for plans: revision → version_major + version_minor
fn apply_v20(conn: &Connection) -> Result<(), AppError> {
    add_column_if_missing(conn, "plans", "version_major", "INTEGER NOT NULL DEFAULT 1")?;
    add_column_if_missing(conn, "plans", "version_minor", "INTEGER NOT NULL DEFAULT 0")?;
    // Migrate existing revision to version_major (revision 0 → v1.0, revision N → v1.N)
    conn.execute("UPDATE plans SET version_minor = revision WHERE revision > 0", [])?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (20, ?1)", [now_epoch()])?;
    Ok(())
}

/// Plan revision counter — tracks how many times subtasks have been replaced/merged
fn apply_v19(conn: &Connection) -> Result<(), AppError> {
    add_column_if_missing(conn, "plans", "revision", "INTEGER NOT NULL DEFAULT 0")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (19, ?1)", [now_epoch()])?;
    Ok(())
}

/// Seconds since Unix epoch
pub fn now_epoch() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Milliseconds since Unix epoch (for Message.timestamp)
pub fn now_epoch_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
