//! Common DB query helpers shared across context-domain commands
//! (agents, plans, memos, artifacts, roundtable).

use rusqlite::{params, Connection};

/// Load the N most recent messages from a conversation in chronological order.
/// Returns `(role, content)` pairs.
pub fn load_recent_messages(
    conn: &Connection,
    conversation_id: &str,
    limit: i64,
) -> Vec<(String, String)> {
    load_recent_messages_with_author(conn, conversation_id, limit)
        .into_iter()
        .map(|(role, content, _, _)| (role, content))
        .collect()
}

/// Load recent messages with author metadata.
/// Returns `(role, content, engine, persona)` tuples.
pub fn load_recent_messages_with_author(
    conn: &Connection,
    conversation_id: &str,
    limit: i64,
) -> Vec<(String, String, Option<String>, Option<String>)> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT role, content, engine, persona FROM messages
         WHERE conversation_id = ?1
         ORDER BY timestamp DESC LIMIT ?2",
    ) else {
        return Vec::new();
    };
    let mut rows: Vec<(String, String, Option<String>, Option<String>)> = stmt
        .query_map(params![conversation_id, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map(|mapped| mapped.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    rows.reverse();
    rows
}

/// Load the display label for a conversation (custom_label if set, otherwise label).
pub fn conversation_label(conn: &Connection, conversation_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT COALESCE(custom_label, label) FROM conversations WHERE id = ?1",
        [conversation_id],
        |row| row.get(0),
    )
    .ok()
}

/// Load the anchor (checkpoint) message for a branch conversation.
///
/// Given a branch shadow conversation id (format `branch:{branch_id}`),
/// looks up the branch's checkpoint_id and returns `(role, content)` of that message.
pub fn load_anchor_message(conn: &Connection, branch_conv_id: &str) -> Option<(String, String)> {
    if !branch_conv_id.starts_with("branch:") {
        return None;
    }
    let branch_id = &branch_conv_id["branch:".len()..];
    let checkpoint_id: String = conn
        .query_row(
            "SELECT checkpoint_id FROM branches WHERE id = ?1",
            [branch_id],
            |row| row.get(0),
        )
        .ok()?;
    conn.query_row(
        "SELECT role, content FROM messages WHERE id = ?1",
        [&checkpoint_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )
    .ok()
}

/// Load the parent conversation id for a branch shadow conversation.
pub fn parent_conversation_id(conn: &Connection, branch_conv_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT parent_id FROM conversations WHERE id = ?1",
        [branch_conv_id],
        |row| row.get(0),
    )
    .ok()
    .flatten()
}

/// Load the project_key for a conversation, then resolve the project path.
pub fn project_path_for_conversation(conn: &Connection, conversation_id: &str) -> Option<String> {
    let project_key: String = conn
        .query_row(
            "SELECT project_key FROM conversations WHERE id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .ok()?;
    conn.query_row(
        "SELECT path FROM projects WHERE key = ?1",
        [&project_key],
        |row| row.get(0),
    )
    .ok()
    .flatten()
}
