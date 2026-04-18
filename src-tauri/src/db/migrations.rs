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
    if current < 21 {
        apply_v21(conn)?;
    }
    if current < 22 {
        apply_v22(conn)?;
    }
    if current < 23 {
        apply_v23(conn)?;
    }
    if current < 24 {
        apply_v24(conn)?;
    }
    if current < 25 {
        apply_v25(conn)?;
    }
    if current < 26 {
        apply_v26(conn)?;
    }
    if current < 27 {
        apply_v27(conn)?;
    }
    if current < 28 {
        apply_v28(conn)?;
    }
    if current < 29 {
        apply_v29(conn)?;
    }
    if current < 30 {
        apply_v30(conn)?;
    }
    if current < 31 {
        apply_v31(conn)?;
    }
    if current < 32 {
        apply_v32(conn)?;
    }
    if current < 33 {
        apply_v33(conn)?;
    }
    if current < 34 {
        apply_v34(conn)?;
    }
    if current < 35 {
        apply_v35(conn)?;
    }
    if current < 36 {
        apply_v36(conn)?;
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

/// Long-term memory enhancements: topic-based memory, provenance, session_links
fn apply_v21(conn: &Connection) -> Result<(), AppError> {
    // Topic-based compressed memory
    add_column_if_missing(conn, "conversation_memory", "topic", "TEXT NOT NULL DEFAULT 'general'")?;
    add_column_if_missing(conn, "conversation_memory", "phase", "TEXT")?;
    add_column_if_missing(conn, "conversation_memory", "message_range", "TEXT")?;

    // Compression provenance tracking
    add_column_if_missing(conn, "conversation_memory", "provenance", "TEXT NOT NULL DEFAULT 'auto'")?;
    add_column_if_missing(conn, "conversation_memory", "model_used", "TEXT")?;

    // Auto session discovery — persistent links between related conversations
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS session_links (
            id              TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            linked_conv_id  TEXT NOT NULL,
            score           REAL NOT NULL DEFAULT 0.0,
            method          TEXT NOT NULL DEFAULT 'fts5',
            created_at      INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
            FOREIGN KEY (linked_conv_id) REFERENCES conversations(id) ON DELETE CASCADE,
            UNIQUE(conversation_id, linked_conv_id)
        );
        CREATE INDEX IF NOT EXISTS idx_session_links_conv ON session_links(conversation_id);
    ")?;

    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (21, ?1)", [now_epoch()])?;
    Ok(())
}

/// Conversation chunks with vector embeddings for semantic search
fn apply_v22(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS conversation_chunks (
            id               TEXT PRIMARY KEY,
            project_key      TEXT NOT NULL,
            conversation_id  TEXT NOT NULL,
            kind             TEXT NOT NULL,
            root_message_id  TEXT,
            text_preview     TEXT NOT NULL,
            embedding        BLOB,
            created_at       INTEGER NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_conv_chunks_project ON conversation_chunks(project_key);
        CREATE INDEX IF NOT EXISTS idx_conv_chunks_conv ON conversation_chunks(conversation_id);
    ")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (22, ?1)", [now_epoch()])?;
    Ok(())
}

/// v23: Add message_id to trace_log for direct message↔trace linkage.
fn apply_v23(conn: &Connection) -> Result<(), AppError> {
    add_column_if_missing(conn, "trace_log", "message_id", "TEXT")?;
    conn.execute("CREATE INDEX IF NOT EXISTS idx_trace_log_message_id ON trace_log(message_id)", [])?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (23, ?1)", [now_epoch()])?;
    Ok(())
}

fn apply_v24(conn: &Connection) -> Result<(), AppError> {
    // Subtask parallel groups: depends_on (JSON array of idx) + parallel_group label
    add_column_if_missing(conn, "plan_subtasks", "depends_on", "TEXT DEFAULT '[]'")?;
    add_column_if_missing(conn, "plan_subtasks", "parallel_group", "TEXT")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (24, ?1)", [now_epoch()])?;
    Ok(())
}

fn apply_v25(conn: &Connection) -> Result<(), AppError> {
    // Follow-up plan lineage: parent_plan_id links to predecessor plan
    add_column_if_missing(conn, "plans", "parent_plan_id", "TEXT")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (25, ?1)", [now_epoch()])?;
    Ok(())
}

fn apply_v26(conn: &Connection) -> Result<(), AppError> {
    // Plan slug: unique file-path-safe identifier, prevents Korean title collisions
    add_column_if_missing(conn, "plans", "slug", "TEXT")?;
    // Backfill existing plans with slugify(title) + collision suffix
    let mut stmt = conn.prepare("SELECT id, title FROM plans WHERE slug IS NULL")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    for (id, title) in &rows {
        let base = slugify_title(title);
        let slug = find_unique_slug(conn, &base, Some(id));
        conn.execute("UPDATE plans SET slug = ?1 WHERE id = ?2", params![slug, id])?;
    }
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (26, ?1)", [now_epoch()])?;
    Ok(())
}

/// Generate ASCII slug from title (no uniqueness guarantee — caller must check)
pub fn slugify_title(title: &str) -> String {
    let base: String = title.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let trimmed = base.trim_matches('-').to_string();
    let collapsed = trimmed.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
    let truncated = if collapsed.len() > 60 {
        collapsed[..60].trim_end_matches('-').to_string()
    } else {
        collapsed
    };
    if truncated.is_empty() { "plan".to_string() } else { truncated }
}

/// Find a unique slug by appending -2, -3, etc. if collision detected
pub fn find_unique_slug(conn: &Connection, base: &str, exclude_id: Option<&str>) -> String {
    let mut candidate = base.to_string();
    let mut counter = 1;
    loop {
        let exists: bool = if let Some(eid) = exclude_id {
            conn.query_row(
                "SELECT COUNT(*) > 0 FROM plans WHERE slug = ?1 AND id != ?2",
                params![candidate, eid], |row| row.get(0),
            ).unwrap_or(false)
        } else {
            conn.query_row(
                "SELECT COUNT(*) > 0 FROM plans WHERE slug = ?1",
                params![candidate], |row| row.get(0),
            ).unwrap_or(false)
        };
        if !exists { return candidate; }
        counter += 1;
        candidate = format!("{}-{}", base, counter);
    }
}

fn apply_v28(conn: &Connection) -> Result<(), AppError> {
    // Artifacts: plan_id column for plan-based grouping
    add_column_if_missing(conn, "artifacts", "plan_id", "TEXT")?;
    // Backfill: link artifacts with subtask_id to their plan
    conn.execute_batch("
        UPDATE artifacts SET plan_id = (
            SELECT ps.plan_id FROM plan_subtasks ps WHERE ps.id = artifacts.subtask_id
        ) WHERE subtask_id IS NOT NULL AND plan_id IS NULL;
    ")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (28, ?1)", [now_epoch()])?;
    Ok(())
}

fn apply_v29(conn: &Connection) -> Result<(), AppError> {
    // Insight system: sessions, findings, reports
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS insight_sessions (
            id           TEXT PRIMARY KEY,
            project_key  TEXT NOT NULL,
            status       TEXT NOT NULL DEFAULT 'pending',
            categories   TEXT,
            test_output  TEXT,
            summary      TEXT,
            created_at   INTEGER NOT NULL,
            completed_at INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_insight_sessions_project
            ON insight_sessions(project_key);

        CREATE TABLE IF NOT EXISTS insight_findings (
            id              TEXT PRIMARY KEY,
            session_id      TEXT NOT NULL,
            project_key     TEXT NOT NULL,
            category        TEXT NOT NULL,
            severity        TEXT NOT NULL,
            fix_difficulty  TEXT NOT NULL,
            title           TEXT NOT NULL,
            description     TEXT NOT NULL,
            file_path       TEXT,
            line_number     INTEGER,
            snippet         TEXT,
            estimated_files INTEGER DEFAULT 1,
            resolution      TEXT,
            plan_id         TEXT,
            status          TEXT NOT NULL DEFAULT 'open',
            created_at      INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_insight_findings_session
            ON insight_findings(session_id);
        CREATE INDEX IF NOT EXISTS idx_insight_findings_project
            ON insight_findings(project_key);
        CREATE INDEX IF NOT EXISTS idx_insight_findings_status
            ON insight_findings(status);

        CREATE TABLE IF NOT EXISTS insight_reports (
            id          TEXT PRIMARY KEY,
            session_id  TEXT NOT NULL,
            project_key TEXT NOT NULL,
            type        TEXT NOT NULL,
            category    TEXT,
            content     TEXT NOT NULL,
            created_at  INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_insight_reports_session
            ON insight_reports(session_id);
    ")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (29, ?1)", [now_epoch()])?;
    Ok(())
}

fn apply_v27(conn: &Connection) -> Result<(), AppError> {
    // Failure learning system: stores review failures for rework prompt injection
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS failure_lessons (
            id          TEXT PRIMARY KEY,
            project_key TEXT NOT NULL,
            plan_id     TEXT,
            file_path   TEXT,
            pattern     TEXT,
            finding     TEXT NOT NULL,
            resolution  TEXT,
            created_at  INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_failure_lessons_project
            ON failure_lessons(project_key);
        CREATE INDEX IF NOT EXISTS idx_failure_lessons_plan
            ON failure_lessons(plan_id);

        CREATE VIRTUAL TABLE IF NOT EXISTS failure_lessons_fts
            USING fts5(finding, pattern, file_path, content=failure_lessons, content_rowid=rowid);

        CREATE TRIGGER IF NOT EXISTS failure_lessons_ai AFTER INSERT ON failure_lessons BEGIN
            INSERT INTO failure_lessons_fts(rowid, finding, pattern, file_path)
            VALUES (new.rowid, new.finding, COALESCE(new.pattern, ''), COALESCE(new.file_path, ''));
        END;
        CREATE TRIGGER IF NOT EXISTS failure_lessons_ad AFTER DELETE ON failure_lessons BEGIN
            INSERT INTO failure_lessons_fts(failure_lessons_fts, rowid, finding, pattern, file_path)
            VALUES ('delete', old.rowid, old.finding, COALESCE(old.pattern, ''), COALESCE(old.file_path, ''));
        END;
        CREATE TRIGGER IF NOT EXISTS failure_lessons_au AFTER UPDATE ON failure_lessons BEGIN
            INSERT INTO failure_lessons_fts(failure_lessons_fts, rowid, finding, pattern, file_path)
            VALUES ('delete', old.rowid, old.finding, COALESCE(old.pattern, ''), COALESCE(old.file_path, ''));
            INSERT INTO failure_lessons_fts(rowid, finding, pattern, file_path)
            VALUES (new.rowid, new.finding, COALESCE(new.pattern, ''), COALESCE(new.file_path, ''));
        END;
    ")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (27, ?1)", [now_epoch()])?;
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

fn apply_v30(conn: &Connection) -> Result<(), AppError> {
    // sqlite-vec: vec0 virtual table for vector search (replaces brute-force cosine)
    // Uses 384-dim float32 embeddings with cosine distance metric.
    // Linked to conversation_chunks via rowid.
    conn.execute_batch("
        CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
            embedding float[384] distance_metric=cosine
        );
    ")?;

    // Backfill: copy existing embeddings from conversation_chunks BLOB to vec0.
    // Only rows with non-NULL embedding are indexed.
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversation_chunks WHERE embedding IS NOT NULL", [], |r| r.get(0)
    ).unwrap_or(0);

    if count > 0 {
        conn.execute_batch("
            INSERT OR IGNORE INTO vec_chunks(rowid, embedding)
            SELECT rowid, embedding FROM conversation_chunks WHERE embedding IS NOT NULL;
        ")?;
        eprintln!("[migration v30] backfilled {} embeddings into vec_chunks", count);
    }

    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (30, ?1)", [now_epoch()])?;
    Ok(())
}

fn apply_v31(conn: &Connection) -> Result<(), AppError> {
    // Project Document RAG: extend conversation_chunks for document/artifact sources,
    // add document_edges for inter-document relationship graph,
    // add document_index_status for SHA-256 change detection.

    // 1. conversation_chunks: add source_type, file_path, section_title
    add_column_if_missing(conn, "conversation_chunks", "source_type", "TEXT NOT NULL DEFAULT 'conversation'")?;
    add_column_if_missing(conn, "conversation_chunks", "file_path", "TEXT")?;
    add_column_if_missing(conn, "conversation_chunks", "section_title", "TEXT")?;

    // 2. document_edges: inter-document link graph (extracted from markdown links)
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS document_edges (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            project_key   TEXT NOT NULL,
            source_path   TEXT NOT NULL,
            target_path   TEXT NOT NULL,
            relation      TEXT NOT NULL DEFAULT 'link',
            context       TEXT,
            created_at    INTEGER NOT NULL,
            UNIQUE(project_key, source_path, target_path, relation)
        );
        CREATE INDEX IF NOT EXISTS idx_document_edges_source ON document_edges(project_key, source_path);
        CREATE INDEX IF NOT EXISTS idx_document_edges_target ON document_edges(project_key, target_path);
    ")?;

    // 3. document_index_status: SHA-256 change detection for incremental re-indexing
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS document_index_status (
            project_key   TEXT NOT NULL,
            file_path     TEXT NOT NULL,
            content_hash  TEXT NOT NULL,
            chunk_count   INTEGER NOT NULL DEFAULT 0,
            indexed_at    INTEGER NOT NULL,
            PRIMARY KEY (project_key, file_path)
        );
    ")?;

    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (31, ?1)", [now_epoch()])?;
    Ok(())
}

fn apply_v32(conn: &Connection) -> Result<(), AppError> {
    // bge-m3 migration: upgrade vec_chunks from 384dim to 1024dim.
    // 1. Drop old vec0 virtual table (384dim)
    // 2. Recreate with 1024dim
    // 3. Clear stale embeddings from conversation_chunks (will be re-embedded by bge-m3)
    // 4. Add embed_model column to track which model generated the embedding

    // Drop old vec0 table
    conn.execute_batch("DROP TABLE IF EXISTS vec_chunks;")?;

    // Recreate with 1024dim
    conn.execute_batch("
        CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
            embedding float[1024] distance_metric=cosine
        );
    ")?;

    // Clear stale 384dim embeddings — they'll be re-generated by bge-m3
    conn.execute("UPDATE conversation_chunks SET embedding = NULL", [])?;

    // Track which embedding model was used
    add_column_if_missing(conn, "conversation_chunks", "embed_model", "TEXT")?;

    let cleared: i64 = conn.query_row(
        "SELECT COUNT(*) FROM conversation_chunks", [], |r| r.get(0)
    ).unwrap_or(0);

    if cleared > 0 {
        eprintln!("[migration v32] cleared {} stale 384dim embeddings, vec_chunks upgraded to 1024dim", cleared);
    }

    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (32, ?1)", [now_epoch()])?;
    Ok(())
}

fn apply_v33(conn: &Connection) -> Result<(), AppError> {
    // Meta-agent: add meta_conversation_id and onboarding_done to projects table.
    // meta_conversation_id — stores the singleton Meta conversation ID for each project.
    // onboarding_done — flag to prevent duplicate onboarding triggers.
    add_column_if_missing(conn, "projects", "meta_conversation_id", "TEXT")?;
    add_column_if_missing(conn, "projects", "onboarding_done", "INTEGER DEFAULT 0")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (33, ?1)", [now_epoch()])?;
    Ok(())
}

fn apply_v34(conn: &Connection) -> Result<(), AppError> {
    // Link insight findings to the Architect Review branch they were sent to.
    // Enables auto-resolve when the branch is adopted/archived.
    add_column_if_missing(conn, "insight_findings", "review_branch_id", "TEXT")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (34, ?1)", [now_epoch()])?;
    Ok(())
}

fn apply_v35(conn: &Connection) -> Result<(), AppError> {
    // Cache token classification for accurate cost calculation.
    // Claude API returns cache_read_input_tokens and cache_creation_input_tokens.
    add_column_if_missing(conn, "trace_log", "cache_read_tokens", "INTEGER DEFAULT 0")?;
    add_column_if_missing(conn, "trace_log", "cache_creation_tokens", "INTEGER DEFAULT 0")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (35, ?1)", [now_epoch()])?;
    Ok(())
}

/// v36 — defensive repair for users whose DB ended up at schema_version=35
/// but with missing `cache_read_tokens` / `cache_creation_tokens` columns
/// (seen in the wild after DB backup/restore mishap). `add_column_if_missing`
/// is idempotent, so running this is a no-op on correctly-migrated DBs.
/// Without this, `list_traces` SELECT fails with "no such column" and the
/// FE silently shows "No trace data yet" — see session 2026-04-18 s37.
fn apply_v36(conn: &Connection) -> Result<(), AppError> {
    add_column_if_missing(conn, "trace_log", "cache_read_tokens", "INTEGER DEFAULT 0")?;
    add_column_if_missing(conn, "trace_log", "cache_creation_tokens", "INTEGER DEFAULT 0")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (36, ?1)", [now_epoch()])?;
    Ok(())
}

/// Milliseconds since Unix epoch (for Message.timestamp)
pub fn now_epoch_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
