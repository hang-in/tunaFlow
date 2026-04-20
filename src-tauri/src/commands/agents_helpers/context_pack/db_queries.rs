/// DB-dependent ContextPack queries.
///
/// All functions here take `&rusqlite::Connection` and return pre-built section strings
/// or raw data. Pure assembly (no DB) lives in `section_builders.rs`.
use rusqlite::params;

use super::utils::{format_section, truncate_str};

/// Maximum number of prior messages for non-Claude lite context prefix.
pub const LITE_CONTEXT_MESSAGES_LIMIT: i64 = 4;
/// Maximum total characters for the lite context prefix.
const LITE_CONTEXT_MAX_CHARS: usize = 4000;

/// Maximum characters for the anchor message included in inheritance context.
const ANCHOR_MAX_CHARS: usize = 600;
/// Maximum recent parent turns for thread inheritance.
const THREAD_PARENT_RECENT: i64 = 3;
/// Maximum recent parent turns for RT inheritance (more concise).
const RT_PARENT_RECENT: i64 = 2;

// ─── Plan ────────────────────────────────────────────────────────────────────

/// Build `## Active Plan` section from DB for the given conversation.
pub fn build_plan_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<String> {
    let plan: (String, String, Option<String>, String, Option<String>) = conn
        .query_row(
            "SELECT id, title, description, phase, slug FROM plans
             WHERE conversation_id = ?1 AND status = 'active'
             ORDER BY updated_at DESC LIMIT 1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .ok()?;

    let (plan_id, title, description, phase, slug) = plan;

    let mut stmt = conn
        .prepare(
            "SELECT idx, title, status, details, depends_on, parallel_group FROM plan_subtasks
             WHERE plan_id = ?1 ORDER BY idx",
        )
        .ok()?;
    let subtasks: Vec<(i64, String, String, Option<String>, Option<String>, Option<String>)> = stmt
        .query_map([&plan_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
        })
        .ok()?
        .filter_map(|r| r.ok())
        .collect();

    let mut out = format!("## Active Plan (phase: {})\n\n### {}\n", phase, title);
    if let Some(desc) = &description {
        if !desc.is_empty() {
            out.push_str(&format!("{}\n", desc));
        }
    }
    // Canonical slug — the only correct filename prefix for this plan's
    // documents. Architect must write `docs/plans/{slug}-task-NN.md` using
    // this exact value (not a slugify of the title, not a manual
    // abbreviation). Reviewer and result/review writers all read the same
    // source, so any other prefix will not be discovered downstream.
    if let Some(s) = &slug {
        if !s.is_empty() {
            out.push_str(&format!(
                "\n> **Plan slug (canonical):** `{}`\n> Task file prefix MUST be `{}-task-NN.md` — use this slug verbatim.\n",
                s, s
            ));
        }
    }
    out.push_str(&format!("\n> **Current phase: {}** — Do NOT create a new plan. Propose revisions to this plan if needed.\n", phase));

    if !subtasks.is_empty() {
        out.push_str("\n**Subtasks:**\n");
        for (idx, title, status, _, depends_on, group) in &subtasks {
            let icon = match status.as_str() {
                "done" => "✅",
                "in_progress" => "🔧",
                _ => "⬜",
            };
            let mut suffix = String::new();
            if let Some(g) = group {
                if !g.is_empty() { suffix.push_str(&format!(" [{}]", g)); }
            }
            if let Some(deps) = depends_on {
                if deps != "[]" && !deps.is_empty() {
                    let dep_str = deps.trim_matches(|c| c == '[' || c == ']').replace(' ', "");
                    if !dep_str.is_empty() {
                        suffix.push_str(&format!(" (depends: {})", dep_str));
                    }
                }
            }
            out.push_str(&format!("- {} Task {:02}: {}{}\n", icon, idx, title, suffix));
        }
    }

    let done_count = subtasks.iter().filter(|(_, _, s, _, _, _)| s == "done").count();
    let total = subtasks.len();
    if total > 0 {
        out.push_str(&format!("\n**Progress:** {}/{} done\n", done_count, total));
    }

    Some(out)
}

/// Resolve branch conversation_id to its parent for plan lookup.
pub fn resolve_plan_conversation_id(conn: &rusqlite::Connection, conversation_id: &str) -> String {
    if !conversation_id.starts_with("branch:") {
        return conversation_id.to_string();
    }
    let branch_id = &conversation_id["branch:".len()..];
    conn.query_row(
        "SELECT conversation_id FROM branches WHERE id = ?1",
        [branch_id],
        |row| row.get::<_, String>(0),
    )
    .unwrap_or_else(|_| conversation_id.to_string())
}

// ─── Findings & Artifacts ────────────────────────────────────────────────────

/// Build `## Recent Agent Findings` from the most recent roundtable_brief memos.
///
/// Queries briefs from the conversation and any RT branch shadow conversations.
pub fn build_findings_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<String> {
    let mut stmt = conn
        .prepare(
            "SELECT content FROM memos
             WHERE type = 'roundtable_brief'
               AND (conversation_id = ?1
                    OR conversation_id IN (
                      SELECT 'branch:' || id FROM branches WHERE conversation_id = ?1
                    ))
             ORDER BY created_at DESC LIMIT 3",
        )
        .ok()?;
    let briefs: Vec<String> = stmt
        .query_map([conversation_id], |row| row.get::<_, String>(0))
        .ok()?
        .filter_map(|r| r.ok())
        .collect();

    if briefs.is_empty() {
        return None;
    }

    let mut out = String::from("## Recent Agent Findings\n");
    for (i, brief) in briefs.iter().enumerate() {
        let truncated = if brief.len() > 600 {
            let end = brief.char_indices().map(|(i, _)| i).take_while(|&i| i <= 597).last().unwrap_or(0);
            format!("{}...", &brief[..end])
        } else {
            brief.clone()
        };
        if i > 0 {
            out.push_str("\n---\n");
        }
        out.push('\n');
        out.push_str(&truncated);
        out.push('\n');
    }

    Some(out)
}

/// Build `## Recent Artifacts` from the most recent approved/draft artifacts.
pub fn build_artifact_handoff_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<String> {
    let mut stmt = conn
        .prepare(
            "SELECT title, type, status, content FROM artifacts
             WHERE conversation_id = ?1
             ORDER BY updated_at DESC LIMIT 3",
        )
        .ok()?;
    let rows: Vec<(String, String, String, String)> = stmt
        .query_map([conversation_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .ok()?
        .filter_map(|r| r.ok())
        .collect();

    if rows.is_empty() {
        return None;
    }

    let mut out = String::from("## Recent Artifacts\n");
    for (title, art_type, status, content) in &rows {
        let preview = if content.len() > 120 {
            let end = content.char_indices().map(|(i, _)| i).take_while(|&i| i <= 117).last().unwrap_or(0);
            format!("{}...", &content[..end])
        } else {
            content.clone()
        };
        out.push_str(&format!(
            "\n**{}** ({}·{}): {}\n",
            title, art_type, status, preview
        ));
    }

    Some(out)
}

// ─── Lite context ────────────────────────────────────────────────────────────

/// Build a lightweight context prefix for non-Claude engines.
///
/// For branch conversations, also includes anchor message + recent parent turns
/// so non-Claude engines in threads still get inherited context.
pub fn build_lite_context_prompt(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    user_prompt: &str,
) -> String {
    use crate::commands::context_queries::{load_anchor_message, load_recent_messages, parent_conversation_id};

    let is_branch = conversation_id.starts_with("branch:");

    let mut parent_prefix = String::new();
    if is_branch {
        if let Some((_role, content)) = load_anchor_message(conn, conversation_id) {
            let truncated = truncate_str(&content, 400);
            parent_prefix.push_str(&format!("Thread anchor:\n{}\n\n", truncated));
        }
        if let Some(parent_id) = parent_conversation_id(conn, conversation_id) {
            let parent_rows = load_recent_messages(conn, &parent_id, 2);
            if !parent_rows.is_empty() {
                parent_prefix.push_str("Parent conversation:\n");
                for (role, content) in &parent_rows {
                    parent_prefix.push_str(&format!("[{}] {}\n", role, truncate_str(content, 200)));
                }
                parent_prefix.push_str("\n---\n\n");
            }
        }
    }

    let Ok(mut stmt) = conn.prepare(
        "SELECT role, content FROM messages
         WHERE conversation_id = ?1
         ORDER BY timestamp DESC LIMIT ?2",
    ) else {
        return format!("{}{}", parent_prefix, user_prompt);
    };

    let mut rows: Vec<(String, String)> = stmt
        .query_map(params![conversation_id, LITE_CONTEXT_MESSAGES_LIMIT], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map(|mapped| mapped.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    rows.reverse();

    if rows.is_empty() && parent_prefix.is_empty() {
        return user_prompt.to_string();
    }

    let mut context = parent_prefix;
    if !rows.is_empty() {
        context.push_str("Recent conversation:\n");
        let mut char_count = context.len();
        for (role, content) in &rows {
            let truncated = truncate_str(content, 600);
            let line = format!("[{}] {}\n", role, truncated);
            if char_count + line.len() > LITE_CONTEXT_MAX_CHARS {
                break;
            }
            context.push_str(&line);
            char_count += line.len();
        }
        context.push_str("\n---\n\n");
    }

    format!("{}{}", context, user_prompt)
}

// ─── Thread / RT inheritance ─────────────────────────────────────────────────

/// Build an inheritance context section for a thread (branch) conversation.
///
/// Priority: anchor message > recent parent turns.
pub fn build_thread_inheritance_section(
    conn: &rusqlite::Connection,
    branch_conv_id: &str,
) -> Option<String> {
    use crate::commands::context_queries::{load_anchor_message, load_recent_messages, parent_conversation_id};

    let mut parts: Vec<String> = Vec::new();

    if let Some((_role, content)) = load_anchor_message(conn, branch_conv_id) {
        let truncated = truncate_str(&content, ANCHOR_MAX_CHARS);
        parts.push(format!("### Thread Anchor\n\nThis thread was started from the following message:\n{}", truncated));
    }

    if let Some(parent_id) = parent_conversation_id(conn, branch_conv_id) {
        let recent = load_recent_messages(conn, &parent_id, THREAD_PARENT_RECENT);
        if !recent.is_empty() {
            parts.push(format_section("Recent parent conversation", &recent, 300));
        }
    }

    if parts.is_empty() {
        return None;
    }
    Some(format!("## Thread Context\n\n{}", parts.join("\n\n")))
}

/// Build an inheritance context section for a roundtable conversation.
///
/// Priority: explicit source > anchor message > recent parent turns.
pub fn build_rt_inheritance_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    explicit_source: Option<&str>,
) -> Option<String> {
    use crate::commands::context_queries::{load_anchor_message, load_recent_messages, parent_conversation_id};

    let mut parts: Vec<String> = Vec::new();

    if let Some(source) = explicit_source {
        if !source.is_empty() {
            let truncated = truncate_str(source, 800);
            parts.push(format!("### Discussion Source\n\n{}", truncated));
        }
    }

    if parts.is_empty() {
        if let Some((_role, content)) = load_anchor_message(conn, conversation_id) {
            let truncated = truncate_str(&content, ANCHOR_MAX_CHARS);
            parts.push(format!("### Discussion Anchor\n\n{}", truncated));
        }
    }

    if let Some(parent_id) = parent_conversation_id(conn, conversation_id) {
        let recent = load_recent_messages(conn, &parent_id, RT_PARENT_RECENT);
        if !recent.is_empty() {
            parts.push(format_section("Recent parent context", &recent, 200));
        }
    }

    if parts.is_empty() {
        return None;
    }
    Some(format!("## Roundtable Context\n\n{}", parts.join("\n\n")))
}
