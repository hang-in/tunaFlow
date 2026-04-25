//! Integration tests using in-memory SQLite DB.
//! Tests migration safety, plan CRUD, trace insert/export, and branch plan lookup.

use rusqlite::{params, Connection};

// Import the library crate
use tuna_flow_lib::db;
use tuna_flow_lib::db::migrations::now_epoch_ms;

/// Create a fresh in-memory DB with all migrations applied.
fn setup_db() -> Connection {
    // Register sqlite-vec extension (same as db::init)
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }
    let mut conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    db::migrations::run(&mut conn).unwrap();
    conn
}

// ─── Migration tests ─────────────────────────────────────────────────────────

#[test]
fn migrations_apply_cleanly() {
    let conn = setup_db();
    let version: i64 = conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert!(version >= 6, "expected at least v6, got {}", version);
}

#[test]
fn v42_meta_notifications_recovered() {
    // v42 가 누락된 meta_notifications 를 idempotent 하게 복구한다.
    // 신규 DB 라도 v38 + v42 둘 다 CREATE IF NOT EXISTS 라 충돌 없이 테이블이 존재해야 함.
    let mut conn = setup_db();
    let has_table: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='meta_notifications'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(has_table, 1, "meta_notifications table must exist after v42");

    // Simulate the real-world broken state: DB at v41 with the table missing.
    // Drop the table, roll schema_version back to 41, then re-run migrations.
    conn.execute("DROP TABLE meta_notifications", []).unwrap();
    conn.execute("DELETE FROM schema_version WHERE version IN (42)", []).unwrap();
    db::migrations::run(&mut conn).unwrap();
    let recovered: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='meta_notifications'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(recovered, 1, "v42 must recover meta_notifications on stale DB");
}

#[test]
fn migrations_are_idempotent() {
    let mut conn = setup_db();
    // Run again — should not fail
    db::migrations::run(&mut conn).unwrap();
    let version: i64 = conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert!(version >= 6);
}

#[test]
fn v4_column_exists() {
    let conn = setup_db();
    // subtask_id should exist on artifacts
    conn.execute(
        "INSERT INTO artifacts (id, type, title, content, status, subtask_id, created_at, updated_at)
         VALUES ('a1', 'note', 'test', 'content', 'draft', NULL, 0, 0)",
        [],
    )
    .unwrap();
}

#[test]
fn v6_columns_exist() {
    let conn = setup_db();
    conn.execute(
        "INSERT INTO trace_log (conversation_id, input_tokens, output_tokens, cost_usd, recorded_at,
         trace_id, span_id, parent_span_id, operation, engine, duration_ms, status)
         VALUES ('c1', 10, 20, 0.01, 0, 'tid', 'sid', NULL, 'test', 'test', 100, 'ok')",
        [],
    )
    .unwrap();
}

// ─── Plan CRUD tests ─────────────────────────────────────────────────────────

fn seed_project_and_conversation(conn: &Connection) -> (String, String) {
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO projects (key, name, type, source, updated_at) VALUES ('p1', 'Test', 'project', 'configured', ?1)",
        [now / 1000],
    ).unwrap();
    conn.execute(
        "INSERT INTO conversations (id, project_key, label, type, mode, source, created_at, updated_at,
         total_input_tokens, total_output_tokens, total_cost_usd)
         VALUES ('conv1', 'p1', 'Test Conv', 'main', 'chat', 'tunadish', ?1, ?1, 0, 0, 0.0)",
        [now / 1000],
    ).unwrap();
    ("p1".into(), "conv1".into())
}

#[test]
fn plan_create_and_list() {
    let conn = setup_db();
    let (_, conv_id) = seed_project_and_conversation(&conn);
    let now = now_epoch_ms();

    conn.execute(
        "INSERT INTO plans (id, conversation_id, title, status, created_at, updated_at)
         VALUES ('plan1', ?1, 'My Plan', 'draft', ?2, ?2)",
        params![conv_id, now],
    ).unwrap();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM plans WHERE conversation_id = ?1",
            [&conv_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn plan_status_update() {
    let conn = setup_db();
    let (_, conv_id) = seed_project_and_conversation(&conn);
    let now = now_epoch_ms();

    conn.execute(
        "INSERT INTO plans (id, conversation_id, title, status, created_at, updated_at)
         VALUES ('plan2', ?1, 'Plan', 'draft', ?2, ?2)",
        params![conv_id, now],
    ).unwrap();

    conn.execute(
        "UPDATE plans SET status = 'active' WHERE id = 'plan2'",
        [],
    ).unwrap();

    let status: String = conn
        .query_row("SELECT status FROM plans WHERE id = 'plan2'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "active");
}

#[test]
fn subtask_create_and_status_cycle() {
    let conn = setup_db();
    let (_, conv_id) = seed_project_and_conversation(&conn);
    let now = now_epoch_ms();

    conn.execute(
        "INSERT INTO plans (id, conversation_id, title, status, created_at, updated_at)
         VALUES ('plan3', ?1, 'Plan', 'active', ?2, ?2)",
        params![conv_id, now],
    ).unwrap();

    conn.execute(
        "INSERT INTO plan_subtasks (id, plan_id, idx, title, status, created_at, updated_at)
         VALUES ('st1', 'plan3', 0, 'Task 1', 'todo', ?1, ?1)",
        [now],
    ).unwrap();

    // todo → in_progress → done
    conn.execute("UPDATE plan_subtasks SET status = 'in_progress' WHERE id = 'st1'", []).unwrap();
    conn.execute("UPDATE plan_subtasks SET status = 'done' WHERE id = 'st1'", []).unwrap();

    let status: String = conn
        .query_row("SELECT status FROM plan_subtasks WHERE id = 'st1'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "done");
}

// ─── v41: ws_event_log ──────────────────────────────────────────────────────

#[test]
fn v41_ws_event_log_table_exists() {
    let conn = setup_db();
    let names: Vec<String> = conn
        .prepare("PRAGMA table_info(ws_event_log)")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    for expected in ["id", "event_type", "payload", "created_at"] {
        assert!(
            names.iter().any(|n| n == expected),
            "ws_event_log is missing column {} after v41; got {:?}",
            expected,
            names
        );
    }
}

#[test]
fn v41_ws_event_log_idx_created_at() {
    let conn = setup_db();
    let idx: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_ws_event_log_created_at'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(idx, 1, "expected idx_ws_event_log_created_at index after v41");
}

#[test]
fn v41_ws_event_log_since_replay_ordering() {
    let conn = setup_db();
    // Seed three events spanning the cursor so we can verify ordering
    // and the `>=` boundary.
    conn.execute(
        "INSERT INTO ws_event_log (event_type, payload, created_at) VALUES ('a', '{\"type\":\"a\"}', 1000)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ws_event_log (event_type, payload, created_at) VALUES ('b', '{\"type\":\"b\"}', 2000)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ws_event_log (event_type, payload, created_at) VALUES ('c', '{\"type\":\"c\"}', 3000)",
        [],
    )
    .unwrap();

    let payloads: Vec<String> = conn
        .prepare(
            "SELECT payload FROM ws_event_log
             WHERE created_at >= ?1
             ORDER BY created_at ASC, id ASC
             LIMIT 2000",
        )
        .unwrap()
        .query_map([2000i64], |r| r.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert_eq!(payloads.len(), 2, "since=2000 should include b and c");
    assert!(payloads[0].contains("\"b\""), "first replayed event must be b, got {}", payloads[0]);
    assert!(payloads[1].contains("\"c\""), "second replayed event must be c, got {}", payloads[1]);
}

// ─── v40: branches.adopted_message_id ───────────────────────────────────────

#[test]
fn v40_adopted_message_id_column_exists() {
    let conn = setup_db();
    // `PRAGMA table_info(branches)` returns one row per column.
    let mut stmt = conn
        .prepare("PRAGMA table_info(branches)")
        .unwrap();
    let names: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(
        names.iter().any(|n| n == "adopted_message_id"),
        "expected branches.adopted_message_id column after v40 migration; got {:?}",
        names,
    );
}

#[test]
fn v40_adopted_message_id_starts_null() {
    let conn = setup_db();
    let now = now_epoch_ms();
    create_api_project(&conn, "proj1", "P1", None);
    create_api_conversation(&conn, "conv1", "proj1", "Main");
    conn.execute(
        "INSERT INTO branches (id, conversation_id, label, status, mode, created_at)
         VALUES ('br-v40', 'conv1', 'test', 'active', 'chat', ?1)",
        params![now],
    )
    .unwrap();
    let adopted: Option<String> = conn
        .query_row(
            "SELECT adopted_message_id FROM branches WHERE id = 'br-v40'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(adopted.is_none(), "fresh branch should have null adopted_message_id");

    // Simulate adopt_branch writing the summary id back.
    conn.execute(
        "UPDATE branches SET adopted_message_id = 'sum-123' WHERE id = 'br-v40'",
        [],
    )
    .unwrap();
    let adopted2: String = conn
        .query_row(
            "SELECT adopted_message_id FROM branches WHERE id = 'br-v40'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(adopted2, "sum-123");
}

// ─── Branch plan lookup ──────────────────────────────────────────────────────

#[test]
fn branch_canonical_conversation_id() {
    let conn = setup_db();
    let (_, conv_id) = seed_project_and_conversation(&conn);
    let now = now_epoch_ms();

    // Create branch
    conn.execute(
        "INSERT INTO branches (id, conversation_id, label, status, created_at)
         VALUES ('br1', ?1, 'b1', 'active', ?2)",
        params![conv_id, now],
    ).unwrap();

    // resolve_plan_conversation_id logic: branch:br1 → conv1
    let branch_conv_id = "branch:br1";
    let branch_id = &branch_conv_id["branch:".len()..];
    let resolved: String = conn
        .query_row(
            "SELECT conversation_id FROM branches WHERE id = ?1",
            [branch_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(resolved, conv_id);
}

#[test]
fn plan_visible_from_branch() {
    let conn = setup_db();
    let (_, conv_id) = seed_project_and_conversation(&conn);
    let now = now_epoch_ms();

    // Create active plan on the conversation
    conn.execute(
        "INSERT INTO plans (id, conversation_id, title, status, created_at, updated_at)
         VALUES ('plan4', ?1, 'Active Plan', 'active', ?2, ?2)",
        params![conv_id, now],
    ).unwrap();

    // Query with canonical conversation id (as resolve_plan_conversation_id would return)
    let plan_title: String = conn
        .query_row(
            "SELECT title FROM plans WHERE conversation_id = ?1 AND status = 'active' LIMIT 1",
            [&conv_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(plan_title, "Active Plan");
}

// ─── Trace log tests ─────────────────────────────────────────────────────────

#[test]
fn trace_insert_and_query() {
    let conn = setup_db();
    let (_, conv_id) = seed_project_and_conversation(&conn);

    conn.execute(
        "INSERT INTO trace_log (conversation_id, input_tokens, output_tokens, cost_usd, recorded_at,
         trace_id, span_id, operation, engine, duration_ms, status)
         VALUES (?1, 100, 200, 0.05, 1000, 'trace1', 'span1', 'agent.send', 'claude-code', 500, 'ok')",
        [&conv_id],
    ).unwrap();

    let (op, eng, dur, st): (String, String, i64, String) = conn
        .query_row(
            "SELECT operation, engine, duration_ms, status FROM trace_log WHERE trace_id = 'trace1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(op, "agent.send");
    assert_eq!(eng, "claude-code");
    assert_eq!(dur, 500);
    assert_eq!(st, "ok");
}

#[test]
fn trace_parent_span_linkage() {
    let conn = setup_db();
    let (_, conv_id) = seed_project_and_conversation(&conn);

    // Root span
    conn.execute(
        "INSERT INTO trace_log (conversation_id, input_tokens, output_tokens, cost_usd, recorded_at,
         trace_id, span_id, parent_span_id, operation, engine, duration_ms, status)
         VALUES (?1, 0, 0, 0.0, 1000, 'rt-trace', 'root-span', NULL, 'roundtable.run', 'system', 1000, 'ok')",
        [&conv_id],
    ).unwrap();

    // Participant span
    conn.execute(
        "INSERT INTO trace_log (conversation_id, input_tokens, output_tokens, cost_usd, recorded_at,
         trace_id, span_id, parent_span_id, operation, engine, duration_ms, status)
         VALUES (?1, 50, 100, 0.02, 1001, 'rt-trace', 'part-span', 'root-span', 'roundtable.participant', 'claude-code', 300, 'ok')",
        [&conv_id],
    ).unwrap();

    // Verify parent linkage
    let parent: String = conn
        .query_row(
            "SELECT parent_span_id FROM trace_log WHERE span_id = 'part-span'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(parent, "root-span");

    // Same trace_id
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM trace_log WHERE trace_id = 'rt-trace'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);
}

// ─── Eval tests ──────────────────────────────────────────────────────────────

#[test]
fn eval_run_crud() {
    let conn = setup_db();
    let (_, conv_id) = seed_project_and_conversation(&conn);
    let now = now_epoch_ms();

    conn.execute(
        "INSERT INTO eval_runs (id, conversation_id, title, prompt, rounds, status, created_at)
         VALUES ('er1', ?1, 'Test Eval', 'prompt', 1, 'pending', ?2)",
        params![conv_id, now],
    ).unwrap();

    conn.execute(
        "INSERT INTO eval_results (id, eval_run_id, agent_name, engine, round, content, created_at)
         VALUES ('res1', 'er1', 'Claude', 'claude-code', 1, 'response', ?1)",
        [now],
    ).unwrap();

    conn.execute("UPDATE eval_runs SET status = 'done' WHERE id = 'er1'", []).unwrap();

    let status: String = conn
        .query_row("SELECT status FROM eval_runs WHERE id = 'er1'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "done");

    let result_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM eval_results WHERE eval_run_id = 'er1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(result_count, 1);
}

// ─── Artifact-subtask link ───────────────────────────────────────────────────

#[test]
fn artifact_subtask_link() {
    let conn = setup_db();
    let (_, conv_id) = seed_project_and_conversation(&conn);
    let now = now_epoch_ms();

    conn.execute(
        "INSERT INTO plans (id, conversation_id, title, status, created_at, updated_at)
         VALUES ('plan5', ?1, 'Plan', 'active', ?2, ?2)",
        params![conv_id, now],
    ).unwrap();

    conn.execute(
        "INSERT INTO plan_subtasks (id, plan_id, idx, title, status, created_at, updated_at)
         VALUES ('st2', 'plan5', 0, 'Task', 'todo', ?1, ?1)",
        [now],
    ).unwrap();

    conn.execute(
        "INSERT INTO artifacts (id, conversation_id, type, title, content, status, created_at, updated_at)
         VALUES ('art1', ?1, 'note', 'Artifact', 'content', 'draft', ?2, ?2)",
        params![conv_id, now],
    ).unwrap();

    // Link
    conn.execute(
        "UPDATE artifacts SET subtask_id = 'st2' WHERE id = 'art1'",
        [],
    ).unwrap();

    let linked: String = conn
        .query_row("SELECT subtask_id FROM artifacts WHERE id = 'art1'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(linked, "st2");
}

// ─── HTTP API DB patterns ───────────────────────────────────────────────────

/// Helper: create a project via the same SQL the HTTP API uses.
fn create_api_project(conn: &Connection, key: &str, name: &str, path: Option<&str>) {
    let now = now_epoch_ms();
    conn.execute(
        "INSERT OR IGNORE INTO projects (key, name, path, type, source, hidden, updated_at) VALUES (?1, ?2, ?3, 'project', 'api', 0, ?4)",
        params![key, name, path, now],
    ).unwrap();
}

/// Helper: create a conversation via the same SQL the HTTP API uses.
fn create_api_conversation(conn: &Connection, id: &str, project_key: &str, label: &str) {
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO conversations (id, project_key, label, mode, usage_status, source, created_at, updated_at) VALUES (?1, ?2, ?3, 'chat', 'active', 'api', ?4, ?4)",
        params![id, project_key, label, now],
    ).unwrap();
}

#[test]
fn http_api_project_crud() {
    let conn = setup_db();
    create_api_project(&conn, "test-proj", "Test Project", Some("/tmp/test"));

    let name: String = conn.query_row("SELECT name FROM projects WHERE key = 'test-proj'", [], |r| r.get(0)).unwrap();
    assert_eq!(name, "Test Project");

    // Duplicate insert is ignored (OR IGNORE)
    create_api_project(&conn, "test-proj", "Different Name", None);
    let name2: String = conn.query_row("SELECT name FROM projects WHERE key = 'test-proj'", [], |r| r.get(0)).unwrap();
    assert_eq!(name2, "Test Project"); // unchanged
}

#[test]
fn http_api_conversation_crud() {
    let conn = setup_db();
    create_api_project(&conn, "proj1", "P1", None);
    create_api_conversation(&conn, "conv1", "proj1", "[E2E] Test Conv");

    let label: String = conn.query_row("SELECT label FROM conversations WHERE id = 'conv1'", [], |r| r.get(0)).unwrap();
    assert_eq!(label, "[E2E] Test Conv");

    // Delete conversation + messages
    conn.execute("INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES ('m1', 'conv1', 'user', 'hello', 0, 'done')", []).unwrap();
    conn.execute("DELETE FROM messages WHERE conversation_id = 'conv1'", []).unwrap();
    conn.execute("DELETE FROM conversations WHERE id = 'conv1'", []).unwrap();

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM conversations WHERE id = 'conv1'", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 0);
    let msg_count: i64 = conn.query_row("SELECT COUNT(*) FROM messages WHERE conversation_id = 'conv1'", [], |r| r.get(0)).unwrap();
    assert_eq!(msg_count, 0);
}

#[test]
fn http_api_branch_lifecycle() {
    let conn = setup_db();
    create_api_project(&conn, "proj1", "P1", None);
    create_api_conversation(&conn, "conv1", "proj1", "Main");

    let now = now_epoch_ms();

    // Create branch
    conn.execute(
        "INSERT INTO branches (id, conversation_id, label, status, mode, created_at) VALUES ('br1', 'conv1', 'test-branch', 'active', 'chat', ?1)",
        params![now],
    ).unwrap();

    // Create shadow conversation
    conn.execute(
        "INSERT INTO conversations (id, project_key, label, mode, usage_status, source, created_at, updated_at) VALUES ('branch:br1', 'proj1', 'Branch test-branch', 'chat', 'active', 'api', ?1, ?1)",
        params![now],
    ).unwrap();

    // Add message to shadow
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES ('m1', 'branch:br1', 'user', 'branch msg', ?1, 'done')",
        params![now],
    ).unwrap();

    // Archive
    conn.execute("UPDATE branches SET status = 'archived' WHERE id = 'br1'", []).unwrap();
    let status: String = conn.query_row("SELECT status FROM branches WHERE id = 'br1'", [], |r| r.get(0)).unwrap();
    assert_eq!(status, "archived");

    // Adopt
    conn.execute("UPDATE branches SET status = 'adopted' WHERE id = 'br1'", []).unwrap();
    let status2: String = conn.query_row("SELECT status FROM branches WHERE id = 'br1'", [], |r| r.get(0)).unwrap();
    assert_eq!(status2, "adopted");

    // Delete branch (active branch = full delete)
    conn.execute("DELETE FROM messages WHERE conversation_id = 'branch:br1'", []).unwrap();
    conn.execute("DELETE FROM conversations WHERE id = 'branch:br1'", []).unwrap();
    conn.execute("DELETE FROM branches WHERE id = 'br1'", []).unwrap();

    let br_count: i64 = conn.query_row("SELECT COUNT(*) FROM branches WHERE id = 'br1'", [], |r| r.get(0)).unwrap();
    assert_eq!(br_count, 0);
}

#[test]
fn http_api_adopt_summary_collects_all_assistants() {
    let conn = setup_db();
    create_api_project(&conn, "proj1", "P1", None);
    create_api_conversation(&conn, "conv1", "proj1", "Main");

    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO branches (id, conversation_id, label, status, mode, created_at) VALUES ('br1', 'conv1', 'rt-review', 'active', 'roundtable', ?1)",
        params![now],
    ).unwrap();
    conn.execute(
        "INSERT INTO conversations (id, project_key, label, mode, usage_status, source, created_at, updated_at) VALUES ('branch:br1', 'proj1', 'Branch RT', 'roundtable', 'active', 'api', ?1, ?1)",
        params![now],
    ).unwrap();

    // 3 assistant messages (RT participants)
    for (id, persona, engine, content) in [
        ("a1", "Reviewer", "claude", "Code looks good."),
        ("a2", "Architect", "gemini", "Consider MVC pattern."),
        ("a3", "Critic", "codex", "Error handling missing."),
    ] {
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, engine, persona, timestamp, status) VALUES (?1, 'branch:br1', 'assistant', ?2, ?3, ?4, ?5, 'done')",
            params![id, content, engine, persona, now],
        ).unwrap();
    }

    // Simulate adopt: collect all assistant messages
    let mut stmt = conn.prepare(
        "SELECT content, persona, engine FROM messages WHERE conversation_id = 'branch:br1' AND role = 'assistant' ORDER BY timestamp ASC"
    ).unwrap();
    let parts: Vec<String> = stmt.query_map([], |r| {
        let content: String = r.get(0)?;
        let persona: Option<String> = r.get(1)?;
        let engine: Option<String> = r.get(2)?;
        let label = persona.or(engine).unwrap_or_default();
        Ok(if label.is_empty() { content } else { format!("**[{}]** {}", label, content) })
    }).unwrap().filter_map(|r| r.ok()).collect();

    assert_eq!(parts.len(), 3);
    assert!(parts[0].contains("Reviewer"));
    assert!(parts[1].contains("Architect"));
    assert!(parts[2].contains("Critic"));

    let summary = parts.join("\n\n");
    assert!(summary.contains("**[Reviewer]**"));
    assert!(summary.contains("**[Architect]**"));
    assert!(summary.contains("**[Critic]**"));
}

/// Branch adopt 4 단계 (status flip → descendants archive → summary insert →
/// adopted_message_id back-pointer) 가 단일 transaction 안에서 atomic 으로
/// 실행되어야 한다 (`branchAdoptFailureAudit_2026-04-25.md` INV-2).
///
/// 본 테스트는 step 3 의 INSERT 가 **존재하지 않는 conversation_id** 에 대해
/// FK 위반으로 실패하도록 설정한 뒤 transaction 이 통째로 rollback 되는지
/// 확인한다. 기존 (non-transactional) 코드라면 step 1 만 commit 되어 branch 가
/// `adopted` 로 남았을 것 — 회귀 방지용 가드.
#[test]
fn adopt_branch_transaction_rolls_back_all_writes_on_failure() {
    let conn = setup_db();
    create_api_project(&conn, "proj1", "P1", None);
    create_api_conversation(&conn, "conv1", "proj1", "Main");

    let now = now_epoch_ms();

    // Active root branch + 1 descendant (to exercise the recursive archive step).
    conn.execute(
        "INSERT INTO branches (id, conversation_id, label, status, mode, created_at) VALUES ('br-root', 'conv1', 'root', 'active', 'chat', ?1)",
        params![now],
    ).unwrap();
    conn.execute(
        "INSERT INTO branches (id, conversation_id, label, status, mode, parent_branch_id, created_at) VALUES ('br-child', 'conv1', 'child', 'active', 'chat', 'br-root', ?1)",
        params![now],
    ).unwrap();

    // Mirror adopt_branch's 4-step transaction, but target a non-existent
    // conversation in step 3 so the INSERT fails with an FK violation.
    let tx = conn.unchecked_transaction().unwrap();

    // Step 1
    tx.execute(
        "UPDATE branches SET status = 'adopted' WHERE id = ?1",
        ["br-root"],
    ).unwrap();

    // Step 2
    tx.execute_batch(
        "WITH RECURSIVE descendants AS (
           SELECT id FROM branches WHERE parent_branch_id = 'br-root'
           UNION ALL
           SELECT b.id FROM branches b JOIN descendants d ON b.parent_branch_id = d.id
         )
         UPDATE branches SET status = 'archived'
         WHERE id IN (SELECT id FROM descendants) AND status = 'active';",
    ).unwrap();

    // Step 3 — FK violation (parent conversation 'ghost-conv' does not exist)
    let step3 = tx.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES (?1, 'ghost-conv', 'assistant', 'summary', ?2, 'done')",
        params!["msg-summary", now],
    );
    assert!(step3.is_err(), "step 3 must fail (FK violation precondition)");

    // Drop transaction (auto rollback — no commit reached)
    drop(tx);

    // Verify rollback: every write from steps 1+2 must be undone.
    let root_status: String = conn
        .query_row("SELECT status FROM branches WHERE id = 'br-root'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        root_status, "active",
        "INV-2 violated: root branch left as '{}' after transaction rollback",
        root_status
    );

    let child_status: String = conn
        .query_row("SELECT status FROM branches WHERE id = 'br-child'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        child_status, "active",
        "INV-2 violated: descendant branch left as '{}' after transaction rollback",
        child_status
    );

    let summary_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM messages WHERE id = 'msg-summary'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(summary_count, 0, "summary message must not survive rollback");
}

/// Happy path: adopt 의 4 단계가 transaction 내에서 모두 성공하면 commit 후
/// branch.status='adopted', descendant='archived', summary message 존재,
/// adopted_message_id 설정 — 모두 보장된다 (INV-1).
#[test]
fn adopt_branch_transaction_commits_atomically_on_success() {
    let conn = setup_db();
    create_api_project(&conn, "proj1", "P1", None);
    create_api_conversation(&conn, "conv1", "proj1", "Main");

    let now = now_epoch_ms();

    conn.execute(
        "INSERT INTO branches (id, conversation_id, label, status, mode, created_at) VALUES ('br-root', 'conv1', 'root', 'active', 'chat', ?1)",
        params![now],
    ).unwrap();
    conn.execute(
        "INSERT INTO branches (id, conversation_id, label, status, mode, parent_branch_id, created_at) VALUES ('br-child', 'conv1', 'child', 'active', 'chat', 'br-root', ?1)",
        params![now],
    ).unwrap();

    let tx = conn.unchecked_transaction().unwrap();
    tx.execute("UPDATE branches SET status = 'adopted' WHERE id = ?1", ["br-root"]).unwrap();
    tx.execute_batch(
        "WITH RECURSIVE descendants AS (
           SELECT id FROM branches WHERE parent_branch_id = 'br-root'
           UNION ALL
           SELECT b.id FROM branches b JOIN descendants d ON b.parent_branch_id = d.id
         )
         UPDATE branches SET status = 'archived'
         WHERE id IN (SELECT id FROM descendants) AND status = 'active';",
    ).unwrap();
    tx.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES (?1, 'conv1', 'assistant', 'summary', ?2, 'done')",
        params!["msg-summary", now],
    ).unwrap();
    tx.execute(
        "UPDATE branches SET adopted_message_id = ?1 WHERE id = ?2",
        params!["msg-summary", "br-root"],
    ).unwrap();
    tx.commit().unwrap();

    let root_status: String = conn
        .query_row("SELECT status FROM branches WHERE id = 'br-root'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(root_status, "adopted");

    let child_status: String = conn
        .query_row("SELECT status FROM branches WHERE id = 'br-child'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(child_status, "archived");

    let summary_id: Option<String> = conn
        .query_row("SELECT adopted_message_id FROM branches WHERE id = 'br-root'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(summary_id.as_deref(), Some("msg-summary"));

    let msg_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM messages WHERE id = 'msg-summary'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(msg_count, 1);
}

#[test]
fn http_api_fk_constraint_on_invalid_project() {
    let conn = setup_db();
    // Attempt to create conversation with non-existent project_key
    let result = conn.execute(
        "INSERT INTO conversations (id, project_key, label, mode, usage_status, source, created_at, updated_at) VALUES ('c1', 'nonexistent', 'test', 'chat', 'active', 'api', 0, 0)",
        [],
    );
    assert!(result.is_err(), "FK constraint should reject nonexistent project_key");
}

#[test]
fn http_api_message_send_pattern() {
    let conn = setup_db();
    create_api_project(&conn, "proj1", "P1", Some("/tmp/test"));
    create_api_conversation(&conn, "conv1", "proj1", "Test");

    // User message (dryRun pattern)
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES ('msg-user', 'conv1', 'user', 'What is 2+2?', ?1, 'done')",
        params![now],
    ).unwrap();

    // Verify project path lookup (same query HTTP API uses)
    let project_path: Option<String> = conn.query_row(
        "SELECT p.path FROM projects p JOIN conversations c ON c.project_key = p.key WHERE c.id = 'conv1'",
        [], |r| r.get(0),
    ).unwrap();
    assert_eq!(project_path, Some("/tmp/test".to_string()));

    // Assistant response (agent completion pattern)
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, engine, model, timestamp, status) VALUES ('msg-asst', 'conv1', 'assistant', 'Four.', 'claude', 'haiku', ?1, 'done')",
        params![now + 1],
    ).unwrap();

    // Verify message ordering
    let mut stmt = conn.prepare("SELECT role, content FROM messages WHERE conversation_id = 'conv1' ORDER BY timestamp ASC").unwrap();
    let msgs: Vec<(String, String)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().filter_map(|r| r.ok()).collect();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].0, "user");
    assert_eq!(msgs[1].0, "assistant");
    assert_eq!(msgs[1].1, "Four.");
}

// ─── Document RAG tests ─────────────────────────────────────────────────────

#[test]
fn v31_document_rag_tables_exist() {
    let conn = setup_db();

    // Setup: create project (FK target)
    let now = now_epoch_ms();
    conn.execute("INSERT INTO projects (key, name, type, source, updated_at) VALUES ('proj1', 'Test', 'local', 'manual', ?1)", params![now]).unwrap();

    // conversation_chunks should have new columns (conversation_id='' for document chunks, disable FK for this)
    conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
    conn.execute(
        "INSERT INTO conversation_chunks (id, project_key, conversation_id, kind, root_message_id, text_preview, created_at, source_type, file_path, section_title)
         VALUES ('dc1', 'proj1', '', 'document', '', 'test preview', 0, 'document', 'docs/plans/foo.md', '## Section 1')",
        [],
    ).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

    let source_type: String = conn.query_row(
        "SELECT source_type FROM conversation_chunks WHERE id = 'dc1'", [], |r| r.get(0)
    ).unwrap();
    assert_eq!(source_type, "document");

    // document_edges table should exist
    conn.execute(
        "INSERT INTO document_edges (project_key, source_path, target_path, relation, created_at)
         VALUES ('proj1', 'docs/plans/a.md', 'docs/plans/b.md', 'link', 0)",
        [],
    ).unwrap();

    // document_index_status table should exist
    conn.execute(
        "INSERT INTO document_index_status (project_key, file_path, content_hash, chunk_count, indexed_at)
         VALUES ('proj1', 'docs/plans/foo.md', 'abc123', 3, 0)",
        [],
    ).unwrap();
}

#[test]
fn document_edge_unique_constraint() {
    let conn = setup_db();
    conn.execute(
        "INSERT INTO document_edges (project_key, source_path, target_path, relation, created_at)
         VALUES ('p1', 'a.md', 'b.md', 'link', 0)", [],
    ).unwrap();
    // Same edge again should replace (OR REPLACE from the app code, but raw INSERT should fail)
    let result = conn.execute(
        "INSERT INTO document_edges (project_key, source_path, target_path, relation, created_at)
         VALUES ('p1', 'a.md', 'b.md', 'link', 1)", [],
    );
    assert!(result.is_err(), "duplicate edge should violate UNIQUE constraint");
}

#[test]
fn document_graph_query() {
    let conn = setup_db();
    conn.execute("INSERT INTO document_edges (project_key, source_path, target_path, relation, created_at) VALUES ('p1', 'a.md', 'b.md', 'link', 0)", []).unwrap();
    conn.execute("INSERT INTO document_edges (project_key, source_path, target_path, relation, created_at) VALUES ('p1', 'b.md', 'c.md', 'link', 0)", []).unwrap();
    conn.execute("INSERT INTO document_edges (project_key, source_path, target_path, relation, created_at) VALUES ('p2', 'x.md', 'y.md', 'link', 0)", []).unwrap();

    let edges = tuna_flow_lib::commands::document_index::get_document_graph(&conn, "p1");
    assert_eq!(edges.len(), 2, "should only return edges for project p1");
    assert_eq!(edges[0].source_path, "a.md");
    assert_eq!(edges[1].source_path, "b.md");
}

#[test]
fn document_orphan_detection() {
    let conn = setup_db();
    // Index status: 3 files exist
    conn.execute("INSERT INTO document_index_status (project_key, file_path, content_hash, chunk_count, indexed_at) VALUES ('p1', 'a.md', 'h1', 1, 0)", []).unwrap();
    conn.execute("INSERT INTO document_index_status (project_key, file_path, content_hash, chunk_count, indexed_at) VALUES ('p1', 'b.md', 'h2', 1, 0)", []).unwrap();
    conn.execute("INSERT INTO document_index_status (project_key, file_path, content_hash, chunk_count, indexed_at) VALUES ('p1', 'c.md', 'h3', 1, 0)", []).unwrap();
    // Edge: a.md → b.md (so b.md is referenced, a.md and c.md are orphans)
    conn.execute("INSERT INTO document_edges (project_key, source_path, target_path, relation, created_at) VALUES ('p1', 'a.md', 'b.md', 'link', 0)", []).unwrap();

    let orphans = tuna_flow_lib::commands::document_index::find_orphan_documents(&conn, "p1");
    assert_eq!(orphans.len(), 2, "a.md and c.md should be orphans (not referenced by any other doc)");
    assert!(orphans.contains(&"a.md".to_string()));
    assert!(orphans.contains(&"c.md".to_string()));
}

#[test]
fn document_index_status_query() {
    let conn = setup_db();
    conn.execute("INSERT INTO document_index_status (project_key, file_path, content_hash, chunk_count, indexed_at) VALUES ('p1', 'docs/a.md', 'hash1', 5, 1000)", []).unwrap();
    conn.execute("INSERT INTO document_index_status (project_key, file_path, content_hash, chunk_count, indexed_at) VALUES ('p1', 'docs/b.md', 'hash2', 3, 2000)", []).unwrap();

    let status = tuna_flow_lib::commands::document_index::get_index_status(&conn, "p1");
    assert_eq!(status.len(), 2);
    assert_eq!(status[0]["filePath"], "docs/a.md");
    assert_eq!(status[0]["chunkCount"], 5);
    assert_eq!(status[1]["filePath"], "docs/b.md");
}

#[test]
fn markdown_parser_integration() {
    use tuna_flow_lib::commands::document_index::{split_by_headings, extract_markdown_links, sha256_hex};

    // Test realistic plan document
    let content = r#"# Authentication Migration Plan

## 1. Overview

This plan covers migrating from JWT to OAuth2 PKCE.
The migration affects 14 files across 3 modules.

## 2. Implementation Steps

1. Install oauth2 crate
2. Create token exchange endpoint
3. Update middleware

See [auth design](./authDesignDoc.md) for details.
Also references [session management](../ideas/sessionManagementIdea.md).

## 3. Risks

- Token invalidation race condition
- Backward compatibility with existing sessions
"#;

    let sections = split_by_headings(content);
    assert!(sections.len() >= 3, "should have at least 3 sections, got {}", sections.len());
    assert!(sections.iter().any(|s| s.title.contains("Overview")));
    assert!(sections.iter().any(|s| s.title.contains("Implementation")));
    assert!(sections.iter().any(|s| s.title.contains("Risks")));

    let links = extract_markdown_links(content);
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].target, "./authDesignDoc.md");
    assert_eq!(links[1].target, "../ideas/sessionManagementIdea.md");

    // SHA-256 change detection
    let hash1 = sha256_hex(content);
    let hash2 = sha256_hex(&format!("{}\n\n## New Section\n\nAdded content.", content));
    assert_ne!(hash1, hash2, "different content should produce different hashes");
    assert_eq!(sha256_hex(content), hash1, "same content should produce same hash");
}
