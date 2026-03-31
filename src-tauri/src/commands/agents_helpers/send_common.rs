use rusqlite::{params, Connection};
use uuid::Uuid;

use tauri::Emitter;

use crate::db::{migrations::now_epoch_ms, models::Message};
use crate::errors::AppError;

use super::trace_log::{insert_trace_log, insert_trace_log_with_context, new_span_id, new_trace_id, SpanInfo, ContextPackMeta};
pub use super::identity::*;

/// Persist a user message if no pre-existing user_message_id was provided.
pub fn persist_user_message(
    conn: &Connection,
    conversation_id: &str,
    prompt: &str,
    user_message_id: &Option<String>,
) -> Result<(), AppError> {
    if user_message_id.is_none() {
        let id = Uuid::new_v4().to_string();
        let now = now_epoch_ms();
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, timestamp, status)
             VALUES (?1, ?2, 'user', ?3, ?4, 'done')",
            params![id, conversation_id, prompt, now],
        )?;
    }
    Ok(())
}

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
    use super::context_pack::build_lite_context_prompt;

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

    // Pass-through (no DB needed)
    pub active_skills: Vec<String>,
    pub cross_session_ids: Vec<String>,
    pub persona_fragment: Option<String>,
    pub context_mode_override: Option<String>,
    pub context_budget_cap: Option<usize>,
}

/// Build a normalized enriched prompt for non-Claude engines.
///
/// Includes the same context sections as Claude's full ContextPack, but assembled
/// into a single prompt string (non-Claude engines don't support system_prompt separation).
///
/// Sections included (same as Claude path):
/// - Project path
/// - Recent conversation context + parent context (branch)
/// - Plan / Findings / Artifacts (Standard+)
/// - Skills / rawq / cross-session (Full)
/// - Thread inheritance (branch)
#[allow(dead_code)]
pub fn build_normalized_prompt(
    conn: &Connection,
    conversation_id: &str,
    prompt: &str,
    project_path: Option<&str>,
    active_skills: &[String],
    cross_session_ids: &[String],
    persona_fragment: Option<&str>,
) -> (String, ContextPackMeta) {
    let (assembled, _, meta) = build_normalized_prompt_with_budget(conn, conversation_id, prompt, project_path, active_skills, cross_session_ids, persona_fragment, None, None);
    (assembled, meta)
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
    use super::context_pack::*;
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
    let plan_section = build_plan_section(conn, &plan_conv_id);

    // Load plan document file if active plan exists
    let plan_document: Option<String> = if has_active_plan {
        if let Some(pp) = project_path {
            // Find active plan title to build slug
            let title: Option<String> = conn.query_row(
                "SELECT title FROM plans WHERE conversation_id = ?1 AND status = 'active' LIMIT 1",
                [plan_lookup_conv.as_deref().unwrap_or(conversation_id)],
                |row| row.get(0),
            ).ok();
            title.and_then(|t| {
                let slug: String = t.chars()
                    .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
                    .collect::<String>()
                    .split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
                let doc_path = std::path::Path::new(pp).join("docs").join("plans").join(format!("{}.md", slug));
                std::fs::read_to_string(&doc_path).ok()
            })
        } else { None }
    } else { None };

    let findings_section = build_findings_section(conn, &plan_conv_id);
    let artifacts_section = build_artifact_handoff_section(conn, &plan_conv_id);

    // Query 8-9: retrieval chunks
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
        // Note: existing_context is None here (not yet assembled).
        // Overlap penalty will be slightly less precise but functionally equivalent.
        retrieve_relevant_chunks_with_overlap(conn, pk, conversation_id, prompt, &recent_ids, 10, None)
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

    // Query 11: cross-session data
    let cross_session_data: Vec<(String, Vec<(String, String)>)> = cross_session_ids
        .iter()
        .filter(|id| id.as_str() != conversation_id)
        .filter_map(|id| {
            let label = conversation_label(conn, id)?;
            let rows = load_recent_messages(conn, id, 3);
            if rows.is_empty() { None } else { Some((label, rows)) }
        })
        .collect();

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
        active_skills: active_skills.to_vec(),
        cross_session_ids: cross_session_ids.to_vec(),
        persona_fragment: persona_fragment.map(|s| s.to_string()),
        context_mode_override: context_mode_override.map(|s| s.to_string()),
        context_budget_cap,
    }
}

/// Phase B: Assemble a ContextPack prompt from pre-loaded data. Pure function — no DB dependency.
///
/// Takes a `ContextData` struct (from `load_context_data`) and produces the same
/// `(assembled, system_context, meta)` tuple as the original monolithic function.
pub fn assemble_prompt(
    data: &ContextData,
    identity_fragment: Option<&str>,
) -> (String, Option<String>, ContextPackMeta) {
    use super::context_pack::*;
    use crate::guardrail;
    use super::compression::maybe_compress_section_typed;

    let mut included_sections: Vec<String> = Vec::new();

    let total_budget = data.context_budget_cap.unwrap_or(guardrail::MAX_TOTAL_PROMPT);

    // Determine context mode — user override takes priority, then auto heuristic
    let (ctx_mode, auto_reason) = match data.context_mode_override.as_deref() {
        Some("full") => (ContextMode::Full, "user-override"),
        Some("standard") => (ContextMode::Standard, "user-override"),
        Some("lite") => (ContextMode::Lite, "user-override"),
        _ => {
            let mut score: i32 = 0;

            if data.active_skills.len() >= 3 { score += 2; }
            else if !data.active_skills.is_empty() { score += 1; }
            if !data.cross_session_ids.is_empty() { score += 1; }
            let has_explicit_persona = data.persona_fragment
                .as_ref()
                .map(|f| f.contains("## Persona"))
                .unwrap_or(false);
            if has_explicit_persona { score += 1; }
            if data.is_branch { score += 1; }
            if data.has_active_plan { score += 1; }

            if data.prompt.len() < 50 { score -= 1; }
            if data.prompt.len() < 20 { score -= 1; }

            let (mode, reason) = if score >= 3 {
                (ContextMode::Full, "auto:full(skills/cross/plan)")
            } else if score <= -1 {
                (ContextMode::Lite, "auto:lite(short-prompt)")
            } else {
                (ContextMode::Standard, "auto:standard(baseline)")
            };
            eprintln!("[auto_mode] score={} → {:?} reason={}", score, mode, reason);
            (mode, reason)
        }
    };

    // ─── Mode-specific context assembly profile ──────────────────────────
    #[allow(dead_code)]
    struct ModeProfile {
        context_cap: usize,
        retrieval_cap: usize,
        compressed_cap: usize,
        cross_session_cap: usize,
        retrieval_min_remaining: usize,
        compressed_min_remaining: usize,
        retrieval_content_max: usize,
        context_message_max: usize,
    }

    let profile = match ctx_mode {
        ContextMode::Lite => ModeProfile {
            context_cap: 4_000,
            retrieval_cap: 2_000,
            compressed_cap: 2_000,
            cross_session_cap: 2_000,
            retrieval_min_remaining: 3_000,
            compressed_min_remaining: 1_500,
            retrieval_content_max: 150,
            context_message_max: 300,
        },
        ContextMode::Standard => ModeProfile {
            context_cap: guardrail::MAX_CONTEXT_SECTION,
            retrieval_cap: guardrail::MAX_RETRIEVAL_SECTION,
            compressed_cap: guardrail::MAX_COMPRESSED_MEMORY_SECTION,
            cross_session_cap: guardrail::MAX_CROSS_SESSION_SECTION,
            retrieval_min_remaining: if total_budget >= 80_000 { 3_000 } else { 4_000 },
            compressed_min_remaining: 2_000,
            retrieval_content_max: 250,
            context_message_max: 400,
        },
        ContextMode::Full => ModeProfile {
            context_cap: 8_000,
            retrieval_cap: 6_000,
            compressed_cap: 4_000,
            cross_session_cap: 6_000,
            retrieval_min_remaining: 2_000,
            compressed_min_remaining: 1_500,
            retrieval_content_max: 400,
            context_message_max: 500,
        },
    };

    eprintln!("[context_pack] mode={:?}({}) budget={} ctx_cap={} ret_cap={} cmp_cap={}", ctx_mode, auto_reason, total_budget, profile.context_cap, profile.retrieval_cap, profile.compressed_cap);

    let mut sections: Vec<String> = Vec::new();
    let mut section_sizes: Vec<(String, usize)> = Vec::new();

    // Project
    if let Some(p) = &data.project_path {
        sections.push(format!("Project: {}", p));
        included_sections.push("project".into());
    }

    // Tier 0: tunaFlow platform instructions (always injected, minimal footprint)
    sections.push(PLATFORM_TIER0.to_string());
    included_sections.push("platform".into());

    // Identity + Persona section
    {
        let (identity_block, persona_block) = parse_identity_and_persona(identity_fragment);
        if let Some(id) = &identity_block {
            sections.push(id.clone());
            included_sections.push("identity".into());
        }
        if let Some(p) = &persona_block {
            sections.push(p.clone());
            included_sections.push("persona".into());
        }
    }

    // ─── Conversation participants meta ────────────────────────────────
    // Always-present section listing which agents participated in this conversation.
    // Ensures agents never lose awareness of other agents' presence, even when
    // individual messages fall outside the recent context window.
    {
        let mut agent_map: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
        for (role, content, engine, persona) in &data.current_messages {
            if role == "assistant" {
                let label = match (persona, engine) {
                    (Some(p), Some(e)) if !p.is_empty() => format!("{} ({})", p, e),
                    (None, Some(e)) if !e.is_empty() => format!("({})", e),
                    _ => continue,
                };
                // Keep latest content preview per agent
                let preview = if content.len() > 80 {
                    format!("{}…", &content[..content.char_indices().take_while(|&(i,_)| i<=80).last().map_or(0,|(i,_)|i)])
                } else { content.clone() };
                agent_map.insert(label, preview);
            }
        }
        if !agent_map.is_empty() {
            let mut meta_section = String::from("## Conversation participants\n\nAgents active in this conversation:\n");
            for (agent, last_msg) in &agent_map {
                meta_section.push_str(&format!("- **{}**: {}\n", agent, last_msg));
            }
            sections.push(meta_section);
            included_sections.push("participants".into());
        }
    }

    // ─── Recent conversation context (budget-based dynamic window) ───
    // Instead of a fixed 6-message window, include as many messages as fit
    // within context_cap, with per-agent last-message guarantee.
    {
        // Step 1: Identify each agent's last message index (must-include set)
        let mut agent_last_idx: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (i, (role, _, engine, persona)) in data.current_messages.iter().enumerate() {
            if role == "assistant" {
                let key = match (persona, engine) {
                    (Some(p), _) if !p.is_empty() => p.clone(),
                    (_, Some(e)) if !e.is_empty() => e.clone(),
                    _ => continue,
                };
                agent_last_idx.insert(key, i);
            }
        }
        let must_include: std::collections::HashSet<usize> = agent_last_idx.values().copied().collect();

        // Step 2: Budget-based trimming from oldest — but keep must-include messages
        let mut trimmed: Vec<&(String, String, Option<String>, Option<String>)> = Vec::new();
        let mut char_budget = profile.context_cap;

        // Work backwards (newest first) to fill budget
        for (i, msg) in data.current_messages.iter().enumerate().rev() {
            let msg_cost = msg.0.len() + msg.1.len().min(profile.context_message_max) + 40; // role + truncated content + overhead
            if msg_cost <= char_budget {
                trimmed.push(msg);
                char_budget = char_budget.saturating_sub(msg_cost);
            } else if must_include.contains(&i) {
                // Force-include agent's last message even if over budget
                trimmed.push(msg);
                char_budget = 0;
            }
            // else: skip this message (oldest first)
        }
        trimmed.reverse(); // restore chronological order

        let trimmed_owned: Vec<(String, String, Option<String>, Option<String>)> =
            trimmed.into_iter().cloned().collect();

        if let Some(ctx) = maybe_compress_section_typed(
            build_context_summary_with_authors(&trimmed_owned, &data.parent_messages, data.is_branch),
            profile.context_cap,
            Some("context"),
        ) {
            sections.push(ctx);
            included_sections.push("context".into());
        }
    }

    // ═══ UNIFIED MEMORY POLICY ═══

    // Layer 2: Structured task memory (plan / findings / artifacts)
    if ctx_mode >= ContextMode::Standard {
        if let Some(s) = guardrail::truncate_section(
            data.plan_section.clone(),
            guardrail::MAX_PLAN_SECTION,
        ) {
            sections.push(s);
            included_sections.push("plan".into());
        }
        // Plan document (full markdown from project docs/plans/)
        if let Some(doc) = &data.plan_document {
            let truncated = if doc.len() > 4000 { format!("{}\n\n[... truncated]", &doc[..4000]) } else { doc.clone() };
            sections.push(format!("## Plan Document\n\n{}", truncated));
            included_sections.push("plan-document".into());
        }
        if let Some(s) = guardrail::truncate_section(
            data.findings_section.clone(),
            guardrail::MAX_FINDINGS_SECTION,
        ) {
            sections.push(s);
            included_sections.push("findings".into());
        }
        if let Some(s) = guardrail::truncate_section(
            data.artifacts_section.clone(),
            guardrail::MAX_ARTIFACTS_SECTION,
        ) {
            sections.push(s);
            included_sections.push("artifacts".into());
        }
    }

    // Layer 3: Retrieval memory — past conversation chunks, ranked and deduped
    if ctx_mode >= ContextMode::Standard {
        let current_size: usize = sections.iter().map(|s| s.len()).sum();
        let remaining = total_budget.saturating_sub(current_size);
        if remaining > profile.retrieval_min_remaining {
            // Apply mode-specific limit to pre-loaded chunks
            let retrieval_limit = match ctx_mode {
                ContextMode::Lite => 3,
                ContextMode::Standard => 6,
                ContextMode::Full => 10,
            };
            let chunks: Vec<&crate::commands::context_queries::RetrievedChunk> =
                data.retrieval_chunks.iter().take(retrieval_limit).collect();
            if !chunks.is_empty() {
                let mut section = String::from("## Relevant prior conversation\n\nPast conversation chunks relevant to the current question.\n");
                for chunk in &chunks {
                    let kind_label = match chunk.kind { "pair" => "Q&A", "anchor" => "Branch anchor", "brief" => "RT brief", _ => chunk.kind };
                    section.push_str(&format!("\n--- {} ---\n", kind_label));
                    for (role, content, engine, persona) in &chunk.messages {
                        let author = match (role.as_str(), persona, engine) {
                            ("assistant", Some(p), Some(e)) if !p.is_empty() => format!("assistant:{} ({})", p, e),
                            ("assistant", None, Some(e)) if !e.is_empty() => format!("assistant ({})", e),
                            _ => role.clone(),
                        };
                        let truncated = if content.len() > profile.retrieval_content_max {
                            let end = content.char_indices().take_while(|&(i, _)| i <= profile.retrieval_content_max).last().map_or(0, |(i, _)| i);
                            format!("{}…", &content[..end])
                        } else { content.clone() };
                        section.push_str(&format!("[{}] {}\n", author, truncated));
                    }
                }
                if let Some(s) = guardrail::truncate_section(Some(section), profile.retrieval_cap) {
                    sections.push(s);
                    included_sections.push("retrieval".into());
                }
            }
        } else {
            eprintln!("[memory_policy] retrieval skipped — remaining {} < threshold {}", remaining, profile.retrieval_min_remaining);
            included_sections.push("retrieval:skipped".into());
        }
    }

    // Layer 4: Compressed conversation memory — continuity layer
    {
        let current_size: usize = sections.iter().map(|s| s.len()).sum();
        let remaining = total_budget.saturating_sub(current_size);
        if remaining > profile.compressed_min_remaining {
            if let Some(memory) = &data.compressed_memory {
                if let Some(s) = guardrail::truncate_section(Some(format!(
                    "## Compressed conversation memory\n\n\
                    Structured summary of older messages. For current task details, see Plan/Findings/Artifacts above.\n\n\
                    {}", memory
                )), profile.compressed_cap) {
                    sections.push(s);
                    included_sections.push("compressed-memory".into());
                }
            }
        } else {
            eprintln!("[memory_policy] compressed-memory skipped — remaining {} < threshold {}", remaining, profile.compressed_min_remaining);
            included_sections.push("compressed-memory:skipped".into());
        }
    }

    // Layer 5+: Skills, rawq, cross-session (supplementary sources)
    if ctx_mode >= ContextMode::Full || !data.active_skills.is_empty() {
        if let Some(s) = guardrail::truncate_section(
            build_skills_section(&data.active_skills, &data.prompt),
            guardrail::MAX_SKILLS_SECTION,
        ) {
            sections.push(s);
            included_sections.push("skills".into());
        }
    }
    // rawq: mode-independent — prompt_needs_rawq() internally decides
    if let Some(s) = guardrail::truncate_section(
        build_rawq_section(data.project_path.as_deref(), &data.prompt),
        guardrail::MAX_RAWQ_SECTION,
    ) {
        sections.push(s);
        included_sections.push("rawq".into());
    }
    if !data.cross_session_data.is_empty() {
        if let Some(s) = maybe_compress_section_typed(
            build_cross_session_section(&data.cross_session_data),
            profile.cross_session_cap,
            Some("cross-session"),
        ) {
            sections.push(s);
            included_sections.push("cross-session".into());
        }
    }

    // Thread inheritance (branch)
    if data.is_branch {
        if let Some(s) = &data.thread_inheritance {
            sections.push(s.clone());
            included_sections.push("thread-inheritance".into());
        }
    }

    // Build section sizes from sections + included_sections (1:1 correspondence for active ones)
    {
        let active_names: Vec<&str> = included_sections.iter()
            .filter(|s| !s.contains(":skipped"))
            .map(|s| s.as_str())
            .collect();
        for (i, name) in active_names.iter().enumerate() {
            if let Some(sec) = sections.get(i) {
                section_sizes.push((name.to_string(), sec.len()));
            }
        }
    }

    // Memory policy summary log
    {
        let total_chars: usize = sections.iter().map(|s| s.len()).sum();
        let active: Vec<&str> = included_sections.iter()
            .filter(|s| !s.contains(":skipped"))
            .map(|s| s.as_str())
            .collect();
        let skipped: Vec<&str> = included_sections.iter()
            .filter(|s| s.contains(":skipped"))
            .map(|s| s.as_str())
            .collect();
        let skipped_str = if skipped.is_empty() { "none".to_string() } else { skipped.join(",") };
        let mut sorted_sizes = section_sizes.clone();
        sorted_sizes.sort_by(|a, b| b.1.cmp(&a.1));
        let top3: Vec<String> = sorted_sizes.iter().take(3).map(|(n, s)| format!("{}={:.1}k", n, *s as f64 / 1000.0)).collect();
        eprintln!(
            "[memory_policy] budget={}/{} active=[{}] skipped=[{}] top=[{}]",
            total_chars, total_budget, active.join(","), skipped_str, top3.join(","),
        );
    }

    // Assemble final prompt
    let system_context = if sections.is_empty() {
        None
    } else {
        guardrail::enforce_total_limit(Some(sections.join("\n\n")), total_budget)
    };
    let assembled = match &system_context {
        Some(ctx) => format!("{}\n\n---\n\n{}", ctx, &data.prompt),
        None => data.prompt.clone(),
    };

    let ctx_mode_str = if auto_reason.starts_with("auto:") {
        format!("{:?}({})", ctx_mode, auto_reason)
    } else {
        format!("{:?}", ctx_mode)
    };
    let total_len = assembled.len();
    let truncated = total_len >= total_budget;

    let sizes_json = serde_json::to_string(
        &section_sizes.iter().map(|(n, s)| serde_json::json!({ "name": n, "chars": s })).collect::<Vec<_>>()
    ).unwrap_or_default();

    let meta = ContextPackMeta {
        mode: ctx_mode_str,
        sections: included_sections,
        length: total_len,
        hash: sizes_json,
        truncated,
    };

    (assembled, system_context, meta)
}

/// Legacy entry point — delegates to load_context_data + assemble_prompt.
/// Signature and return type are unchanged; no caller modifications needed.
pub fn build_normalized_prompt_with_budget(
    conn: &Connection,
    conversation_id: &str,
    prompt: &str,
    project_path: Option<&str>,
    active_skills: &[String],
    cross_session_ids: &[String],
    persona_fragment: Option<&str>,
    context_mode_override: Option<&str>,
    context_budget_cap: Option<usize>,
) -> (String, Option<String>, ContextPackMeta) {
    let data = load_context_data(
        conn, conversation_id, prompt, project_path,
        active_skills, cross_session_ids, persona_fragment,
        context_mode_override, context_budget_cap,
    );
    assemble_prompt(&data, persona_fragment)
}

// ═══════════════════════════════════════════════════════════════════════════
// Shared Phase 1 (prepare) + Phase 3 (finalize) for start_* commands
// ═══════════════════════════════════════════════════════════════════════════

use crate::db::DbState;

/// Output from Phase 1: everything needed to run the engine in a background thread.
pub struct PreparedRun {
    pub msg_id: String,
    pub job_id: String,
    pub enriched_prompt: String,       // context + prompt (for non-Claude engines)
    pub system_context: Option<String>, // context only (for Claude system_prompt)
    pub project_path: Option<String>,
    pub ctx_meta: ContextPackMeta,
}

/// Phase 1: Persist user message, build context, pre-create streaming message, create job.
///
/// Returns PreparedRun with everything needed for the background engine thread.
/// DB lock is acquired and released within this function.
pub fn prepare_engine_run(
    engine_key: &str,
    input: &super::super::agents::SendWithClaudeInput,
    identity_frag: Option<&str>,
    state: &DbState,
) -> Result<PreparedRun, crate::errors::AppError> {
    // Phase A: DB operations under lock — persist user msg, load context data, pre-create streaming msg
    let (data, project_path, msg_id) = {
        let conn = state.write.lock().map_err(|_| crate::errors::AppError::Lock)?;
        persist_user_message(&conn, &input.conversation_id, &input.prompt, &input.user_message_id)?;
        let pp = load_project_path(&conn, &input.project_key);
        let ctx_data = load_context_data(
            &conn, &input.conversation_id, &input.prompt, pp.as_deref(),
            &input.active_skills, &input.cross_session_ids, identity_frag,
            input.context_mode_override.as_deref(), input.context_budget_cap,
        );
        let mid = Uuid::new_v4().to_string();
        let now = now_epoch_ms();
        conn.execute(
            "INSERT INTO messages(id,conversation_id,role,content,timestamp,status,engine,model,persona)\
             VALUES(?1,?2,'assistant','',?3,'streaming',?4,?5,?6)",
            params![mid, input.conversation_id, now, engine_key, input.model, input.persona_label],
        )?;
        (ctx_data, pp, mid)
        // lock released here
    };

    // Phase B: Pure prompt assembly — no DB lock held
    let (enriched_prompt, system_context, ctx_meta) = assemble_prompt(&data, identity_frag);

    let job_id = format!("job-{}", Uuid::new_v4());
    {
        let conn = state.write.lock().map_err(|_| crate::errors::AppError::Lock)?;
        let _ = super::super::jobs::create_job(&conn, &job_id, &input.conversation_id, Some(&msg_id), engine_key, "agent");
    }

    Ok(PreparedRun { msg_id, job_id, enriched_prompt, system_context, project_path, ctx_meta })
}

/// Phase 3: Persist engine result, update conversation usage, emit events.
///
/// Called from the background thread after the engine finishes.
pub fn finalize_engine_run(
    conn: &Connection,
    engine_key: &str,
    msg_id: &str,
    conversation_id: &str,
    job_id: &str,
    result: &Result<crate::agents::claude::RunOutput, crate::errors::AppError>,
    duration_ms: u128,
    ctx_meta: &ContextPackMeta,
    app: &tauri::AppHandle,
) {
    let now = now_epoch_ms();
    match result {
        Ok(out) => {
            let content = if out.content.is_empty() {
                format!("({} returned no output)", engine_key)
            } else {
                out.content.clone()
            };
            let _ = conn.execute(
                "UPDATE messages SET content=?1,status='done',timestamp=?2 WHERE id=?3",
                params![content, now, msg_id],
            );
            // Update conversation usage (tokens + cost + resume token for claude)
            if out.cost_usd > 0.0 {
                let _ = conn.execute(
                    "UPDATE conversations SET total_input_tokens=total_input_tokens+?1,\
                     total_output_tokens=total_output_tokens+?2,total_cost_usd=total_cost_usd+?3,\
                     updated_at=?4 WHERE id=?5",
                    params![out.input_tokens, out.output_tokens, out.cost_usd, now / 1000, conversation_id],
                );
            } else {
                let _ = conn.execute(
                    "UPDATE conversations SET total_input_tokens=total_input_tokens+?1,\
                     total_output_tokens=total_output_tokens+?2,updated_at=?3 WHERE id=?4",
                    params![out.input_tokens, out.output_tokens, now / 1000, conversation_id],
                );
            }
            // Claude-specific: save resume token
            if let Some(ref sid) = out.session_id {
                let _ = conn.execute(
                    "UPDATE conversations SET resume_token=?1,\
                     resume_token_engine=CASE WHEN ?1 IS NOT NULL THEN ?2 ELSE resume_token_engine END WHERE id=?3",
                    params![sid, engine_key, conversation_id],
                );
            }
            insert_trace_log_with_context(conn, conversation_id, out.input_tokens, out.output_tokens, out.cost_usd, now,
                &SpanInfo { trace_id: &new_trace_id(), span_id: new_span_id(), parent_span_id: None,
                    operation: "agent.stream", engine: engine_key, duration_ms: duration_ms as i64, status: "ok" },
                ctx_meta);
            let _ = super::super::jobs::complete_job(conn, job_id, "done", None);
            let _ = app.emit("agent:completed", serde_json::json!({
                "messageId": msg_id, "conversationId": conversation_id, "engine": engine_key
            }));
        }
        Err(ref e) => {
            let em = crate::guardrail::fallback_error(engine_key, e);
            let _ = conn.execute(
                "UPDATE messages SET content=?1,status='error',timestamp=?2 WHERE id=?3",
                params![em, now, msg_id],
            );
            let _ = super::super::jobs::complete_job(conn, job_id, "error", Some(&em));
            let _ = app.emit("agent:error", serde_json::json!({
                "messageId": msg_id, "conversationId": conversation_id, "engine": engine_key, "error": em
            }));
        }
    }
}

/// Result from an agent run, before DB persistence.
#[allow(dead_code)]
pub struct AgentRunResult {
    pub content: String,
    pub status: String,
    pub cost_usd: f64,
    pub in_tokens: i64,
    pub out_tokens: i64,
}

/// Persist assistant message and update conversation usage.
/// Returns the constructed Message for the Tauri response.
/// If `ctx_meta` is provided, records context metadata in trace_log.
#[allow(dead_code)]
pub fn persist_assistant_message(
    conn: &Connection,
    conversation_id: &str,
    engine: &str,
    model: &Option<String>,
    run: &AgentRunResult,
    duration_ms: u128,
    ctx_meta: Option<&ContextPackMeta>,
) -> Result<Message, AppError> {
    let msg_id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();

    conn.execute(
        "INSERT INTO messages
         (id, conversation_id, role, content, timestamp, status, engine, model)
         VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6, ?7)",
        params![
            msg_id,
            conversation_id,
            run.content,
            now,
            run.status,
            engine,
            model,
        ],
    )?;

    // Update conversation usage — full version (tokens + cost)
    if run.in_tokens > 0 || run.out_tokens > 0 || run.cost_usd > 0.0 {
        conn.execute(
            "UPDATE conversations SET
                 total_input_tokens  = total_input_tokens  + ?1,
                 total_output_tokens = total_output_tokens + ?2,
                 total_cost_usd      = total_cost_usd      + ?3,
                 updated_at          = ?4
             WHERE id = ?5",
            params![
                run.in_tokens,
                run.out_tokens,
                run.cost_usd,
                now / 1000,
                conversation_id,
            ],
        )?;
    } else {
        // Lightweight update — just touch updated_at
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now / 1000, conversation_id],
        )?;
    }

    let span = SpanInfo {
        trace_id: &new_trace_id(),
        span_id: new_span_id(),
        parent_span_id: None,
        operation: "agent.send",
        engine,
        duration_ms: duration_ms as i64,
        status: if run.status == "done" { "ok" } else { "error" },
    };
    if let Some(meta) = ctx_meta {
        insert_trace_log_with_context(conn, conversation_id, run.in_tokens, run.out_tokens, run.cost_usd, now, &span, meta);
    } else {
        insert_trace_log(conn, conversation_id, run.in_tokens, run.out_tokens, run.cost_usd, now, &span);
    }

    Ok(Message {
        id: msg_id,
        conversation_id: conversation_id.to_string(),
        role: "assistant".into(),
        content: run.content.clone(),
        timestamp: now,
        status: run.status.clone(),
        progress_content: None,
        engine: Some(engine.into()),
        model: model.clone(),
        persona: None,
    })
}

/// Same as `persist_assistant_message` but uses a pre-generated message ID.
/// Required for streaming commands where the ID is emitted to the frontend before DB persist.
#[allow(dead_code)]
pub fn persist_assistant_message_with_id(
    conn: &Connection,
    msg_id: &str,
    conversation_id: &str,
    engine: &str,
    model: &Option<String>,
    run: &AgentRunResult,
    duration_ms: u128,
    ctx_meta: Option<&ContextPackMeta>,
) -> Result<Message, AppError> {
    let now = now_epoch_ms();

    conn.execute(
        "INSERT INTO messages
         (id, conversation_id, role, content, timestamp, status, engine, model)
         VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6, ?7)",
        params![msg_id, conversation_id, run.content, now, run.status, engine, model],
    )?;

    if run.in_tokens > 0 || run.out_tokens > 0 || run.cost_usd > 0.0 {
        conn.execute(
            "UPDATE conversations SET
                 total_input_tokens  = total_input_tokens  + ?1,
                 total_output_tokens = total_output_tokens + ?2,
                 total_cost_usd      = total_cost_usd      + ?3,
                 updated_at          = ?4
             WHERE id = ?5",
            params![run.in_tokens, run.out_tokens, run.cost_usd, now / 1000, conversation_id],
        )?;
    } else {
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now / 1000, conversation_id],
        )?;
    }

    let span = SpanInfo {
        trace_id: &new_trace_id(),
        span_id: new_span_id(),
        parent_span_id: None,
        operation: "agent.send",
        engine,
        duration_ms: duration_ms as i64,
        status: if run.status == "done" { "ok" } else { "error" },
    };
    if let Some(meta) = ctx_meta {
        insert_trace_log_with_context(conn, conversation_id, run.in_tokens, run.out_tokens, run.cost_usd, now, &span, meta);
    } else {
        insert_trace_log(conn, conversation_id, run.in_tokens, run.out_tokens, run.cost_usd, now, &span);
    }

    Ok(Message {
        id: msg_id.to_string(),
        conversation_id: conversation_id.to_string(),
        role: "assistant".into(),
        content: run.content.clone(),
        timestamp: now,
        status: run.status.clone(),
        progress_content: None,
        engine: Some(engine.into()),
        model: model.clone(),
        persona: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── assemble_prompt (pure function) ────────────────────────────────

    fn empty_context_data() -> ContextData {
        ContextData {
            conversation_id: "test-conv".into(),
            project_path: Some("/tmp/test".into()),
            prompt: "hello".into(),
            is_branch: false,
            has_active_plan: false,
            current_messages: vec![],
            parent_messages: vec![],
            plan_section: None,
            plan_document: None,
            findings_section: None,
            artifacts_section: None,
            retrieval_chunks: vec![],
            compressed_memory: None,
            cross_session_data: vec![],
            thread_inheritance: None,
            active_skills: vec![],
            cross_session_ids: vec![],
            persona_fragment: None,
            context_mode_override: None,
            context_budget_cap: None,
        }
    }

    #[test]
    fn assemble_empty_data_returns_prompt_only() {
        let data = empty_context_data();
        let (assembled, _sys_ctx, meta) = assemble_prompt(&data, None);
        assert!(assembled.contains("hello"));
        // project section should be present
        assert!(meta.sections.contains(&"project".to_string()));
    }

    #[test]
    fn assemble_with_plan_includes_plan_section() {
        let mut data = empty_context_data();
        data.plan_section = Some("## Active Plan\n\n### Migration\n\n**Progress:** 2/5 done".into());
        data.context_mode_override = Some("standard".into());
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"plan".to_string()));
    }

    #[test]
    fn auto_mode_short_prompt_selects_lite() {
        let mut data = empty_context_data();
        data.prompt = "ㅇㅇ".into();
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.mode.contains("Lite"), "expected Lite mode, got: {}", meta.mode);
    }

    #[test]
    fn auto_mode_with_skills_pushes_toward_full() {
        let mut data = empty_context_data();
        data.active_skills = vec!["a".into(), "b".into(), "c".into()]; // +2
        data.cross_session_ids = vec!["other-conv".into()];            // +1  → total ≥ 3
        data.prompt = "코드를 리팩토링해주세요. 이 함수가 너무 길어요.".into();
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.mode.contains("Full"), "expected Full mode, got: {}", meta.mode);
    }
}
