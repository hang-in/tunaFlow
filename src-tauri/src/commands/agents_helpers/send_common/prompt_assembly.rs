use rusqlite::Connection;

use super::context_loading::{ContextData, load_context_data};
use super::super::trace_log::ContextPackMeta;
use super::super::identity::{parse_identity_and_persona, PLATFORM_TIER0};
use super::super::context_pack::ContextMode;

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

/// Phase B: Assemble a ContextPack prompt from pre-loaded data. Pure function — no DB dependency.
///
/// Takes a `ContextData` struct (from `load_context_data`) and produces the same
/// `(assembled, system_context, meta)` tuple as the original monolithic function.
pub fn assemble_prompt(
    data: &ContextData,
    identity_fragment: Option<&str>,
) -> (String, Option<String>, ContextPackMeta) {
    use super::super::context_pack::*;
    use crate::guardrail;
    use super::super::compression::maybe_compress_section_typed;

    let mut included_sections: Vec<String> = Vec::new();

    let total_budget = data.context_budget_cap.unwrap_or(guardrail::MAX_TOTAL_PROMPT);

    // Dynamic budget allocation — measure actual content, distribute proportionally
    let budget_alloc = guardrail::allocate_budgets(total_budget, &[
        guardrail::SectionBudget { name: "plan",       content_len: data.plan_section.as_ref().map_or(0, |s| s.len()),     weight: 1.0, min_chars: 500,  max_chars: guardrail::MAX_PLAN_SECTION },
        guardrail::SectionBudget { name: "plan-doc",   content_len: data.plan_document.as_ref().map_or(0, |s| s.len()),    weight: 2.0, min_chars: 1000, max_chars: 6000 },
        guardrail::SectionBudget { name: "findings",   content_len: data.findings_section.as_ref().map_or(0, |s| s.len()), weight: 1.0, min_chars: 500,  max_chars: guardrail::MAX_FINDINGS_SECTION },
        guardrail::SectionBudget { name: "artifacts",  content_len: data.artifacts_section.as_ref().map_or(0, |s| s.len()),weight: 0.8, min_chars: 300,  max_chars: guardrail::MAX_ARTIFACTS_SECTION },
        guardrail::SectionBudget { name: "skills",     content_len: if data.active_skills.is_empty() { 0 } else { 2000 },  weight: 1.0, min_chars: 500,  max_chars: guardrail::MAX_SKILLS_SECTION },
        guardrail::SectionBudget { name: "rawq",       content_len: if data.retrieval_chunks.is_empty() { 0 } else { 1000 }, weight: 0.8, min_chars: 500, max_chars: guardrail::MAX_RAWQ_SECTION },
        guardrail::SectionBudget { name: "retrieval",  content_len: data.retrieval_chunks.len() * 300,                     weight: 1.2, min_chars: 500,  max_chars: guardrail::MAX_RETRIEVAL_SECTION },
        guardrail::SectionBudget { name: "compressed", content_len: data.compressed_memory.as_ref().map_or(0, |s| s.len()),weight: 1.0, min_chars: 500,  max_chars: guardrail::MAX_COMPRESSED_MEMORY_SECTION },
        guardrail::SectionBudget { name: "cross",      content_len: if data.cross_session_data.is_empty() { 0 } else { 1000 }, weight: 0.6, min_chars: 300, max_chars: guardrail::MAX_CROSS_SESSION_SECTION },
    ]);
    let dyn_cap = |name: &str| -> usize {
        budget_alloc.iter().find(|(n, _)| *n == name).map_or(2000, |(_, c)| *c)
    };

    let (ctx_mode, auto_reason) = determine_context_mode(data);

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
            compressed_cap: 3_000,
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
            compressed_cap: 6_000,
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

    // Agent role document (docs/agents/{role}.md) — injected right after platform
    if let Some(role_doc) = &data.agent_role_doc {
        sections.push(format!("## Agent Role Instructions\n\n{}", role_doc));
        included_sections.push("agent-role".into());
    }

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
            dyn_cap("plan"),
        ) {
            sections.push(s);
            included_sections.push("plan".into());
        }
        // Plan document (full markdown from project docs/plans/)
        if let Some(doc) = &data.plan_document {
            let doc_cap = dyn_cap("plan-doc");
            let truncated = if doc.len() > doc_cap {
                let mut end = doc_cap;
                while end > 0 && !doc.is_char_boundary(end) { end -= 1; }
                format!("{}\n\n[... truncated]", &doc[..end])
            } else { doc.clone() };
            sections.push(format!("## Plan Document\n\n{}", truncated));
            included_sections.push("plan-document".into());
        }
        if let Some(s) = guardrail::truncate_section(
            data.findings_section.clone(),
            dyn_cap("findings"),
        ) {
            sections.push(s);
            included_sections.push("findings".into());
        }
        if let Some(s) = guardrail::truncate_section(
            data.artifacts_section.clone(),
            dyn_cap("artifacts"),
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
            dyn_cap("skills"),
        ) {
            sections.push(s);
            included_sections.push("skills".into());
        }
    }
    // rawq: mode-independent — prompt_needs_rawq() internally decides
    if let Some(s) = guardrail::truncate_section(
        build_rawq_section(data.project_path.as_deref(), &data.prompt),
        dyn_cap("rawq"),
    ) {
        sections.push(s);
        included_sections.push("rawq".into());
    }
    // context-hub (chops): library documentation search — best-effort, skip if unavailable
    if ctx_mode >= ContextMode::Standard {
        if let Some(s) = guardrail::truncate_section(
            build_chops_section(&data.prompt),
            2000, // max 2k chars for library docs
        ) {
            sections.push(s);
            included_sections.push("chops".into());
        }
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
/// Determine the context mode based on user override or auto heuristic.
///
/// Scoring: skills≥3 (+2), skills>0 (+1), cross_session (+1), explicit persona (+1),
/// branch (+1), active plan (+1), short prompt (-1/-2).
/// Full ≥ 3, Lite ≤ -1, Standard otherwise.
/// Words that signal the user is asking about past context (retrieval needed).
const HISTORY_SIGNAL_WORDS: &[&str] = &[
    "처음", "이전", "그때", "정리", "요약", "기억", "논의", "결정", "히스토리",
    "first", "earlier", "previous", "history", "summarize", "remember", "decided",
];

fn determine_context_mode(data: &ContextData) -> (ContextMode, &'static str) {
    match data.context_mode_override.as_deref() {
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

            // Long conversations need retrieval — never drop to Lite
            let msg_count = data.current_messages.len();
            if msg_count >= 20 && score < 0 {
                score = 0; // Floor at Standard for long conversations
                eprintln!("[auto_mode] floor: {} msgs, preventing Lite", msg_count);
            }

            // History signal words → boost to ensure retrieval is included
            let prompt_lower = data.prompt.to_lowercase();
            if HISTORY_SIGNAL_WORDS.iter().any(|w| prompt_lower.contains(w)) {
                score = score.max(1); // At least Standard
                eprintln!("[auto_mode] history signal detected in prompt");
            }

            let (mode, reason) = if score >= 3 {
                (ContextMode::Full, "auto:full(skills/cross/plan)")
            } else if score <= -1 {
                (ContextMode::Lite, "auto:lite(short-prompt)")
            } else {
                (ContextMode::Standard, "auto:standard(baseline)")
            };
            eprintln!("[auto_mode] score={} msgs={} → {:?} reason={}", score, msg_count, mode, reason);
            (mode, reason)
        }
    }
}

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
