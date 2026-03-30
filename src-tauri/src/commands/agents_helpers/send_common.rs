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
    build_normalized_prompt_with_budget(conn, conversation_id, prompt, project_path, active_skills, cross_session_ids, persona_fragment, None, None)
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
) -> (String, ContextPackMeta) {
    use super::context_pack::*;
    use crate::guardrail;
    use super::compression::maybe_compress_section_typed;

    let is_branch = conversation_id.starts_with("branch:");
    let mut included_sections: Vec<String> = Vec::new();

    let total_budget = context_budget_cap.unwrap_or(guardrail::MAX_TOTAL_PROMPT);

    // Determine context mode — user override takes priority, then auto logic
    let ctx_mode = match context_mode_override {
        Some("full") => ContextMode::Full,
        Some("standard") => ContextMode::Standard,
        Some("lite") => ContextMode::Lite,
        _ => {
            // Auto: determine from conversation state
            if is_branch {
                ContextMode::Standard
            } else if !active_skills.is_empty() {
                ContextMode::Full
            } else {
                ContextMode::Lite
            }
        }
    };
    eprintln!("[context_pack] mode={:?} budget={} for normalized_prompt", ctx_mode, total_budget);

    let mut sections: Vec<String> = Vec::new();

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
            guardrail::MAX_CONTEXT_SECTION,
            Some("context"),
        ) {
            sections.push(ctx);
            included_sections.push("context".into());
        }
    }

    // Compressed conversation memory (long-term continuity)
    {
        use crate::commands::conversation_memory::load_compressed_memory;
        if let Some(memory) = load_compressed_memory(conn, conversation_id) {
            if let Some(s) = guardrail::truncate_section(Some(format!("## Compressed conversation memory\n\nThis is a structured summary of older messages in this conversation that are no longer in the recent window.\n\n{}", memory)), guardrail::MAX_CONTEXT_SECTION) {
                sections.push(s);
                included_sections.push("compressed-memory".into());
            }
        }
    }

    // Standard+ sections: plan, findings, artifacts
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

    // Full sections: skills, rawq, cross-session
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
            guardrail::MAX_CROSS_SESSION_SECTION,
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

    // Assemble final prompt
    let assembled = if sections.is_empty() {
        prompt.to_string()
    } else {
        let context = sections.join("\n\n");
        let limited = guardrail::enforce_total_limit(Some(context), total_budget)
            .unwrap_or_default();
        format!("{}\n\n---\n\n{}", limited, prompt)
    };

    let ctx_mode_str = format!("{:?}", ctx_mode);
    let total_len = assembled.len();
    let truncated = total_len >= total_budget;

    let meta = ContextPackMeta {
        mode: ctx_mode_str,
        sections: included_sections,
        length: total_len,
        hash: String::new(), // skip hash for now
        truncated,
    };

    (assembled, meta)
}

/// Result from an agent run, before DB persistence.
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
