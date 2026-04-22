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
    if current < 37 {
        apply_v37(conn)?;
    }
    if current < 38 {
        apply_v38(conn)?;
    }
    if current < 39 {
        apply_v39(conn)?;
    }
    if current < 40 {
        apply_v40(conn)?;
    }
    if current < 41 {
        apply_v41(conn)?;
    }
    if current < 42 {
        apply_v42(conn)?;
    }
    if current < 43 {
        apply_v43(conn)?;
    }
    if current < 44 {
        apply_v44(conn)?;
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

/// v37 — per-project conventions sync toggle. When on, ContextPack skips the
/// static layers (platform / agent-role / persona / user-profile) because
/// they've been synced into CLAUDE.md/AGENTS.md/GEMINI.md — they get prepended
/// automatically by the CLI and (for Anthropic API) benefit from prompt cache.
/// Default 0 (off) — experimental opt-in. See `conventionsContextSyncPlan.md`.
fn apply_v37(conn: &Connection) -> Result<(), AppError> {
    add_column_if_missing(conn, "projects", "conventions_sync_enabled", "INTEGER DEFAULT 0")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (37, ?1)", [now_epoch()])?;
    Ok(())
}

/// v38 — meta_notifications 테이블 신설.
/// Meta agent 알림 (워크플로우 이벤트, Tier 2 분석 결과) 영속화.
/// 설계: docs/plans/metaAgentPlan.md
fn apply_v38(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS meta_notifications (
            id            TEXT PRIMARY KEY,
            project_key   TEXT,
            kind          TEXT NOT NULL,
            title         TEXT NOT NULL,
            summary       TEXT,
            route_json    TEXT,
            created_at    INTEGER NOT NULL,
            read_at       INTEGER,
            dismissed_at  INTEGER,
            FOREIGN KEY (project_key) REFERENCES projects(key) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_meta_notif_project ON meta_notifications(project_key);
        CREATE INDEX IF NOT EXISTS idx_meta_notif_created ON meta_notifications(created_at DESC);
    ")?;
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (38, ?1)", [now_epoch_ms()])?;
    Ok(())
}

/// v39 — conversation_chunks 스테일 row 정리.
/// 구버전 `index_conversation` (현재 dead_code) 이 저장하던 `kind='anchor'` / `kind='pair'`
/// row 는 현재 production 경로(`build_sliding_window_chunks` 는 `window` 만 생성)에서
/// 재인덱싱 대상이 아니라 embedding=NULL 인 채로 영구 잔존. 벡터 검색에선 어차피 무시되지만
/// COUNT/통계를 왜곡시키고 disk 차지. 정리.
fn apply_v39(conn: &Connection) -> Result<(), AppError> {
    // 1) vec_chunks(rowid 매핑) 에 있는 해당 chunk 들도 정리.
    let stale_rowids: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT rowid FROM conversation_chunks
             WHERE embedding IS NULL AND kind IN ('anchor', 'pair')"
        )?;
        stmt.query_map([], |r| r.get::<_, i64>(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    };
    for rid in &stale_rowids {
        // vec_chunks 는 virtual table (vec0). 없는 rowid 삭제는 그냥 no-op.
        conn.execute("DELETE FROM vec_chunks WHERE rowid = ?1", [rid]).ok();
    }
    // 2) 본체 row 삭제
    let deleted = conn.execute(
        "DELETE FROM conversation_chunks
         WHERE embedding IS NULL AND kind IN ('anchor', 'pair')",
        [],
    )?;
    eprintln!("[migration v39] cleaned up {} stale anchor/pair chunks (NULL embedding)", deleted);
    conn.execute("INSERT INTO schema_version (version, applied_at) VALUES (39, ?1)", [now_epoch_ms()])?;
    Ok(())
}

fn apply_v40(conn: &Connection) -> Result<(), AppError> {
    // Phase 2 Finding 2-2: `adopt_branch` command currently inserts a
    // system message into the parent conversation to mark the adoption
    // but never records which message id that was. The mobile δ-Branch
    // screen needs a cheap lookup for "was this branch adopted, and
    // where does the summary live?" — so record the id on the row
    // itself. Null = not adopted.
    add_column_if_missing(conn, "branches", "adopted_message_id", "TEXT")?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (40, ?1)",
        [now_epoch_ms()],
    )?;
    Ok(())
}

fn apply_v41(conn: &Connection) -> Result<(), AppError> {
    // Phase 2 Finding 2-6: append-only log of WS events, so a mobile
    // client that reconnects after a brief drop can ask for `?since=<ms>`
    // and replay the events it missed instead of forcing a full refetch.
    //
    // Schema is intentionally thin — `id` autoincrements for monotonic
    // ordering; `event_type` duplicates whatever the payload's `type`
    // field says, but having it as a column makes future filtering /
    // indexing possible without re-parsing JSON. The index on
    // `created_at` supports the `WHERE created_at >= ?` replay query.
    //
    // TTL: a background task (see http_api::events::spawn_ttl_cleanup)
    // trims rows older than 24 h every hour. Non-goal: at-least-once
    // delivery guarantees; reconnect windows past the TTL must re-sync
    // from the DB through the regular REST endpoints.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ws_event_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL,
            payload TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_ws_event_log_created_at
            ON ws_event_log(created_at);",
    )?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (41, ?1)",
        [now_epoch_ms()],
    )?;
    Ok(())
}

/// v42 — `meta_notifications` 복구 migration.
///
/// 실제 필드에서 발견된 불일치: 일부 DB 는 schema_version = 41 로 기록되어
/// 있지만 `meta_notifications` 테이블이 존재하지 않는 상태였다. `.pre-v38.bak`
/// 으로부터의 복원이나 과거 v38 실행 중 부분 실패로 추정됨. 결과적으로 베타
/// 시점 HTTP API `GET /meta-notifications` 가 `db: no such table` 500 에러.
///
/// 해결: `CREATE TABLE IF NOT EXISTS` 를 v42 로 다시 돌려 idempotent 하게 복구.
/// 신규 DB 는 이미 v38 이 같은 DDL 을 찍었으므로 영향 없음 (멱등).
fn apply_v42(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta_notifications (
            id            TEXT PRIMARY KEY,
            project_key   TEXT,
            kind          TEXT NOT NULL,
            title         TEXT NOT NULL,
            summary       TEXT,
            route_json    TEXT,
            created_at    INTEGER NOT NULL,
            read_at       INTEGER,
            dismissed_at  INTEGER,
            FOREIGN KEY (project_key) REFERENCES projects(key) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_meta_notif_project ON meta_notifications(project_key);
        CREATE INDEX IF NOT EXISTS idx_meta_notif_created ON meta_notifications(created_at DESC);",
    )?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (42, ?1)",
        [now_epoch_ms()],
    )?;
    Ok(())
}

/// v43 — `agent_session_audit` 테이블 추가.
///
/// Phase 3a (harnessVerificationGapPlan §4) — 에이전트 세션의 commit/rollback
/// 이력을 기록. 중간 실패 시 어떤 세션이 어떤 상태로 끝났는지 추적 가능해야
/// State Pollution (half-formed row 잔존) 디버깅이 실현된다.
///
/// 본 migration 은 **스키마만 추가**. 실제 persistence.rs 와 연결하는 로직은
/// Phase 3b 에서 wrapper 와 함께 도입.
fn apply_v43(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS agent_session_audit (
            session_id    TEXT PRIMARY KEY,
            conversation_id TEXT,
            started_at    INTEGER NOT NULL,
            ended_at      INTEGER,
            outcome       TEXT NOT NULL DEFAULT 'in_progress',
            -- 'committed' | 'rolled_back' | 'in_progress' | 'panic'
            rollback_reason TEXT,
            savepoint_name TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_asa_outcome ON agent_session_audit(outcome);
        CREATE INDEX IF NOT EXISTS idx_asa_conv ON agent_session_audit(conversation_id);
        CREATE INDEX IF NOT EXISTS idx_asa_started ON agent_session_audit(started_at DESC);",
    )?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (43, ?1)",
        [now_epoch_ms()],
    )?;
    Ok(())
}

/// v44 — `query_cache` 테이블 추가.
///
/// Phase A (searchPipelineFromSecallPlan §4): query expansion 결과 캐싱.
/// secall 의 `query_expand.rs` 는 Claude Haiku subprocess 호출로 쿼리를 확장
/// 하는데 매번 ~1-2초 걸림. 같은 쿼리가 반복되는 경우가 많으므로 7일 캐시로
/// 대부분의 호출을 원천 차단한다.
///
/// - `query` (PK): 원본 쿼리 (정규화: trim + lowercase)
/// - `expanded`: 확장된 키워드 문자열 (공백 구분)
/// - `cached_at`: 캐시 생성 시각 (7일 TTL 계산용)
fn apply_v44(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS query_cache (
            query      TEXT PRIMARY KEY,
            expanded   TEXT NOT NULL,
            cached_at  INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_query_cache_cached_at ON query_cache(cached_at);",
    )?;
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (44, ?1)",
        [now_epoch_ms()],
    )?;
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

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Apply ONLY v43 in isolation — `run()` triggers earlier migrations that
    /// depend on sqlite-vec `vec0` module which isn't loaded in the unit-test
    /// harness. We prepare `schema_version` manually then call `apply_v43`
    /// directly.
    fn open_with_v43_only() -> Connection {
        let conn = Connection::open_in_memory().expect("open");
        conn.execute_batch(schema::CREATE_SCHEMA_VERSION).expect("schema_version");
        apply_v43(&conn).expect("apply_v43");
        conn
    }

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [name],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false)
    }

    #[test]
    fn v43_creates_agent_session_audit() {
        let conn = open_with_v43_only();
        assert!(table_exists(&conn, "agent_session_audit"),
            "v43 migration must create agent_session_audit table");
    }

    #[test]
    fn v43_schema_has_expected_columns() {
        let conn = open_with_v43_only();
        let mut stmt = conn.prepare("PRAGMA table_info(agent_session_audit)").unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        for expected in ["session_id", "conversation_id", "started_at", "ended_at", "outcome", "rollback_reason", "savepoint_name"] {
            assert!(cols.contains(&expected.to_string()),
                "column {} missing from agent_session_audit, cols={:?}", expected, cols);
        }
    }

    #[test]
    fn v43_indexes_exist() {
        let conn = open_with_v43_only();
        for idx in ["idx_asa_outcome", "idx_asa_conv", "idx_asa_started"] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    [idx],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "index {} missing", idx);
        }
    }

    #[test]
    fn v43_default_outcome_is_in_progress() {
        let conn = open_with_v43_only();
        conn.execute(
            "INSERT INTO agent_session_audit (session_id, started_at) VALUES (?1, ?2)",
            params!["s1", now_epoch_ms()],
        )
        .expect("insert with defaults");
        let outcome: String = conn
            .query_row(
                "SELECT outcome FROM agent_session_audit WHERE session_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(outcome, "in_progress", "default outcome should be in_progress");
    }

    #[test]
    fn v43_records_schema_version() {
        let conn = open_with_v43_only();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_version WHERE version=43",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "schema_version must record v43 application");
    }

    // ─── v44 — query_cache ──────────────────────────────────────────────────

    fn open_with_v44_only() -> Connection {
        let conn = Connection::open_in_memory().expect("open");
        conn.execute_batch(schema::CREATE_SCHEMA_VERSION).expect("schema_version");
        apply_v44(&conn).expect("apply_v44");
        conn
    }

    #[test]
    fn v44_creates_query_cache() {
        let conn = open_with_v44_only();
        assert!(table_exists(&conn, "query_cache"), "v44 must create query_cache");
    }

    #[test]
    fn v44_schema_has_expected_columns() {
        let conn = open_with_v44_only();
        let mut stmt = conn.prepare("PRAGMA table_info(query_cache)").unwrap();
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        for expected in ["query", "expanded", "cached_at"] {
            assert!(cols.contains(&expected.to_string()),
                "column {} missing, cols={:?}", expected, cols);
        }
    }

    #[test]
    fn v44_query_is_primary_key() {
        let conn = open_with_v44_only();
        // Enable FK checks so PK violations surface consistently.
        conn.execute(
            "INSERT INTO query_cache (query, expanded, cached_at) VALUES ('k', 'v', 1)",
            [],
        )
        .unwrap();
        let err = conn.execute(
            "INSERT INTO query_cache (query, expanded, cached_at) VALUES ('k', 'v2', 2)",
            [],
        );
        assert!(err.is_err(), "second insert of same query PK must fail");
    }

    #[test]
    fn v44_records_schema_version() {
        let conn = open_with_v44_only();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_version WHERE version=44",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
