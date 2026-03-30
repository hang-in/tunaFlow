use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::{migrations::now_epoch_ms, models::Message};
use crate::errors::AppError;

use super::trace_log::{insert_trace_log, new_span_id, new_trace_id, SpanInfo, ContextPackMeta};

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
pub fn build_normalized_prompt(
    conn: &Connection,
    conversation_id: &str,
    prompt: &str,
    project_path: Option<&str>,
    active_skills: &[String],
    cross_session_ids: &[String],
    persona_fragment: Option<&str>,
) -> (String, ContextPackMeta) {
    use super::context_pack::*;
    use crate::guardrail;
    use super::compression::maybe_compress_section;

    let is_branch = conversation_id.starts_with("branch:");
    let mut included_sections: Vec<String> = Vec::new();

    // Determine context mode — same logic as Claude path
    let ctx_mode = if is_branch {
        ContextMode::Standard
    } else if !active_skills.is_empty() {
        ContextMode::Full
    } else {
        ContextMode::Lite
    };
    eprintln!("[context_pack] mode={:?} for normalized_prompt", ctx_mode);

    let mut sections: Vec<String> = Vec::new();

    // Project
    if let Some(p) = project_path {
        sections.push(format!("Project: {}", p));
        included_sections.push("project".into());
    }

    // Persona section (role contract)
    if let Some(fragment) = persona_fragment {
        if !fragment.trim().is_empty() {
            sections.push(format!("## Persona\n\n{}", fragment.trim()));
            included_sections.push("persona".into());
        }
    }

    // Recent conversation context
    {
        use crate::commands::context_queries::load_recent_messages;
        let current = load_recent_messages(conn, conversation_id, 6);
        let parent: Vec<(String, String)> = if is_branch {
            let parent_id: Option<String> = conn
                .query_row(
                    "SELECT parent_id FROM conversations WHERE id = ?1",
                    [conversation_id],
                    |row| row.get(0),
                )
                .ok()
                .flatten();
            parent_id
                .map(|pid| load_recent_messages(conn, &pid, 4))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        if let Some(ctx) = maybe_compress_section(
            build_context_summary(&current, &parent, is_branch),
            guardrail::MAX_CONTEXT_SECTION,
        ) {
            sections.push(ctx);
            included_sections.push("context".into());
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
        if let Some(s) = maybe_compress_section(
            build_cross_session_section(&cross_data),
            guardrail::MAX_CROSS_SESSION_SECTION,
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
        let limited = guardrail::enforce_total_limit(Some(context), guardrail::MAX_TOTAL_PROMPT)
            .unwrap_or_default();
        format!("{}\n\n---\n\n{}", limited, prompt)
    };

    let ctx_mode_str = format!("{:?}", ctx_mode);
    let total_len = assembled.len();
    let truncated = total_len >= guardrail::MAX_TOTAL_PROMPT;

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
pub fn persist_assistant_message(
    conn: &Connection,
    conversation_id: &str,
    engine: &str,
    model: &Option<String>,
    run: &AgentRunResult,
    duration_ms: u128,
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

    insert_trace_log(
        conn,
        conversation_id,
        run.in_tokens,
        run.out_tokens,
        run.cost_usd,
        now,
        &SpanInfo {
            trace_id: &new_trace_id(),
            span_id: new_span_id(),
            parent_span_id: None,
            operation: "agent.send",
            engine,
            duration_ms: duration_ms as i64,
            status: if run.status == "done" { "ok" } else { "error" },
        },
    );

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

    insert_trace_log(
        conn,
        conversation_id,
        run.in_tokens,
        run.out_tokens,
        run.cost_usd,
        now,
        &SpanInfo {
            trace_id: &new_trace_id(),
            span_id: new_span_id(),
            parent_span_id: None,
            operation: "agent.send",
            engine,
            duration_ms: duration_ms as i64,
            status: if run.status == "done" { "ok" } else { "error" },
        },
    );

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
