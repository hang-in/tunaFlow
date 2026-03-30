use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::{migrations::now_epoch_ms, models::Message};
use crate::errors::AppError;

use super::trace_log::{insert_trace_log, insert_trace_log_with_context, new_span_id, new_trace_id, SpanInfo, ContextPackMeta};

/// Build a combined identity + persona fragment for prompt assembly.
///
/// The identity framing block ensures agents consistently identify themselves
/// using the profile/engine/persona hierarchy (profile first, engine second).
pub fn build_identity_persona_fragment(
    profile_label: Option<&str>,
    engine: &str,
    persona_fragment: Option<&str>,
) -> Option<String> {
    let identity = build_identity_block(profile_label, engine);
    match persona_fragment {
        Some(pf) if !pf.trim().is_empty() => {
            Some(format!("{}\n\n{}", identity, pf.trim()))
        }
        _ => Some(identity),
    }
}

fn build_identity_block(profile_label: Option<&str>, engine: &str) -> String {
    let profile_line = match profile_label {
        Some(label) if !label.is_empty() => format!("당신의 프로필 이름은 \"{}\"입니다.", label),
        _ => "프로필이 지정되지 않았습니다.".to_string(),
    };
    format!(
        "## Identity\n\n\
        {}\n\
        실행 엔진은 {}입니다.\n\n\
        자기소개 규칙:\n\
        - 사용자에게 보이는 1급 이름은 프로필 이름입니다. 자기소개는 프로필 기준으로 시작하세요.\n\
        - 엔진은 필요할 때만 2순위 정보로 설명하세요.\n\
        - persona는 역할/정책 정보이며, 자기 이름처럼 답하지 마세요.\n\
        - 사용자가 다른 이름으로 부르면 짧게 정정하세요.\n\
        - 혼합 표현(예: \"Claude Code(opencode)\")을 사용하지 마세요.\n\
        - 사용자의 언어에 맞춰 응답하세요.\n\n\
        메시지 작성자 규칙:\n\
        - 대화 기록에서 각 assistant 메시지는 작성자가 표시되어 있습니다(예: [assistant:ProfileName (engine)]).\n\
        - 당신이 작성하지 않은 메시지의 소유권을 주장하지 마세요.\n\
        - 사용자가 과거 답변의 작성자를 물으면, 표시된 작성자 정보를 기준으로 답하세요.\n\
        - 작성자가 불분명한 메시지는 추측하지 말고 \"작성자 정보가 없습니다\"라고 답하세요.",
        profile_line, engine
    )
}

/// Parse identity metadata from the combined persona_fragment.
/// Returns (identity_section, persona_section).
fn parse_identity_and_persona(fragment: Option<&str>) -> (Option<String>, Option<String>) {
    match fragment {
        Some(f) if !f.trim().is_empty() => {
            // Check if fragment starts with "## Identity" (injected by build_identity_persona_fragment)
            if f.contains("## Identity") {
                // Split at the persona boundary if exists
                if let Some(pos) = f.find("\n\n## Persona") {
                    let identity = f[..pos].trim().to_string();
                    let persona = f[pos..].trim().to_string();
                    (Some(identity), if persona.is_empty() { None } else { Some(persona) })
                } else {
                    // Identity block only, no persona
                    (Some(f.trim().to_string()), None)
                }
            } else {
                // Legacy: plain persona fragment without identity
                (None, Some(format!("## Persona\n\n{}", f.trim())))
            }
        }
        _ => (None, None),
    }
}

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
    use super::context_pack::*;
    use crate::guardrail;
    use super::compression::maybe_compress_section_typed;

    let is_branch = conversation_id.starts_with("branch:");
    let mut included_sections: Vec<String> = Vec::new();

    let total_budget = context_budget_cap.unwrap_or(guardrail::MAX_TOTAL_PROMPT);

    // Determine context mode — user override takes priority, then auto heuristic
    let (ctx_mode, auto_reason) = match context_mode_override {
        Some("full") => (ContextMode::Full, "user-override"),
        Some("standard") => (ContextMode::Standard, "user-override"),
        Some("lite") => (ContextMode::Lite, "user-override"),
        _ => {
            // Auto heuristic: score signals to decide Lite / Standard / Full
            // Each signal pushes toward heavier modes. Default baseline = Standard.
            let mut score: i32 = 0; // 0 = Standard baseline

            // Signals pushing toward Full (+)
            if active_skills.len() >= 3 { score += 2; }           // many skills → Full territory
            else if !active_skills.is_empty() { score += 1; }     // 1-2 skills → moderate push
            if !cross_session_ids.is_empty() { score += 1; }       // cross-session → multi-conv work
            // Only count explicit persona (not the always-present identity block)
            let has_explicit_persona = persona_fragment
                .map(|f| f.contains("## Persona"))
                .unwrap_or(false);
            if has_explicit_persona { score += 1; }                 // persona set → structured task
            if is_branch { score += 1; }                            // branch → deeper work

            // Check if structured memory exists (plan/findings/artifacts)
            let has_plan: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM plans WHERE conversation_id = ?1 AND status = 'active'",
                [conversation_id], |row| row.get(0),
            ).unwrap_or(false);
            if has_plan { score += 1; }

            // Signals pushing toward Lite (-)
            if prompt.len() < 50 { score -= 1; }                   // short prompt → likely simple
            if prompt.len() < 20 { score -= 1; }                   // very short → Lite territory

            // Map score to mode
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
    // Each mode defines: section caps, thresholds, and content resolution.
    // Lite = focused (fewer sources, lower resolution)
    // Standard = balanced (default)
    // Full = rich (more permissive, higher resolution)
    #[allow(dead_code)]
    struct ModeProfile {
        context_cap: usize,
        retrieval_cap: usize,
        compressed_cap: usize,
        cross_session_cap: usize,
        retrieval_min_remaining: usize,
        compressed_min_remaining: usize,
        retrieval_content_max: usize,  // per-chunk content truncation
        context_message_max: usize,    // per-message truncation in context summary
    }

    let profile = match ctx_mode {
        ContextMode::Lite => ModeProfile {
            context_cap: 4_000,
            retrieval_cap: 2_000,
            compressed_cap: 2_000,
            cross_session_cap: 2_000,
            retrieval_min_remaining: 3_000,  // lowered from 6k — Lite still needs continuity
            compressed_min_remaining: 1_500, // lowered from 3k — allow memory in tight budgets
            retrieval_content_max: 150,      // raised from 120 — 120 chars loses intent
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
    if let Some(p) = project_path {
        sections.push(format!("Project: {}", p));
        included_sections.push("project".into());
    }

    // Identity + Persona section
    // Identity framing is always injected regardless of persona selection.
    // It uses persona_fragment format: first line = "profile:{label}|engine:{name}" metadata
    // (injected by callers via build_identity_persona_fragment)
    {
        let (identity_block, persona_block) = parse_identity_and_persona(persona_fragment);
        if let Some(id) = &identity_block {
            sections.push(id.clone());
            included_sections.push("identity".into());
        }
        if let Some(p) = &persona_block {
            sections.push(p.clone());
            included_sections.push("persona".into());
        }
    }

    // Recent conversation context (with author attribution)
    {
        use crate::commands::context_queries::load_recent_messages_with_author;
        let current = load_recent_messages_with_author(conn, conversation_id, 6);
        let parent: Vec<(String, String, Option<String>, Option<String>)> = if is_branch {
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
        } else {
            Vec::new()
        };
        if let Some(ctx) = maybe_compress_section_typed(
            build_context_summary_with_authors(&current, &parent, is_branch),
            profile.context_cap,
            Some("context"),
        ) {
            sections.push(ctx);
            included_sections.push("context".into());
        }
    }

    // ═══ UNIFIED MEMORY POLICY ═══
    // Priority (high to low): explicit handoff → recent → structured → retrieval → compressed → memo/cross-session
    // Budget fallback: lower priority layers yield first.
    // Overlap: structured > retrieval > compressed. Duplicates are suppressed by later layers.

    // Layer 2: Structured task memory (plan / findings / artifacts) — highest task relevance
    if ctx_mode >= ContextMode::Standard {
        let plan_conv_id = resolve_plan_conversation_id(conn, conversation_id);
        if let Some(s) = guardrail::truncate_section(
            build_plan_section(conn, &plan_conv_id),
            guardrail::MAX_PLAN_SECTION,
        ) {
            sections.push(s);
            included_sections.push("plan".into());
        }
        if let Some(s) = guardrail::truncate_section(
            build_findings_section(conn, &plan_conv_id),
            guardrail::MAX_FINDINGS_SECTION,
        ) {
            sections.push(s);
            included_sections.push("findings".into());
        }
        if let Some(s) = guardrail::truncate_section(
            build_artifact_handoff_section(conn, &plan_conv_id),
            guardrail::MAX_ARTIFACTS_SECTION,
        ) {
            sections.push(s);
            included_sections.push("artifacts".into());
        }
    }

    // Layer 3: Retrieval memory — past conversation chunks, ranked and deduped
    // Placed AFTER structured (plan/findings/artifacts) but BEFORE compressed memory.
    // Budget-aware: skip if remaining budget is tight.
    if ctx_mode >= ContextMode::Standard {
        let current_size: usize = sections.iter().map(|s| s.len()).sum();
        let remaining = total_budget.saturating_sub(current_size);
        if remaining > profile.retrieval_min_remaining {
            let project_key: Option<String> = conn.query_row(
                "SELECT project_key FROM conversations WHERE id = ?1",
                [conversation_id],
                |row| row.get(0),
            ).ok();
            if let Some(pk) = &project_key {
                use crate::commands::context_queries::retrieve_relevant_chunks_with_overlap;
                let existing_snapshot = sections.join(" ");
                let recent_ids: Vec<String> = conn.prepare(
                    "SELECT id FROM messages WHERE conversation_id = ?1 ORDER BY timestamp DESC LIMIT 12"
                ).ok().map(|mut stmt| {
                    stmt.query_map([conversation_id], |row| row.get::<_, String>(0))
                        .map(|rows| rows.filter_map(|r| r.ok()).collect())
                        .unwrap_or_default()
                }).unwrap_or_default();

                let retrieval_limit = match ctx_mode {
                    ContextMode::Lite => 3,
                    ContextMode::Standard => 6,
                    ContextMode::Full => 10,
                };
                let chunks = retrieve_relevant_chunks_with_overlap(conn, pk, conversation_id, prompt, &recent_ids, retrieval_limit, Some(&existing_snapshot));
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
            }
        } else {
            eprintln!("[memory_policy] retrieval skipped — remaining {} < threshold {}", remaining, profile.retrieval_min_remaining);
            included_sections.push("retrieval:skipped".into());
        }
    }

    // Layer 4: Compressed conversation memory — continuity layer
    // Lowest priority among memory layers. Yields first when budget is tight.
    {
        let current_size: usize = sections.iter().map(|s| s.len()).sum();
        let remaining = total_budget.saturating_sub(current_size);
        if remaining > profile.compressed_min_remaining {
            use crate::commands::conversation_memory::load_compressed_memory;
            if let Some(memory) = load_compressed_memory(conn, conversation_id) {
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
    if ctx_mode >= ContextMode::Full || !active_skills.is_empty() {
        if let Some(s) = guardrail::truncate_section(
            build_skills_section(active_skills),
            guardrail::MAX_SKILLS_SECTION,
        ) {
            sections.push(s);
            included_sections.push("skills".into());
        }
    }
    // rawq: mode-independent — prompt_needs_rawq() internally decides
    if let Some(s) = guardrail::truncate_section(
        build_rawq_section(project_path, prompt),
        guardrail::MAX_RAWQ_SECTION,
    ) {
        sections.push(s);
        included_sections.push("rawq".into());
    }
    if !cross_session_ids.is_empty() {
        use crate::commands::context_queries::{load_recent_messages, conversation_label};
        let cross_data: Vec<(String, Vec<(String, String)>)> = cross_session_ids
            .iter()
            .filter(|id| id.as_str() != conversation_id)
            .filter_map(|id| {
                let label = conversation_label(conn, id)?;
                let rows = load_recent_messages(conn, id, 3);
                if rows.is_empty() { None } else { Some((label, rows)) }
            })
            .collect();
        if let Some(s) = maybe_compress_section_typed(
            build_cross_session_section(&cross_data),
            profile.cross_session_cap,
            Some("cross-session"),
        ) {
            sections.push(s);
            included_sections.push("cross-session".into());
        }
    }

    // Thread inheritance (branch)
    if is_branch {
        if let Some(s) = build_thread_inheritance_section(conn, conversation_id) {
            sections.push(s);
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
        // Log top consumers
        let mut sorted_sizes = section_sizes.clone();
        sorted_sizes.sort_by(|a, b| b.1.cmp(&a.1));
        let top3: Vec<String> = sorted_sizes.iter().take(3).map(|(n, s)| format!("{}={:.1}k", n, *s as f64 / 1000.0)).collect();
        eprintln!(
            "[memory_policy] budget={}/{} active=[{}] skipped=[{}] top=[{}]",
            total_chars, total_budget, active.join(","), skipped_str, top3.join(","),
        );
    }

    // Assemble final prompt
    // system_context: context sections only (for engines with system_prompt separation, e.g. Claude)
    let system_context = if sections.is_empty() {
        None
    } else {
        guardrail::enforce_total_limit(Some(sections.join("\n\n")), total_budget)
    };
    let assembled = match &system_context {
        Some(ctx) => format!("{}\n\n---\n\n{}", ctx, prompt),
        None => prompt.to_string(),
    };

    // Include auto reason in mode string for trace readability (e.g., "Standard(auto:standard(baseline))")
    let ctx_mode_str = if auto_reason.starts_with("auto:") {
        format!("{:?}({})", ctx_mode, auto_reason)
    } else {
        format!("{:?}", ctx_mode)
    };
    let total_len = assembled.len();
    let truncated = total_len >= total_budget;

    // Encode section sizes as JSON in the hash field for trace observability
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

    #[test]
    fn identity_with_profile_and_persona() {
        let result = build_identity_persona_fragment(
            Some("Architect Claude"), "claude-code", Some("You are a reviewer"),
        ).unwrap();
        assert!(result.contains("## Identity"));
        assert!(result.contains("Architect Claude"));
        assert!(result.contains("claude-code"));
        assert!(result.contains("You are a reviewer"));
    }

    #[test]
    fn identity_without_persona() {
        let result = build_identity_persona_fragment(
            Some("General"), "opencode", None,
        ).unwrap();
        assert!(result.contains("## Identity"));
        assert!(result.contains("General"));
    }

    #[test]
    fn identity_without_profile() {
        let result = build_identity_persona_fragment(
            None, "gemini", None,
        ).unwrap();
        assert!(result.contains("프로필이 지정되지 않았습니다"));
        assert!(result.contains("gemini"));
    }

    #[test]
    fn parse_identity_only() {
        let fragment = "## Identity\n\nYour profile is Test.\nEngine: claude.";
        let (id, persona) = parse_identity_and_persona(Some(fragment));
        assert!(id.is_some());
        assert!(persona.is_none());
    }

    #[test]
    fn parse_identity_and_persona_split() {
        let fragment = "## Identity\n\nProfile: Test\n\n## Persona\n\nYou are a reviewer.";
        let (id, persona) = parse_identity_and_persona(Some(fragment));
        assert!(id.unwrap().contains("Identity"));
        assert!(persona.unwrap().contains("reviewer"));
    }

    #[test]
    fn parse_legacy_persona_only() {
        let fragment = "You are a code reviewer.";
        let (id, persona) = parse_identity_and_persona(Some(fragment));
        assert!(id.is_none());
        assert!(persona.unwrap().contains("## Persona"));
    }

    #[test]
    fn parse_none_fragment() {
        let (id, persona) = parse_identity_and_persona(None);
        assert!(id.is_none());
        assert!(persona.is_none());
    }

    #[test]
    fn identity_block_has_attribution_rules() {
        let block = build_identity_block(Some("Test"), "claude");
        assert!(block.contains("메시지 작성자 규칙"));
        assert!(block.contains("소유권을 주장하지 마세요"));
    }

    #[test]
    fn identity_block_user_language() {
        let block = build_identity_block(Some("Test"), "claude");
        assert!(block.contains("사용자의 언어에 맞춰"));
    }
}
