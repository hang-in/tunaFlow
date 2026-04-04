use rusqlite::params;

use super::utils::{fold_similar_blocks, format_section, format_section_with_authors, truncate_str};

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

/// Words to skip when building chops search query.
#[allow(dead_code)]
const CHOPS_SKIP_WORDS: &[&str] = &[
    "the", "this", "that", "with", "from", "have", "been",
    "will", "would", "could", "should", "about", "into",
    "구현", "수정", "변경", "추가", "삭제", "확인", "진행",
    "해주세요", "합니다", "입니다", "있습니다", "없습니다",
];

/// Build skills section with selective injection.
///
/// Instead of injecting full SKILL.md content, splits each skill by `## ` headers
/// and only includes sections whose header or content matches keywords from the prompt.
/// Unmatched sections are replaced with a compact reference: "[SkillName: N sections omitted]".
pub fn build_skills_section(skill_names: &[String], prompt: &str) -> Option<String> {
    if skill_names.is_empty() {
        return None;
    }

    // Extract keywords from prompt (≥3 chars, lowercased, unique)
    let keywords: Vec<String> = prompt
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 3)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let mut skill_blocks = Vec::new();
    for name in skill_names {
        if let Ok(skill) = crate::commands::skills::get_skill(name.clone()) {
            let block = extract_relevant_skill_sections(&skill.name, &skill.content, &keywords);
            skill_blocks.push(block);
        }
    }
    if skill_blocks.is_empty() {
        return None;
    }
    Some(format!("## Active skills\n\n{}", skill_blocks.join("\n\n")))
}

/// Extract only relevant sections from a skill's markdown content.
///
/// Splits by `## ` headers, checks each section for keyword matches,
/// includes matching sections and summarizes omitted ones.
fn extract_relevant_skill_sections(skill_name: &str, content: &str, keywords: &[String]) -> String {
    // Split content by ## headers
    let mut sections: Vec<(&str, &str)> = Vec::new(); // (header, body)
    let mut current_header = "";
    let mut current_start = 0;

    for (i, line) in content.lines().enumerate() {
        if line.starts_with("## ") {
            if i > 0 {
                let body = &content[current_start..content.lines().take(i).map(|l| l.len() + 1).sum::<usize>().saturating_sub(1)];
                sections.push((current_header, body.trim()));
            }
            current_header = line;
            current_start = content.lines().take(i).map(|l| l.len() + 1).sum::<usize>();
        }
    }
    // Last section
    let remaining = &content[current_start..];
    sections.push((current_header, remaining.trim()));

    // If no headers found (flat content) or keywords empty → include everything
    if sections.len() <= 1 || keywords.is_empty() {
        return format!("### {}\n\n{}", skill_name, truncate_str(content, 2000));
    }

    let mut included = Vec::new();
    let mut omitted = 0;

    for (header, body) in &sections {
        let combined = format!("{} {}", header.to_lowercase(), body.to_lowercase());
        let matches = keywords.iter().any(|kw| combined.contains(kw.as_str()));
        if matches || header.is_empty() {
            // Include this section (truncate individual sections to prevent bloat)
            included.push(format!("{}\n{}", header, truncate_str(body, 800)));
        } else {
            omitted += 1;
        }
    }

    let mut result = format!("### {}\n\n", skill_name);
    if !included.is_empty() {
        result.push_str(&included.join("\n\n"));
    }
    if omitted > 0 {
        result.push_str(&format!("\n\n[{}: {} section{} omitted]", skill_name, omitted, if omitted > 1 { "s" } else { "" }));
    }
    result
}

/// Build a code-review-graph section — detect changes + impact radius.
/// Returns None if code-review-graph is unavailable or project has no graph.
pub fn build_crg_section(project_path: &str) -> Option<String> {
    if !crate::agents::crg::is_available() {
        return None;
    }

    // detect-changes: risk-scored change analysis
    let changes = crate::agents::crg::detect_changes(project_path, "HEAD~1").ok()?;
    let summary = changes.get("summary").and_then(|v| v.as_str()).unwrap_or("");
    if summary.is_empty() || summary.contains("No changes") {
        return None;
    }

    let mut out = String::from("## Code change impact (code-review-graph)\n\n");
    out.push_str(summary);

    // Add impacted files list if available
    if let Some(files) = changes.get("files").and_then(|v| v.as_array()) {
        if !files.is_empty() {
            out.push_str("\n\n**Risk-scored files**:\n");
            for (i, f) in files.iter().take(10).enumerate() {
                if let Some(name) = f.get("file").and_then(|v| v.as_str()) {
                    let score = f.get("risk_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    out.push_str(&format!("{}. {} (risk: {:.1})\n", i + 1, name, score));
                }
            }
        }
    }

    eprintln!("[context_pack] crg: change impact section built ({} chars)", out.len());
    Some(out)
}

/// Build a context-hub (chops) section from automatic keyword search.
///
/// Calls `context_hub::search()` with keywords extracted from the prompt.
/// Returns None if context-hub is unavailable or no results found.
/// Graceful degradation: never blocks or errors — just skips.
#[allow(dead_code)]
pub fn build_chops_section(prompt: &str) -> Option<String> {
    // Extract meaningful keywords for search (reuse same logic as rawq)
    let keywords: Vec<&str> = prompt
        .split_whitespace()
        .filter(|w| w.len() >= 3)
        .filter(|w| !CHOPS_SKIP_WORDS.contains(&w.to_lowercase().as_str()))
        .take(6)
        .collect();

    if keywords.is_empty() {
        return None;
    }

    let query = keywords.join(" ");
    match crate::agents::context_hub::search(&query, None, 3) {
        Ok(results) => {
            if results.is_empty() {
                return None;
            }
            let mut out = String::from("## Library documentation (context-hub)\n\n");
            for r in &results {
                out.push_str(&format!(
                    "### {} ({})\n{}\n\n",
                    r.title,
                    r.source,
                    truncate_str(&r.snippet, 500),
                ));
            }
            eprintln!("[context_pack] chops: {} results for \"{}\"", results.len(), query);
            Some(out.trim_end().to_string())
        }
        Err(_) => {
            // context-hub not available or search failed — skip silently
            None
        }
    }
}

pub fn build_cross_session_section(
    cross_session: &[(String, Vec<(String, String)>)],
) -> Option<String> {
    if cross_session.is_empty() {
        return None;
    }
    let mut blocks = Vec::new();
    for (label, rows) in cross_session {
        if rows.is_empty() {
            continue;
        }
        let mut block = format!("### {}\n", label);
        for (role, content) in rows {
            block.push_str(&format!("\n[{}] {}\n", role, truncate_str(content, 200)));
        }
        blocks.push(block);
    }
    // Fold near-duplicate cross-session blocks
    fold_similar_blocks(&mut blocks);
    if blocks.is_empty() {
        return None;
    }
    Some(format!("## Cross-session context\n\n{}", blocks.join("\n")))
}

#[allow(dead_code)]
pub fn build_context_summary(
    current_rows: &[(String, String)],
    parent_rows: &[(String, String)],
    is_branch: bool,
) -> Option<String> {
    let has_current = !current_rows.is_empty();
    let has_parent = !parent_rows.is_empty();

    if !has_current && !has_parent {
        return None;
    }

    let mut parts: Vec<String> = Vec::new();

    if has_parent {
        parts.push(format_section("Parent conversation context", parent_rows, 300));
    }

    if has_current {
        let header = if is_branch {
            "Branch conversation history (you are continuing this conversation)"
        } else {
            "Conversation history (you are continuing this conversation — refer to these messages as your own prior responses)"
        };
        parts.push(format_section(header, current_rows, 400));
    }

    Some(parts.join("\n"))
}

/// Build context summary with per-message author attribution.
///
/// Each assistant message shows `[assistant:ProfileName (engine)]` so the model
/// can distinguish which agent authored each past message.
pub fn build_context_summary_with_authors(
    current_rows: &[(String, String, Option<String>, Option<String>)],
    parent_rows: &[(String, String, Option<String>, Option<String>)],
    is_branch: bool,
) -> Option<String> {
    let has_current = !current_rows.is_empty();
    let has_parent = !parent_rows.is_empty();

    if !has_current && !has_parent {
        return None;
    }

    let mut parts: Vec<String> = Vec::new();

    if has_parent {
        parts.push(format_section_with_authors("Parent conversation context", parent_rows, 300));
    }

    if has_current {
        let header = if is_branch {
            "Branch conversation history (each assistant message shows its author — do not claim other agents' messages as your own)"
        } else {
            "Conversation history (each assistant message shows its author — you are continuing this conversation, but do not claim messages authored by other agents as your own)"
        };
        parts.push(format_section_with_authors(header, current_rows, 400));
    }

    Some(parts.join("\n"))
}

/// Build `## Recent Artifacts` from the most recent approved/draft artifacts.
///
/// Takes up to 3 recent artifacts for the conversation, showing title, type,
/// status, and a short content preview. Returns None if no artifacts exist.
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

/// Build `## Recent Agent Findings` from the most recent roundtable_brief memos.
///
/// Queries briefs from:
/// 1. The conversation itself
/// 2. Any RT branch shadow conversations (`branch:{id}` where branch belongs to this conversation)
///
/// This ensures RT branch results are visible in the parent conversation's ContextPack.
pub fn build_findings_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<String> {
    // Briefs from this conversation + its branch shadow conversations
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
        // Take first 600 chars of each brief to keep section short
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

pub fn build_plan_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<String> {
    let plan: (String, String, Option<String>) = conn
        .query_row(
            "SELECT id, title, description FROM plans
             WHERE conversation_id = ?1 AND status = 'active'
             ORDER BY updated_at DESC LIMIT 1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok()?;

    let (plan_id, title, description) = plan;

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

    let mut out = format!("## Active Plan\n\n### {}\n", title);
    if let Some(desc) = &description {
        if !desc.is_empty() {
            out.push_str(&format!("{}\n", desc));
        }
    }

    // Subtask status list with indices + parallel group info
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
                    // Parse JSON array of indices
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

    // Parent context for branch conversations
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

// ─── Thread / RT inheritance helpers ─────────────────────────────────────────

/// Build an inheritance context section for a thread (branch) conversation.
///
/// Priority: anchor message > recent parent turns.
/// Does NOT include full parent history — only checkpoint + last N turns.
pub fn build_thread_inheritance_section(
    conn: &rusqlite::Connection,
    branch_conv_id: &str,
) -> Option<String> {
    use crate::commands::context_queries::{load_anchor_message, load_recent_messages, parent_conversation_id};

    let mut parts: Vec<String> = Vec::new();

    // 1. Anchor message (the message this branch was created from)
    if let Some((_role, content)) = load_anchor_message(conn, branch_conv_id) {
        let truncated = truncate_str(&content, ANCHOR_MAX_CHARS);
        parts.push(format!("### Thread Anchor\n\nThis thread was started from the following message:\n{}", truncated));
    }

    // 2. Recent parent turns (2-3)
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
/// More concise than thread inheritance.
pub fn build_rt_inheritance_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    explicit_source: Option<&str>,
) -> Option<String> {
    use crate::commands::context_queries::{load_anchor_message, load_recent_messages, parent_conversation_id};

    let mut parts: Vec<String> = Vec::new();

    // 1. Explicit source (highest priority)
    if let Some(source) = explicit_source {
        if !source.is_empty() {
            let truncated = truncate_str(source, 800);
            parts.push(format!("### Discussion Source\n\n{}", truncated));
        }
    }

    // 2. Anchor message (if no explicit source, or as supplementary)
    if parts.is_empty() {
        if let Some((_role, content)) = load_anchor_message(conn, conversation_id) {
            let truncated = truncate_str(&content, ANCHOR_MAX_CHARS);
            parts.push(format!("### Discussion Anchor\n\n{}", truncated));
        }
    }

    // 3. Recent parent turns (1-2, concise)
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

#[cfg(test)]
mod tests {
    use super::*;

    // ─── build_context_summary ───────────────────────────────────────────
    #[test]
    fn context_summary_empty_inputs() {
        assert_eq!(build_context_summary(&[], &[], false), None);
    }

    #[test]
    fn context_summary_current_only() {
        let current = vec![("user".into(), "hello".into())];
        let result = build_context_summary(&current, &[], false).unwrap();
        assert!(result.contains("Conversation history"));
        assert!(result.contains("hello"));
    }

    #[test]
    fn context_summary_parent_only() {
        let parent = vec![("assistant".into(), "response".into())];
        let result = build_context_summary(&[], &parent, false).unwrap();
        assert!(result.contains("Parent conversation context"));
    }

    #[test]
    fn context_summary_branch_mode() {
        let current = vec![("user".into(), "msg".into())];
        let result = build_context_summary(&current, &[], true).unwrap();
        assert!(result.contains("Branch conversation history"));
    }

    #[test]
    fn context_summary_both() {
        let current = vec![("user".into(), "cur".into())];
        let parent = vec![("user".into(), "par".into())];
        let result = build_context_summary(&current, &parent, true).unwrap();
        assert!(result.contains("Parent conversation context"));
        assert!(result.contains("Branch conversation history"));
    }

    // ─── build_cross_session_section ─────────────────────────────────────
    #[test]
    fn cross_session_empty() {
        assert_eq!(build_cross_session_section(&[]), None);
    }

    #[test]
    fn cross_session_with_data() {
        let data = vec![
            ("Session A".into(), vec![("user".into(), "question".into())]),
        ];
        let result = build_cross_session_section(&data).unwrap();
        assert!(result.contains("Cross-session context"));
        assert!(result.contains("Session A"));
    }

    // ─── build_context_summary_with_authors ─────────────────────────────
    #[test]
    fn context_summary_authors_attribution_header() {
        let current = vec![
            ("user".into(), "hi".into(), None, None),
            ("assistant".into(), "hello".into(), Some("claude-code".into()), Some("Arch".into())),
        ];
        let result = build_context_summary_with_authors(&current, &[], false).unwrap();
        assert!(result.contains("do not claim messages authored by other agents"));
        assert!(result.contains("[assistant:Arch (claude-code)]"));
    }
}
