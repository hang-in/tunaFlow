use rusqlite::Connection;

use super::super::context_pack::build_lite_context_prompt;

/// Load the project path for a given project_key.
pub fn load_project_path(conn: &Connection, project_key: &str) -> Option<String> {
    conn.query_row(
        "SELECT path FROM projects WHERE key = ?1",
        [project_key],
        |row| row.get(0),
    )
    .ok()
    .flatten()
}

/// Build an enriched prompt with lite context prefix for non-Claude engines.
/// Retained for roundtable participant paths that don't carry full SendWithClaudeInput.
#[allow(dead_code)]
pub fn build_lite_enriched_prompt(
    conn: &Connection,
    conversation_id: &str,
    prompt: &str,
    project_path: Option<&str>,
) -> String {
    let prefix = project_path
        .map(|p| format!("Project: {}\n", p))
        .unwrap_or_default();
    format!(
        "{}{}",
        prefix,
        build_lite_context_prompt(conn, conversation_id, prompt)
    )
}

/// All data needed to assemble a ContextPack, pre-loaded from DB.
/// Separating data loading (DB-dependent) from prompt assembly (pure function)
/// enables unit testing of assembly logic and tighter DB lock scopes.
#[allow(dead_code)]
pub struct ContextData {
    pub conversation_id: String,
    pub project_path: Option<String>,
    pub prompt: String,
    pub is_branch: bool,

    // Auto mode signals
    pub has_active_plan: bool,

    // Recent context (role, content, engine, persona)
    pub current_messages: Vec<(String, String, Option<String>, Option<String>)>,
    pub parent_messages: Vec<(String, String, Option<String>, Option<String>)>,

    // Structured memory (pre-built section strings)
    pub plan_section: Option<String>,
    /// Full plan document markdown (from docs/plans/{slug}.md)
    pub plan_document: Option<String>,
    pub findings_section: Option<String>,
    pub artifacts_section: Option<String>,

    // Retrieval
    pub retrieval_chunks: Vec<crate::commands::context_queries::RetrievedChunk>,

    // Compressed memory
    pub compressed_memory: Option<String>,

    // Cross-session (label, messages)
    pub cross_session_data: Vec<(String, Vec<(String, String)>)>,

    // Thread inheritance
    pub thread_inheritance: Option<String>,

    // Agent role document (docs/agents/{role}.md content)
    pub agent_role_doc: Option<String>,

    // Pass-through (no DB needed)
    pub active_skills: Vec<String>,
    pub cross_session_ids: Vec<String>,
    pub persona_fragment: Option<String>,
    pub context_mode_override: Option<String>,
    pub context_budget_cap: Option<usize>,
}

/// Phase A: Load all data needed for ContextPack assembly from DB.
///
/// This function gathers all DB-dependent data into a `ContextData` struct.
/// The returned struct can then be passed to `assemble_prompt()` (Phase B)
/// which is a pure function with no DB dependency.
pub fn load_context_data(
    conn: &Connection,
    conversation_id: &str,
    prompt: &str,
    project_path: Option<&str>,
    active_skills: &[String],
    cross_session_ids: &[String],
    persona_fragment: Option<&str>,
    context_mode_override: Option<&str>,
    context_budget_cap: Option<usize>,
) -> ContextData {
    use super::super::context_pack::*;
    use crate::commands::context_queries::{
        load_recent_messages, load_recent_messages_with_author, conversation_label,
        retrieve_relevant_chunks_with_overlap,
    };
    use crate::commands::conversation_memory::load_compressed_memory;

    let is_branch = conversation_id.starts_with("branch:");

    // Check conversation type (scratchpad inherits main chat context)
    let conv_type: String = conn.query_row(
        "SELECT COALESCE(type, 'main') FROM conversations WHERE id = ?1",
        [conversation_id], |row| row.get(0),
    ).unwrap_or_else(|_| "main".into());
    let is_scratchpad = conv_type == "scratchpad";

    // Query 1: has_active_plan (auto mode signal) — check main chat plan for scratchpads
    let plan_lookup_conv = if is_scratchpad {
        // Find main chat for this project
        conn.query_row(
            "SELECT c2.id FROM conversations c1
             JOIN conversations c2 ON c2.project_key = c1.project_key AND c2.type = 'main'
             WHERE c1.id = ?1 LIMIT 1",
            [conversation_id], |row| row.get::<_, String>(0),
        ).ok()
    } else { None };
    let has_active_plan: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM plans WHERE conversation_id = ?1 AND status = 'active'",
        [plan_lookup_conv.as_deref().unwrap_or(conversation_id)], |row| row.get(0),
    ).unwrap_or(false);

    // Query 2: current messages — budget-based dynamic window + per-agent last-message guarantee
    let current_messages = load_recent_messages_with_author(conn, conversation_id, 20);

    // Query 3: parent messages (branch: parent conv, scratchpad: main chat)
    let parent_messages: Vec<(String, String, Option<String>, Option<String>)> = if is_branch {
        let parent_id: Option<String> = conn
            .query_row(
                "SELECT parent_id FROM conversations WHERE id = ?1",
                [conversation_id],
                |row| row.get(0),
            )
            .ok()
            .flatten();
        parent_id
            .map(|pid| load_recent_messages_with_author(conn, &pid, 4))
            .unwrap_or_default()
    } else if is_scratchpad {
        // Scratchpad inherits main chat context — load recent messages from main conversation
        plan_lookup_conv.as_ref()
            .map(|main_id| load_recent_messages_with_author(conn, main_id, 8))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // Query 4-7: plan, findings, artifacts (scratchpad: use main chat's plan)
    let effective_conv_id = if is_scratchpad {
        plan_lookup_conv.as_deref().unwrap_or(conversation_id)
    } else { conversation_id };
    let plan_conv_id = resolve_plan_conversation_id(conn, effective_conv_id);
    let mut plan_section = build_plan_section(conn, &plan_conv_id);

    // Append completed plan titles so the agent knows what was already done
    let done_plans: Vec<String> = conn.prepare(
        "SELECT title FROM plans WHERE conversation_id = ?1 AND status = 'done' ORDER BY updated_at DESC LIMIT 10"
    ).ok().map(|mut stmt| {
        stmt.query_map([plan_lookup_conv.as_deref().unwrap_or(conversation_id)], |row| row.get::<_, String>(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }).unwrap_or_default();
    if !done_plans.is_empty() {
        let suffix = format!(
            "\n\n### Completed Plans\n{}",
            done_plans.iter().map(|t| format!("- ✅ {}", t)).collect::<Vec<_>>().join("\n")
        );
        plan_section = Some(match plan_section {
            Some(s) => format!("{}{}", s, suffix),
            None => format!("## Plans{}", suffix),
        });
    }

    // Load plan documents from filesystem (plan, result, review)
    let plan_document: Option<String> = if has_active_plan {
        if let Some(pp) = project_path {
            let plan_row = conn.query_row(
                "SELECT title, phase FROM plans WHERE conversation_id = ?1 AND status = 'active' LIMIT 1",
                [plan_lookup_conv.as_deref().unwrap_or(conversation_id)],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            ).ok();
            plan_row.and_then(|(title, phase)| {
                let slug = crate::commands::plans::slugify_pub(&title);
                let plans_dir = std::path::Path::new(pp).join("docs").join("plans");
                let mut combined = String::new();

                // Plan document (always)
                if let Ok(doc) = std::fs::read_to_string(plans_dir.join(format!("{}.md", slug))) {
                    combined.push_str(&doc);
                }

                // Result report (review/rework phase — Reviewer needs implementation context)
                if phase == "review" || phase == "rework" {
                    if let Ok(doc) = std::fs::read_to_string(plans_dir.join(format!("{}-result.md", slug))) {
                        combined.push_str("\n\n---\n\n");
                        combined.push_str(&doc);
                    }
                }

                // Latest review report (rework phase — Developer needs review feedback)
                if phase == "rework" {
                    // Find latest review-r{N}.md
                    let mut round = 1;
                    let mut latest_review = None;
                    while plans_dir.join(format!("{}-review-r{}.md", slug, round)).exists() {
                        latest_review = std::fs::read_to_string(plans_dir.join(format!("{}-review-r{}.md", slug, round))).ok();
                        round += 1;
                    }
                    if let Some(doc) = latest_review {
                        combined.push_str("\n\n---\n\n");
                        combined.push_str(&doc);
                    }
                }

                if combined.is_empty() { None } else { Some(combined) }
            })
        } else { None }
    } else { None };

    let findings_section = build_findings_section(conn, &plan_conv_id);
    let artifacts_section = build_artifact_handoff_section(conn, &plan_conv_id);

    // Query 8-9: retrieval chunks (FTS5 + vector hybrid)
    let project_key: Option<String> = conn.query_row(
        "SELECT project_key FROM conversations WHERE id = ?1",
        [conversation_id],
        |row| row.get(0),
    ).ok();
    let retrieval_chunks = if let Some(pk) = &project_key {
        let recent_ids: Vec<String> = conn.prepare(
            "SELECT id FROM messages WHERE conversation_id = ?1 ORDER BY timestamp DESC LIMIT 12"
        ).ok().map(|mut stmt| {
            stmt.query_map([conversation_id], |row| row.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        }).unwrap_or_default();
        let mut fts_chunks = retrieve_relevant_chunks_with_overlap(conn, pk, conversation_id, prompt, &recent_ids, 10, None);

        // Boost with vector search results (best-effort, skip if rawq unavailable)
        if let Ok(query_emb) = crate::agents::rawq::embed_text(prompt, true) {
            let vec_results = crate::commands::vector_search::search_similar(conn, &query_emb, pk, conversation_id, 5);
            // RRF merge: add vector-only results that aren't already in FTS results
            let fts_conv_ids: std::collections::HashSet<String> = fts_chunks.iter().map(|c| c.conversation_id.clone()).collect();
            for vc in vec_results {
                if vc.score > 0.3 && !fts_conv_ids.contains(&vc.conversation_id) {
                    fts_chunks.push(crate::commands::context_queries::RetrievedChunk {
                        kind: "anchor",
                        messages: vec![("assistant".to_string(), vc.text_preview, None, None)],
                        conversation_id: vc.conversation_id,
                        score: vc.score as f64,
                        timestamp: 0,
                    });
                }
            }
        }
        fts_chunks
    } else {
        Vec::new()
    };

    // Query 10: compressed memory (scratchpad: also check main chat memory)
    let compressed_memory = load_compressed_memory(conn, conversation_id)
        .or_else(|| {
            if is_scratchpad {
                plan_lookup_conv.as_deref().and_then(|main_id| load_compressed_memory(conn, main_id))
            } else { None }
        });

    // Query 11: cross-session data (manual IDs + auto-discovered links)
    let effective_cross_ids: Vec<String> = if cross_session_ids.is_empty() {
        // No manual selection: use auto-discovered session links
        crate::commands::session_discovery::load_active_session_ids(conn, conversation_id, 3)
    } else {
        cross_session_ids.to_vec()
    };
    let cross_session_data: Vec<(String, Vec<(String, String)>)> = effective_cross_ids
        .iter()
        .filter(|id| id.as_str() != conversation_id)
        .filter_map(|id| {
            let label = conversation_label(conn, id)?;
            let rows = load_recent_messages(conn, id, 3);
            if rows.is_empty() { None } else { Some((label, rows)) }
        })
        .collect();

    // Load agent role document from project docs/agents/
    let agent_role_doc: Option<String> = project_path.and_then(|pp| {
        let agents_dir = std::path::Path::new(pp).join("docs").join("agents");
        if !agents_dir.is_dir() { return None; }

        // Determine role:
        // - Implementation branch (linked to plan) → developer
        // - Review branch → reviewer
        // - Everything else (main chat, subtask discussion branch) → architect
        let role = if is_branch {
            let branch_id = conversation_id.strip_prefix("branch:").unwrap_or("");
            let is_impl: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM plans WHERE implementation_branch_id = ?1",
                [branch_id], |row| row.get(0),
            ).unwrap_or(false);
            let is_review: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM plans WHERE review_branch_id = ?1",
                [branch_id], |row| row.get(0),
            ).unwrap_or(false);
            if is_impl { "developer" } else if is_review { "reviewer" } else { "architect" }
        } else { "architect" };

        let role_file = agents_dir.join(format!("{}.md", role));
        std::fs::read_to_string(&role_file).ok()
    });

    // Query 12: thread inheritance
    let thread_inheritance = if is_branch {
        build_thread_inheritance_section(conn, conversation_id)
    } else {
        None
    };

    ContextData {
        conversation_id: conversation_id.to_string(),
        project_path: project_path.map(|s| s.to_string()),
        prompt: prompt.to_string(),
        is_branch,
        has_active_plan,
        current_messages,
        parent_messages,
        plan_section,
        plan_document,
        findings_section,
        artifacts_section,
        retrieval_chunks,
        compressed_memory,
        cross_session_data,
        thread_inheritance,
        agent_role_doc,
        active_skills: active_skills.to_vec(),
        cross_session_ids: cross_session_ids.to_vec(),
        persona_fragment: persona_fragment.map(|s| s.to_string()),
        context_mode_override: context_mode_override.map(|s| s.to_string()),
        context_budget_cap,
    }
}
