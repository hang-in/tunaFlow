use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::agents::{claude, codex, gemini, opencode};
use crate::db::{migrations::now_epoch_ms, models::Message, DbState};
use crate::errors::AppError;
use crate::guardrail;

// Submodule re-exports — ContextPack assembly, compression, and trace logging
use super::agents_helpers::context_pack::{
    assemble_system_prompt, build_context_summary, build_cross_session_section,
    build_artifact_handoff_section, build_findings_section,
    build_plan_section, build_rawq_section, build_skills_section,
    build_thread_inheritance_section,
    combine_prompt_parts, resolve_plan_conversation_id, ContextMode,
};
use super::agents_helpers::compression::maybe_compress_section_typed;
use super::agents_helpers::trace_log::{insert_trace_log_with_context, new_span_id, new_trace_id, SpanInfo, ContextPackMeta};
use super::jobs;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkPayload {
    pub message_id: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendWithClaudeInput {
    pub project_key: String,
    pub conversation_id: String,
    pub user_message_id: Option<String>,
    pub prompt: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub active_skills: Vec<String>,
    #[serde(default)]
    pub cross_session_ids: Vec<String>,
    #[serde(default)]
    pub persona_fragment: Option<String>,
    #[serde(default)]
    pub persona_label: Option<String>,
    /// Context mode override: "lite", "standard", "full", or null (auto)
    #[serde(default)]
    pub context_mode_override: Option<String>,
    /// Total context budget cap override (chars). null = use default (60000)
    #[serde(default)]
    pub context_budget_cap: Option<usize>,
}

const CONTEXT_MESSAGES_LIMIT: i64 = 6;
const PARENT_CONTEXT_MESSAGES_LIMIT: i64 = 4;
const CROSS_SESSION_MESSAGES_LIMIT: i64 = 3;

/// Wrap persona_fragment with identity framing block for a given engine.
fn identity_fragment(input: &SendWithClaudeInput, engine: &str) -> Option<String> {
    super::agents_helpers::send_common::build_identity_persona_fragment(
        input.persona_label.as_deref(),
        engine,
        input.persona_fragment.as_deref(),
    )
}

/// Send a one-shot request to the local `claude` CLI and persist the result.
///
/// Flow:
///   1. Persist user message (if no user_message_id provided)
///   2. Load ResumeToken from conversations — discard if engine mismatch
///   3. Release DB lock
///   4. Spawn claude subprocess (may take seconds/minutes)
///   5. Re-acquire DB lock, persist assistant message + update usage + save new token
#[tauri::command]
pub fn send_with_claude(
    input: SendWithClaudeInput,
    state: State<DbState>,
) -> Result<Message, AppError> {
    // Step 1: load context + persist user message + load resume token + project path (single lock)
    let is_branch = input.conversation_id.starts_with("branch:");
    let (resume_token, project_path, current_context, parent_context, cross_session_data, plan_section, findings_section, artifacts_section, thread_inheritance) = {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;

        use super::context_queries::{load_recent_messages, conversation_label};

        let current_context = load_recent_messages(&conn, &input.conversation_id, CONTEXT_MESSAGES_LIMIT);

        let parent_context: Vec<(String, String)> = if is_branch {
            let parent_id: Option<String> = conn
                .query_row(
                    "SELECT parent_id FROM conversations WHERE id = ?1",
                    [&input.conversation_id],
                    |row| row.get(0),
                )
                .ok()
                .flatten();
            parent_id
                .map(|pid| load_recent_messages(&conn, &pid, PARENT_CONTEXT_MESSAGES_LIMIT))
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let cross_session_data: Vec<(String, Vec<(String, String)>)> = input
            .cross_session_ids
            .iter()
            .filter(|id| **id != input.conversation_id)
            .filter_map(|conv_id| {
                let label = conversation_label(&conn, conv_id)?;
                let rows = load_recent_messages(&conn, conv_id, CROSS_SESSION_MESSAGES_LIMIT);
                if rows.is_empty() { None } else { Some((label, rows)) }
            })
            .collect();

        let plan_conv_id = resolve_plan_conversation_id(&conn, &input.conversation_id);
        let plan_section = guardrail::truncate_section(
            build_plan_section(&conn, &plan_conv_id),
            guardrail::MAX_PLAN_SECTION,
        );
        let findings_section = guardrail::truncate_section(
            build_findings_section(&conn, &plan_conv_id),
            guardrail::MAX_FINDINGS_SECTION,
        );
        let artifacts_section = guardrail::truncate_section(
            build_artifact_handoff_section(&conn, &plan_conv_id),
            guardrail::MAX_ARTIFACTS_SECTION,
        );

        // Thread inheritance: anchor message + recent parent turns for branches
        let thread_inheritance = if is_branch {
            build_thread_inheritance_section(&conn, &input.conversation_id)
        } else {
            None
        };

        // 1c. Persist new user message
        super::agents_helpers::send_common::persist_user_message(&conn, &input.conversation_id, &input.prompt, &input.user_message_id)?;

        // Load stored token; discard if engine differs (DATA_MODEL §1.8 lifecycle)
        let token_result: rusqlite::Result<(Option<String>, Option<String>)> = conn.query_row(
            "SELECT resume_token, resume_token_engine FROM conversations WHERE id = ?1",
            [&input.conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );
        let resume_token = match token_result {
            Ok((Some(token), Some(engine))) if engine == "claude-code" => Some(token),
            _ => None,
        };

        // Load project path for ContextPack assembly (DATA_MODEL §4.2)
        let project_path: Option<String> = conn
            .query_row(
                "SELECT path FROM projects WHERE key = ?1",
                [&input.project_key],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        (resume_token, project_path, current_context, parent_context, cross_session_data, plan_section, findings_section, artifacts_section, thread_inheritance)
        // Lock released here
    };

    // Step 1b: assemble ContextPack — mode-based (lite/standard/full)
    // Branch stream → Standard (plan/findings needed for branch work)
    // Agent prompt or system prompt set → Standard (agent-directed task)
    // Otherwise → Lite (fast default for simple questions)
    let ctx_mode = if is_branch || input.agent_name.is_some() || input.system_prompt.is_some() {
        ContextMode::Standard
    } else {
        ContextMode::Lite
    };
    eprintln!("[context_pack] mode={:?} for send_with_claude", ctx_mode);

    let project_context = project_path.as_deref().map(|p| {
        format!("## Project\n\nYou are working on a project located at: `{}`\nAll file paths and code references are relative to this directory.", p)
    });
    let base_system_prompt = assemble_system_prompt(
        input.agent_name.as_deref(),
        project_path.as_deref(),
        input.system_prompt.as_deref(),
    );
    let context_summary = maybe_compress_section_typed(
        build_context_summary(&current_context, &parent_context, is_branch),
        guardrail::MAX_CONTEXT_SECTION,
        Some("context"),
    );

    // Standard+ 섹션 (plan, findings, artifacts)
    let (plan_s, findings_s, artifacts_s) = if ctx_mode >= ContextMode::Standard {
        (plan_section, findings_section, artifacts_section)
    } else {
        (None, None, None)
    };

    // Skills + cross-session (Full mode or active skills present)
    let (skills_s, cross_s) = if ctx_mode >= ContextMode::Full || !input.active_skills.is_empty() {
        (
            guardrail::truncate_section(build_skills_section(&input.active_skills), guardrail::MAX_SKILLS_SECTION),
            maybe_compress_section_typed(build_cross_session_section(&cross_session_data), guardrail::MAX_CROSS_SESSION_SECTION, Some("cross-session")),
        )
    } else {
        (None, None)
    };

    // rawq: mode-independent — prompt_needs_rawq() internally decides
    let rawq_s = guardrail::truncate_section(
        build_rawq_section(project_path.as_deref(), &input.prompt),
        guardrail::MAX_RAWQ_SECTION,
    );

    // Identity section
    let identity_s = identity_fragment(&input, "claude-code");

    // Track included sections for trace metadata
    let mut ctx_sections: Vec<String> = Vec::new();
    if identity_s.is_some() { ctx_sections.push("identity".into()); }
    if project_context.is_some() { ctx_sections.push("project".into()); }
    if context_summary.is_some() { ctx_sections.push("context".into()); }
    if plan_s.is_some() { ctx_sections.push("plan".into()); }
    if findings_s.is_some() { ctx_sections.push("findings".into()); }
    if artifacts_s.is_some() { ctx_sections.push("artifacts".into()); }
    if skills_s.is_some() { ctx_sections.push("skills".into()); }
    if rawq_s.is_some() { ctx_sections.push("rawq".into()); }
    if cross_s.is_some() { ctx_sections.push("cross-session".into()); }
    if thread_inheritance.is_some() { ctx_sections.push("thread-inheritance".into()); }

    let system_prompt = guardrail::enforce_total_limit(
        combine_prompt_parts([identity_s, project_context, base_system_prompt, plan_s, findings_s, artifacts_s, skills_s, rawq_s, cross_s, thread_inheritance.clone(), context_summary]),
        guardrail::MAX_TOTAL_PROMPT,
    );

    // Step 2: run claude subprocess in background thread — prevents UI freeze
    let prompt_len = input.prompt.len() + system_prompt.as_ref().map_or(0, |s| s.len());
    let ctx_meta = ContextPackMeta {
        mode: format!("{:?}", ctx_mode),
        sections: ctx_sections,
        length: prompt_len,
        hash: String::new(),
        truncated: prompt_len >= guardrail::MAX_TOTAL_PROMPT,
    };
    let t0 = std::time::Instant::now();
    let run_input = claude::RunInput {
        prompt: input.prompt.clone(),
        model: input.model.clone(),
        system_prompt,
        resume_token,
        project_path: project_path.clone(),
    };
    let run_result = std::thread::spawn(move || claude::run(run_input))
        .join()
        .unwrap_or_else(|_| Err(AppError::Agent("claude thread panicked".into())));
    let duration_ms = t0.elapsed().as_millis();
    guardrail::log_run("claude-code", input.model.as_deref(), duration_ms, prompt_len, run_result.is_ok());

    let (content, status, cost_usd, in_tokens, out_tokens, new_token) = match run_result {
        Ok(out) => (
            out.content,
            "done".to_string(),
            out.cost_usd,
            out.input_tokens,
            out.output_tokens,
            out.session_id,
        ),
        Err(ref e) => (guardrail::fallback_error("claude-code", e), "error".to_string(), 0.0, 0, 0, None),
    };

    // Step 3: persist assistant message, update usage, save new resume token
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let msg_id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();

    conn.execute(
        "INSERT INTO messages
         (id, conversation_id, role, content, timestamp, status, engine, model)
         VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, 'claude-code', ?6)",
        params![
            msg_id,
            input.conversation_id,
            content,
            now,
            status,
            input.model,
        ],
    )?;

    conn.execute(
        "UPDATE conversations SET
             total_input_tokens  = total_input_tokens  + ?1,
             total_output_tokens = total_output_tokens + ?2,
             total_cost_usd      = total_cost_usd      + ?3,
             updated_at          = ?4,
             resume_token        = ?5,
             resume_token_engine = CASE WHEN ?5 IS NOT NULL THEN 'claude-code' ELSE resume_token_engine END
         WHERE id = ?6",
        params![
            in_tokens,
            out_tokens,
            cost_usd,
            now / 1000,
            new_token,
            input.conversation_id,
        ],
    )?;

    insert_trace_log_with_context(&conn, &input.conversation_id, in_tokens, out_tokens, cost_usd, now, &SpanInfo {
        trace_id: &new_trace_id(),
        span_id: new_span_id(),
        parent_span_id: None,
        operation: "agent.send",
        engine: "claude-code",
        duration_ms: duration_ms as i64,
        status: if status == "done" { "ok" } else { "error" },
    }, &ctx_meta);

    Ok(Message {
        id: msg_id,
        conversation_id: input.conversation_id,
        role: "assistant".into(),
        content,
        timestamp: now,
        status,
        progress_content: None,
        engine: Some("claude-code".into()),
        model: input.model,
        persona: None,
    })
}

/// Send a one-shot request to the local `codex` CLI and persist the result.
///
/// Same flow as `send_with_claude` but uses `codex::run`.
/// Full ContextPack not supported by codex — uses lite context prefix instead.
#[tauri::command]
pub fn send_with_codex(
    input: SendWithClaudeInput,
    state: State<DbState>,
) -> Result<Message, AppError> {
    use super::agents_helpers::send_common::*;

    // Step 1: persist user message + build normalized context
    let id_frag = identity_fragment(&input, "codex");
    let (enriched_prompt, project_path, ctx_meta) = {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;
        persist_user_message(&conn, &input.conversation_id, &input.prompt, &input.user_message_id)?;
        let pp = load_project_path(&conn, &input.project_key);
        let (prompt, meta) = build_normalized_prompt_with_budget(&conn, &input.conversation_id, &input.prompt, pp.as_deref(), &input.active_skills, &input.cross_session_ids, id_frag.as_deref(), input.context_mode_override.as_deref(), input.context_budget_cap);
        (prompt, pp, meta)
    };

    // Step 2: run codex subprocess in background thread — prevents UI freeze
    let t0 = std::time::Instant::now();
    let run_input = claude::RunInput {
        prompt: enriched_prompt,
        model: input.model.clone(),
        system_prompt: None,
        resume_token: None,
        project_path,
    };
    let run_result = std::thread::spawn(move || codex::run(run_input))
        .join()
        .unwrap_or_else(|_| Err(AppError::Agent("codex thread panicked".into())));
    let duration_ms = t0.elapsed().as_millis();
    guardrail::log_run("codex", input.model.as_deref(), duration_ms, input.prompt.len(), run_result.is_ok());

    let run = match run_result {
        Ok(out) if out.content.is_empty() => AgentRunResult {
            content: "(codex returned no output)".to_string(), status: "done".to_string(),
            cost_usd: 0.0, in_tokens: 0, out_tokens: 0,
        },
        Ok(out) => AgentRunResult {
            content: out.content, status: "done".to_string(),
            cost_usd: out.cost_usd, in_tokens: out.input_tokens, out_tokens: out.output_tokens,
        },
        Err(ref e) => AgentRunResult {
            content: guardrail::fallback_error("codex", e), status: "error".to_string(),
            cost_usd: 0.0, in_tokens: 0, out_tokens: 0,
        },
    };

    // Step 3: persist assistant message + update usage
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    persist_assistant_message(&conn, &input.conversation_id, "codex", &input.model, &run, duration_ms, Some(&ctx_meta))
}

/// Send a one-shot request to the local `gemini` CLI and persist the result.
///
/// Full ContextPack not supported by gemini — uses lite context prefix instead.
#[tauri::command]
pub fn send_with_gemini(
    input: SendWithClaudeInput,
    state: State<DbState>,
) -> Result<Message, AppError> {
    use super::agents_helpers::send_common::*;

    // Step 1: persist user message + build normalized context
    let id_frag = identity_fragment(&input, "gemini");
    let (enriched_prompt, project_path, ctx_meta) = {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;
        persist_user_message(&conn, &input.conversation_id, &input.prompt, &input.user_message_id)?;
        let pp = load_project_path(&conn, &input.project_key);
        let (prompt, meta) = build_normalized_prompt_with_budget(&conn, &input.conversation_id, &input.prompt, pp.as_deref(), &input.active_skills, &input.cross_session_ids, id_frag.as_deref(), input.context_mode_override.as_deref(), input.context_budget_cap);
        (prompt, pp, meta)
    };

    // Step 2: run gemini subprocess in background thread — prevents UI freeze
    let t0 = std::time::Instant::now();
    let run_input = claude::RunInput {
        prompt: enriched_prompt,
        model: input.model.clone(),
        system_prompt: None,
        resume_token: None,
        project_path,
    };
    let run_result = std::thread::spawn(move || gemini::run(run_input))
        .join()
        .unwrap_or_else(|_| Err(AppError::Agent("gemini thread panicked".into())));
    let duration_ms = t0.elapsed().as_millis();
    guardrail::log_run("gemini", input.model.as_deref(), duration_ms, input.prompt.len(), run_result.is_ok());

    let run = match run_result {
        Ok(out) if out.content.is_empty() => AgentRunResult {
            content: "(gemini returned no output)".to_string(), status: "done".to_string(),
            cost_usd: 0.0, in_tokens: 0, out_tokens: 0,
        },
        Ok(out) => AgentRunResult {
            content: out.content, status: "done".to_string(),
            cost_usd: 0.0, in_tokens: 0, out_tokens: 0,
        },
        Err(ref e) => AgentRunResult {
            content: guardrail::fallback_error("gemini", e), status: "error".to_string(),
            cost_usd: 0.0, in_tokens: 0, out_tokens: 0,
        },
    };

    // Step 3: persist assistant message
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    persist_assistant_message(&conn, &input.conversation_id, "gemini", &input.model, &run, duration_ms, Some(&ctx_meta))
}

/// Send a streaming request to the local `gemini` CLI and persist the result.
///
/// Uses `--output-format stream-json` for real-time streaming.
/// Emits `gemini:progress` and `gemini:chunk` events during execution.
#[tauri::command]
pub fn stream_with_gemini(
    input: SendWithClaudeInput,
    app: AppHandle,
    state: State<DbState>,
    cancel: State<crate::CancelRegistry>,
) -> Result<Message, AppError> {
    use super::agents_helpers::send_common::*;

    // Step 1: persist user message + build normalized context
    let id_frag = identity_fragment(&input, "gemini");
    let (enriched_prompt, project_path, ctx_meta) = {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;
        persist_user_message(&conn, &input.conversation_id, &input.prompt, &input.user_message_id)?;
        let pp = load_project_path(&conn, &input.project_key);
        let (prompt, meta) = build_normalized_prompt_with_budget(&conn, &input.conversation_id, &input.prompt, pp.as_deref(), &input.active_skills, &input.cross_session_ids, id_frag.as_deref(), input.context_mode_override.as_deref(), input.context_budget_cap);
        (prompt, pp, meta)
    };

    // Step 2: create placeholder message ID
    let msg_id = format!("msg-{}", Uuid::new_v4());

    // Step 3: run streaming subprocess
    let t0 = std::time::Instant::now();
    let chunk_msg_id = msg_id.clone();
    let progress_msg_id = msg_id.clone();
    let progress_app = app.clone();
    let run_result = gemini::stream_run(
        claude::RunInput {
            prompt: enriched_prompt,
            model: input.model.clone(),
            system_prompt: None,
            resume_token: None,
            project_path,
        },
        |progress_text| {
            let _ = progress_app.emit(
                "gemini:progress",
                ChunkPayload {
                    message_id: progress_msg_id.clone(),
                    text: progress_text,
                },
            );
        },
        |text| {
            let _ = app.emit(
                "gemini:chunk",
                ChunkPayload {
                    message_id: chunk_msg_id.clone(),
                    text,
                },
            );
        },
        {
            let conv_id = input.conversation_id.clone();
            let registry = std::sync::Arc::clone(&cancel.0);
            move || {
                if let Ok(mut set) = registry.lock() {
                    set.remove(&conv_id)
                } else {
                    false
                }
            }
        },
    );
    let duration_ms = t0.elapsed().as_millis();
    guardrail::log_run("gemini-stream", input.model.as_deref(), duration_ms, input.prompt.len(), run_result.is_ok());

    let run = match run_result {
        Ok(out) if out.content.is_empty() => AgentRunResult {
            content: "(gemini returned no output)".to_string(), status: "done".to_string(),
            cost_usd: 0.0, in_tokens: out.input_tokens, out_tokens: out.output_tokens,
        },
        Ok(out) => AgentRunResult {
            content: out.content, status: "done".to_string(),
            cost_usd: 0.0, in_tokens: out.input_tokens, out_tokens: out.output_tokens,
        },
        Err(ref e) => AgentRunResult {
            content: guardrail::fallback_error("gemini", e), status: "error".to_string(),
            cost_usd: 0.0, in_tokens: 0, out_tokens: 0,
        },
    };

    // Step 4: persist assistant message
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    persist_assistant_message_with_id(&conn, &msg_id, &input.conversation_id, "gemini", &input.model, &run, duration_ms, Some(&ctx_meta))
}

/// Send a one-shot request to the local `opencode` CLI and persist the result.
///
/// Full ContextPack not supported by opencode — uses lite context prefix instead.
#[tauri::command]
pub fn send_with_opencode(
    input: SendWithClaudeInput,
    state: State<DbState>,
) -> Result<Message, AppError> {
    use super::agents_helpers::send_common::*;

    let id_frag = identity_fragment(&input, "opencode");
    let (enriched_prompt, project_path, ctx_meta) = {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;
        persist_user_message(&conn, &input.conversation_id, &input.prompt, &input.user_message_id)?;
        let pp = load_project_path(&conn, &input.project_key);
        let (prompt, meta) = build_normalized_prompt_with_budget(&conn, &input.conversation_id, &input.prompt, pp.as_deref(), &input.active_skills, &input.cross_session_ids, id_frag.as_deref(), input.context_mode_override.as_deref(), input.context_budget_cap);
        (prompt, pp, meta)
    };

    let t0 = std::time::Instant::now();
    let run_input = claude::RunInput {
        prompt: enriched_prompt,
        model: input.model.clone(),
        system_prompt: None,
        resume_token: None,
        project_path,
    };
    let run_result = std::thread::spawn(move || opencode::run(run_input))
        .join()
        .unwrap_or_else(|_| Err(AppError::Agent("opencode thread panicked".into())));
    let duration_ms = t0.elapsed().as_millis();
    guardrail::log_run("opencode", input.model.as_deref(), duration_ms, input.prompt.len(), run_result.is_ok());

    let run = match run_result {
        Ok(out) if out.content.is_empty() => AgentRunResult {
            content: "(opencode returned no output)".to_string(), status: "done".to_string(),
            cost_usd: 0.0, in_tokens: 0, out_tokens: 0,
        },
        Ok(out) => AgentRunResult {
            content: out.content, status: "done".to_string(),
            cost_usd: 0.0, in_tokens: 0, out_tokens: 0,
        },
        Err(ref e) => AgentRunResult {
            content: guardrail::fallback_error("opencode", e), status: "error".to_string(),
            cost_usd: 0.0, in_tokens: 0, out_tokens: 0,
        },
    };

    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    persist_assistant_message(&conn, &input.conversation_id, "opencode", &input.model, &run, duration_ms, Some(&ctx_meta))
}

/// Streaming version of send_with_claude.
///
/// Flow:
///   1. Persist user message + load resume token + project path (single lock, then released)
///   2. Insert placeholder assistant message with status = 'streaming'
///   3. Spawn claude with --output-format stream-json; emit "claude:chunk" per assistant event
///   4. Re-acquire lock: update message content + status + usage + resume token
#[tauri::command]
pub fn stream_with_claude(
    input: SendWithClaudeInput,
    state: State<DbState>,
    app: AppHandle,
    cancel: State<crate::CancelRegistry>,
) -> Result<Message, AppError> {
    // Step 1: load context + persist user message + load resume token + project path
    let is_branch = input.conversation_id.starts_with("branch:");
    let (resume_token, project_path, msg_id, current_context, parent_context, cross_session_data, plan_section, findings_section, artifacts_section, thread_inheritance) = {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;

        use super::context_queries::{load_recent_messages, conversation_label};

        let current_context = load_recent_messages(&conn, &input.conversation_id, CONTEXT_MESSAGES_LIMIT);

        let parent_context: Vec<(String, String)> = if is_branch {
            let parent_id: Option<String> = conn
                .query_row(
                    "SELECT parent_id FROM conversations WHERE id = ?1",
                    [&input.conversation_id],
                    |row| row.get(0),
                )
                .ok()
                .flatten();
            parent_id
                .map(|pid| load_recent_messages(&conn, &pid, PARENT_CONTEXT_MESSAGES_LIMIT))
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let cross_session_data: Vec<(String, Vec<(String, String)>)> = input
            .cross_session_ids
            .iter()
            .filter(|id| **id != input.conversation_id)
            .filter_map(|conv_id| {
                let label = conversation_label(&conn, conv_id)?;
                let rows = load_recent_messages(&conn, conv_id, CROSS_SESSION_MESSAGES_LIMIT);
                if rows.is_empty() { None } else { Some((label, rows)) }
            })
            .collect();

        let plan_conv_id = resolve_plan_conversation_id(&conn, &input.conversation_id);
        let plan_section = guardrail::truncate_section(
            build_plan_section(&conn, &plan_conv_id),
            guardrail::MAX_PLAN_SECTION,
        );
        let findings_section = guardrail::truncate_section(
            build_findings_section(&conn, &plan_conv_id),
            guardrail::MAX_FINDINGS_SECTION,
        );
        let artifacts_section = guardrail::truncate_section(
            build_artifact_handoff_section(&conn, &plan_conv_id),
            guardrail::MAX_ARTIFACTS_SECTION,
        );

        // Thread inheritance: anchor message + recent parent turns for branches
        let thread_inheritance = if is_branch {
            build_thread_inheritance_section(&conn, &input.conversation_id)
        } else {
            None
        };

        if input.user_message_id.is_none() {
            let id = Uuid::new_v4().to_string();
            let now = now_epoch_ms();
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, timestamp, status)
                 VALUES (?1, ?2, 'user', ?3, ?4, 'done')",
                params![id, input.conversation_id, input.prompt, now],
            )?;
        }

        let token_result: rusqlite::Result<(Option<String>, Option<String>)> = conn.query_row(
            "SELECT resume_token, resume_token_engine FROM conversations WHERE id = ?1",
            [&input.conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );
        let resume_token = match token_result {
            Ok((Some(token), Some(engine))) if engine == "claude-code" => Some(token),
            _ => None,
        };

        let project_path: Option<String> = conn
            .query_row(
                "SELECT path FROM projects WHERE key = ?1",
                [&input.project_key],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        let msg_id = Uuid::new_v4().to_string();
        let now = now_epoch_ms();
        conn.execute(
            "INSERT INTO messages
             (id, conversation_id, role, content, timestamp, status, engine, model)
             VALUES (?1, ?2, 'assistant', '', ?3, 'streaming', 'claude-code', ?4)",
            params![msg_id, input.conversation_id, now, input.model],
        )?;

        (resume_token, project_path, msg_id, current_context, parent_context, cross_session_data, plan_section, findings_section, artifacts_section, thread_inheritance)
    };

    // Step 2: assemble ContextPack — mode-based
    let ctx_mode = if is_branch || input.agent_name.is_some() || input.system_prompt.is_some() {
        ContextMode::Standard
    } else {
        ContextMode::Lite
    };
    eprintln!("[context_pack] mode={:?} for stream_with_claude", ctx_mode);

    let project_context = project_path.as_deref().map(|p| {
        format!("## Project\n\nYou are working on a project located at: `{}`\nAll file paths and code references are relative to this directory.", p)
    });
    let base_system_prompt = assemble_system_prompt(
        input.agent_name.as_deref(),
        project_path.as_deref(),
        input.system_prompt.as_deref(),
    );
    let context_summary = maybe_compress_section_typed(
        build_context_summary(&current_context, &parent_context, is_branch),
        guardrail::MAX_CONTEXT_SECTION,
        Some("context"),
    );

    let (plan_s, findings_s, artifacts_s) = if ctx_mode >= ContextMode::Standard {
        (plan_section, findings_section, artifacts_section)
    } else {
        (None, None, None)
    };

    let (skills_s, rawq_s, cross_s) = if ctx_mode >= ContextMode::Full {
        (
            guardrail::truncate_section(build_skills_section(&input.active_skills), guardrail::MAX_SKILLS_SECTION),
            guardrail::truncate_section(build_rawq_section(project_path.as_deref(), &input.prompt), guardrail::MAX_RAWQ_SECTION),
            maybe_compress_section_typed(build_cross_session_section(&cross_session_data), guardrail::MAX_CROSS_SESSION_SECTION, Some("cross-session")),
        )
    } else {
        (None, None, None)
    };

    // Identity section
    let identity_s = identity_fragment(&input, "claude-code");

    // Track included sections for trace metadata
    let mut ctx_sections: Vec<String> = Vec::new();
    if identity_s.is_some() { ctx_sections.push("identity".into()); }
    if project_context.is_some() { ctx_sections.push("project".into()); }
    if context_summary.is_some() { ctx_sections.push("context".into()); }
    if plan_s.is_some() { ctx_sections.push("plan".into()); }
    if findings_s.is_some() { ctx_sections.push("findings".into()); }
    if artifacts_s.is_some() { ctx_sections.push("artifacts".into()); }
    if skills_s.is_some() { ctx_sections.push("skills".into()); }
    if rawq_s.is_some() { ctx_sections.push("rawq".into()); }
    if cross_s.is_some() { ctx_sections.push("cross-session".into()); }
    if thread_inheritance.is_some() { ctx_sections.push("thread-inheritance".into()); }

    let system_prompt = guardrail::enforce_total_limit(
        combine_prompt_parts([identity_s, project_context, base_system_prompt, plan_s, findings_s, artifacts_s, skills_s, rawq_s, cross_s, thread_inheritance.clone(), context_summary]),
        guardrail::MAX_TOTAL_PROMPT,
    );

    // Step 3: run streaming subprocess — DB lock must NOT be held
    let prompt_len = input.prompt.len() + system_prompt.as_ref().map_or(0, |s| s.len());
    let ctx_meta = ContextPackMeta {
        mode: format!("{:?}", ctx_mode),
        sections: ctx_sections,
        length: prompt_len,
        hash: String::new(),
        truncated: prompt_len >= guardrail::MAX_TOTAL_PROMPT,
    };
    let t0 = std::time::Instant::now();
    let chunk_msg_id = msg_id.clone();
    let progress_msg_id = msg_id.clone();
    let progress_app = app.clone();
    let run_result = claude::stream_run(
        claude::RunInput {
            prompt: input.prompt.clone(),
            model: input.model.clone(),
            system_prompt,
            resume_token,
            project_path: project_path.clone(),
        },
        |progress_text| {
            let _ = progress_app.emit(
                "claude:progress",
                ChunkPayload {
                    message_id: progress_msg_id.clone(),
                    text: progress_text,
                },
            );
        },
        |text| {
            let _ = app.emit(
                "claude:chunk",
                ChunkPayload {
                    message_id: chunk_msg_id.clone(),
                    text,
                },
            );
        },
        // Cancel check — evaluated per stream line
        {
            let conv_id = input.conversation_id.clone();
            let registry = std::sync::Arc::clone(&cancel.0);
            move || {
                if let Ok(mut set) = registry.lock() {
                    set.remove(&conv_id)
                } else {
                    false
                }
            }
        },
    );
    let duration_ms = t0.elapsed().as_millis();
    guardrail::log_run("claude-code-stream", input.model.as_deref(), duration_ms, prompt_len, run_result.is_ok());

    let (content, status, cost_usd, in_tokens, out_tokens, new_token) = match run_result {
        Ok(out) => (
            out.content,
            "done".to_string(),
            out.cost_usd,
            out.input_tokens,
            out.output_tokens,
            out.session_id,
        ),
        Err(ref e) => (guardrail::fallback_error("claude-code", e), "error".to_string(), 0.0, 0, 0, None),
    };

    // Step 4: update placeholder message + conversation usage + resume token
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();

    conn.execute(
        "UPDATE messages SET content = ?1, status = ?2, timestamp = ?3 WHERE id = ?4",
        params![content, status, now, msg_id],
    )?;

    conn.execute(
        "UPDATE conversations SET
             total_input_tokens  = total_input_tokens  + ?1,
             total_output_tokens = total_output_tokens + ?2,
             total_cost_usd      = total_cost_usd      + ?3,
             updated_at          = ?4,
             resume_token        = ?5,
             resume_token_engine = CASE WHEN ?5 IS NOT NULL THEN 'claude-code' ELSE resume_token_engine END
         WHERE id = ?6",
        params![
            in_tokens,
            out_tokens,
            cost_usd,
            now / 1000,
            new_token,
            input.conversation_id,
        ],
    )?;

    insert_trace_log_with_context(&conn, &input.conversation_id, in_tokens, out_tokens, cost_usd, now, &SpanInfo {
        trace_id: &new_trace_id(),
        span_id: new_span_id(),
        parent_span_id: None,
        operation: "agent.stream",
        engine: "claude-code",
        duration_ms: duration_ms as i64,
        status: if status == "done" { "ok" } else { "error" },
    }, &ctx_meta);

    Ok(Message {
        id: msg_id,
        conversation_id: input.conversation_id,
        role: "assistant".into(),
        content,
        timestamp: now,
        status,
        progress_content: None,
        engine: Some("claude-code".into()),
        model: input.model,
        persona: None,
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// Background / event-driven start_* commands (Phase 1)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRunResult { pub message_id: String }

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDonePayload { pub message_id: String, pub conversation_id: String, pub engine: String }

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentErrorPayload { pub message_id: String, pub conversation_id: String, pub engine: String, pub error: String }

/// Background Claude stream — returns immediately, subprocess runs in background.
#[tauri::command]
pub fn start_claude_stream(
    input: SendWithClaudeInput, app: AppHandle,
    state: State<DbState>, cancel: State<crate::CancelRegistry>,
) -> Result<StartRunResult, AppError> {
    let is_branch = input.conversation_id.starts_with("branch:");
    let (resume_token, project_path, msg_id, system_prompt, ctx_meta) = {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;
        use super::context_queries::{load_recent_messages, conversation_label};
        let cur = load_recent_messages(&conn, &input.conversation_id, CONTEXT_MESSAGES_LIMIT);
        let par: Vec<(String,String)> = if is_branch {
            conn.query_row("SELECT parent_id FROM conversations WHERE id=?1",[&input.conversation_id],|r|r.get::<_,Option<String>>(0)).ok().flatten()
                .map(|pid| load_recent_messages(&conn,&pid,PARENT_CONTEXT_MESSAGES_LIMIT)).unwrap_or_default()
        } else { Vec::new() };
        let csd: Vec<(String,Vec<(String,String)>)> = input.cross_session_ids.iter()
            .filter(|id|**id!=input.conversation_id)
            .filter_map(|cid|{let l=conversation_label(&conn,cid)?;let r=load_recent_messages(&conn,cid,CROSS_SESSION_MESSAGES_LIMIT);if r.is_empty(){None}else{Some((l,r))}}).collect();
        let pcid = resolve_plan_conversation_id(&conn, &input.conversation_id);
        let pl = guardrail::truncate_section(build_plan_section(&conn,&pcid),guardrail::MAX_PLAN_SECTION);
        let fi = guardrail::truncate_section(build_findings_section(&conn,&pcid),guardrail::MAX_FINDINGS_SECTION);
        let ar = guardrail::truncate_section(build_artifact_handoff_section(&conn,&pcid),guardrail::MAX_ARTIFACTS_SECTION);
        let th = if is_branch { build_thread_inheritance_section(&conn,&input.conversation_id) } else { None };
        if input.user_message_id.is_none() {
            let id=Uuid::new_v4().to_string();let now=now_epoch_ms();
            conn.execute("INSERT INTO messages(id,conversation_id,role,content,timestamp,status)VALUES(?1,?2,'user',?3,?4,'done')",params![id,input.conversation_id,input.prompt,now])?;
        }
        let rt = conn.query_row("SELECT resume_token,resume_token_engine FROM conversations WHERE id=?1",[&input.conversation_id],|r|Ok((r.get::<_,Option<String>>(0)?,r.get::<_,Option<String>>(1)?)))
            .ok().and_then(|(t,e)|if e.as_deref()==Some("claude-code"){t}else{None});
        let pp: Option<String> = conn.query_row("SELECT path FROM projects WHERE key=?1",[&input.project_key],|r|r.get(0)).ok().flatten();
        let mid=Uuid::new_v4().to_string();let now=now_epoch_ms();
        conn.execute("INSERT INTO messages(id,conversation_id,role,content,timestamp,status,engine,model,persona)VALUES(?1,?2,'assistant','',?3,'streaming','claude-code',?4,?5)",params![mid,input.conversation_id,now,input.model,input.persona_label])?;
        let cm = if is_branch||input.agent_name.is_some()||input.system_prompt.is_some(){ContextMode::Standard}else{ContextMode::Lite};
        let pc = pp.as_deref().map(|p|format!("## Project\n\nYou are working on a project located at: `{}`\nAll file paths and code references are relative to this directory.",p));
        let bp = assemble_system_prompt(input.agent_name.as_deref(),pp.as_deref(),input.system_prompt.as_deref());
        let cs = maybe_compress_section_typed(build_context_summary(&cur,&par,is_branch),guardrail::MAX_CONTEXT_SECTION,Some("context"));
        let(ps,fs,a2)=if cm>=ContextMode::Standard{(pl,fi,ar)}else{(None,None,None)};
        let(sk,rq,cr)=if cm>=ContextMode::Full{
            (guardrail::truncate_section(build_skills_section(&input.active_skills),guardrail::MAX_SKILLS_SECTION),
             guardrail::truncate_section(build_rawq_section(pp.as_deref(),&input.prompt),guardrail::MAX_RAWQ_SECTION),
             maybe_compress_section_typed(build_cross_session_section(&csd),guardrail::MAX_CROSS_SESSION_SECTION,Some("cross-session")))
        }else{(None,None,None)};
        let id_sec = identity_fragment(&input, "claude-code");
        let sf = [("identity",id_sec.is_some()),("project",pc.is_some()),("system_prompt",bp.is_some()),("plan",ps.is_some()),("findings",fs.is_some()),
              ("artifacts",a2.is_some()),("skills",sk.is_some()),("rawq",rq.is_some()),("cross_session",cr.is_some()),
              ("thread_inheritance",th.is_some()),("context_summary",cs.is_some())];
        let pre_limit = combine_prompt_parts([id_sec,pc,bp,ps,fs,a2,sk,rq,cr,th,cs]);
        let pre_len = pre_limit.as_ref().map_or(0,|s|s.len());
        let sp = guardrail::enforce_total_limit(pre_limit, guardrail::MAX_TOTAL_PROMPT);
        let ctx_meta = ContextPackMeta::from_parts(&format!("{:?}",cm), &sf, &sp, pre_len > sp.as_ref().map_or(0,|s|s.len()));
        (rt,pp,mid,sp,ctx_meta)
    };
    // Create durable job record
    let job_id = format!("job-{}", Uuid::new_v4());
    { let conn = state.write.lock().map_err(|_| AppError::Lock)?;
      let _ = jobs::create_job(&conn, &job_id, &input.conversation_id, Some(&msg_id), "claude-code", "agent"); }

    let carc=std::sync::Arc::clone(&cancel.0);
    let write_arc=std::sync::Arc::clone(&state.write);
    let ab=app;let ret=msg_id.clone();let jid=job_id;
    let cid=input.conversation_id;let pr=input.prompt;let mo=input.model;
    let plen=pr.len()+system_prompt.as_ref().map_or(0,|s|s.len());
    std::thread::spawn(move||{
        let pa=ab.clone();let pi=msg_id.clone();let c2=ab.clone();let ci=msg_id.clone();
        let t0=std::time::Instant::now();
        let rr=claude::stream_run(
            claude::RunInput{prompt:pr,model:mo.clone(),system_prompt,resume_token,project_path},
            move|t|{let _=pa.emit("claude:progress",ChunkPayload{message_id:pi.clone(),text:t});},
            move|t|{let _=c2.emit("claude:chunk",ChunkPayload{message_id:ci.clone(),text:t});},
            {let c=cid.clone();let r=carc;move||{if let Ok(mut s)=r.lock(){s.remove(&c)}else{false}}},
        );
        let dur=t0.elapsed().as_millis();
        guardrail::log_run("claude-bg",mo.as_deref(),dur,plen,rr.is_ok());
        if let Ok(conn)=write_arc.lock(){let now=now_epoch_ms();match rr{
            Ok(out)=>{
                let _=conn.execute("UPDATE messages SET content=?1,status='done',timestamp=?2 WHERE id=?3",params![out.content,now,msg_id]);
                let _=conn.execute("UPDATE conversations SET total_input_tokens=total_input_tokens+?1,total_output_tokens=total_output_tokens+?2,total_cost_usd=total_cost_usd+?3,updated_at=?4,resume_token=?5,resume_token_engine=CASE WHEN ?5 IS NOT NULL THEN 'claude-code' ELSE resume_token_engine END WHERE id=?6",
                    params![out.input_tokens,out.output_tokens,out.cost_usd,now/1000,out.session_id,cid]);
                insert_trace_log_with_context(&conn,&cid,out.input_tokens,out.output_tokens,out.cost_usd,now,&SpanInfo{trace_id:&new_trace_id(),span_id:new_span_id(),parent_span_id:None,operation:"agent.stream",engine:"claude-code",duration_ms:dur as i64,status:"ok"},&ctx_meta);
                let _=jobs::complete_job(&conn,&jid,"done",None);
                let _=ab.emit("agent:completed",AgentDonePayload{message_id:msg_id,conversation_id:cid,engine:"claude-code".into()});
            }
            Err(ref e)=>{
                let em=guardrail::fallback_error("claude-code",e);
                let _=conn.execute("UPDATE messages SET content=?1,status='error',timestamp=?2 WHERE id=?3",params![em,now,msg_id]);
                let _=jobs::complete_job(&conn,&jid,"error",Some(&em));
                let _=ab.emit("agent:error",AgentErrorPayload{message_id:msg_id,conversation_id:cid,engine:"claude-code".into(),error:em});
            }
        }}
    });
    Ok(StartRunResult{message_id:ret})
}

/// Background Gemini stream — returns immediately.
#[tauri::command]
pub fn start_gemini_stream(input:SendWithClaudeInput,app:AppHandle,state:State<DbState>,cancel:State<crate::CancelRegistry>)->Result<StartRunResult,AppError>{
    use super::agents_helpers::send_common::*;
    let id_frag=identity_fragment(&input,"gemini");
    let(ep,pp,mid,ep_meta)={let conn=state.write.lock().map_err(|_|AppError::Lock)?;
        persist_user_message(&conn,&input.conversation_id,&input.prompt,&input.user_message_id)?;
        let pp=load_project_path(&conn,&input.project_key);let (ep,ep_meta)=build_normalized_prompt_with_budget(&conn,&input.conversation_id,&input.prompt,pp.as_deref(),&input.active_skills,&input.cross_session_ids,id_frag.as_deref(),input.context_mode_override.as_deref(),input.context_budget_cap);
        let mid=format!("msg-{}",Uuid::new_v4());let now=now_epoch_ms();
        conn.execute("INSERT INTO messages(id,conversation_id,role,content,timestamp,status,engine,model,persona)VALUES(?1,?2,'assistant','',?3,'streaming','gemini',?4,?5)",params![mid,input.conversation_id,now,input.model,input.persona_label])?;
        (ep,pp,mid,ep_meta)};
    let jid=format!("job-{}",Uuid::new_v4());
    {let conn=state.write.lock().map_err(|_|AppError::Lock)?;let _=jobs::create_job(&conn,&jid,&input.conversation_id,Some(&mid),"gemini","agent");}
    let ca=std::sync::Arc::clone(&cancel.0);let write_arc=std::sync::Arc::clone(&state.write);
    let ab=app;let r=mid.clone();let cid=input.conversation_id;let m=input.model;
    std::thread::spawn(move||{
        let pa=ab.clone();let pi=mid.clone();let c2=ab.clone();let ci=mid.clone();
        let t0=std::time::Instant::now();
        let rr=gemini::stream_run(claude::RunInput{prompt:ep,model:m.clone(),system_prompt:None,resume_token:None,project_path:pp},
            move|t|{let _=pa.emit("gemini:progress",ChunkPayload{message_id:pi.clone(),text:t});},
            move|t|{let _=c2.emit("gemini:chunk",ChunkPayload{message_id:ci.clone(),text:t});},
            {let c=cid.clone();let r=ca;move||{if let Ok(mut s)=r.lock(){s.remove(&c)}else{false}}});
        let _dur=t0.elapsed().as_millis();
        if let Ok(conn)=write_arc.lock(){let now=now_epoch_ms();match rr{
            Ok(out)=>{let c=if out.content.is_empty(){"(gemini returned no output)".into()}else{out.content};
                let _=conn.execute("UPDATE messages SET content=?1,status='done',timestamp=?2 WHERE id=?3",params![c,now,mid]);
                let _=conn.execute("UPDATE conversations SET total_input_tokens=total_input_tokens+?1,total_output_tokens=total_output_tokens+?2,updated_at=?3 WHERE id=?4",params![out.input_tokens,out.output_tokens,now/1000,cid]);
                insert_trace_log_with_context(&conn,&cid,out.input_tokens,out.output_tokens,out.cost_usd,now,&SpanInfo{trace_id:&new_trace_id(),span_id:new_span_id(),parent_span_id:None,operation:"agent.stream",engine:"gemini",duration_ms:_dur as i64,status:"ok"},&ep_meta);
                let _=jobs::complete_job(&conn,&jid,"done",None);
                let _=ab.emit("agent:completed",AgentDonePayload{message_id:mid,conversation_id:cid,engine:"gemini".into()});}
            Err(ref e)=>{let em=guardrail::fallback_error("gemini",e);
                let _=conn.execute("UPDATE messages SET content=?1,status='error',timestamp=?2 WHERE id=?3",params![em,now,mid]);
                let _=jobs::complete_job(&conn,&jid,"error",Some(&em));
                let _=ab.emit("agent:error",AgentErrorPayload{message_id:mid,conversation_id:cid,engine:"gemini".into(),error:em});}
        }}
    });
    Ok(StartRunResult{message_id:r})
}

/// Background Codex run — returns immediately.
#[tauri::command]
pub fn start_codex_run(input:SendWithClaudeInput,app:AppHandle,state:State<DbState>)->Result<StartRunResult,AppError>{
    use super::agents_helpers::send_common::*;
    let id_frag=identity_fragment(&input,"codex");
    let(ep,pp,mid,ep_meta)={let conn=state.write.lock().map_err(|_|AppError::Lock)?;
        persist_user_message(&conn,&input.conversation_id,&input.prompt,&input.user_message_id)?;
        let pp=load_project_path(&conn,&input.project_key);let (ep,ep_meta)=build_normalized_prompt_with_budget(&conn,&input.conversation_id,&input.prompt,pp.as_deref(),&input.active_skills,&input.cross_session_ids,id_frag.as_deref(),input.context_mode_override.as_deref(),input.context_budget_cap);
        let mid=format!("msg-{}",Uuid::new_v4());let now=now_epoch_ms();
        conn.execute("INSERT INTO messages(id,conversation_id,role,content,timestamp,status,engine,model,persona)VALUES(?1,?2,'assistant','',?3,'streaming','codex',?4,?5)",params![mid,input.conversation_id,now,input.model,input.persona_label])?;
        (ep,pp,mid,ep_meta)};
    let jid=format!("job-{}",Uuid::new_v4());
    {let conn=state.write.lock().map_err(|_|AppError::Lock)?;let _=jobs::create_job(&conn,&jid,&input.conversation_id,Some(&mid),"codex","agent");}
    let write_arc=std::sync::Arc::clone(&state.write);
    let ab=app;let r=mid.clone();let cid=input.conversation_id;
    std::thread::spawn(move||{
        let chunk_mid=mid.clone();let chunk_app=ab.clone();
        let progress_mid=mid.clone();let progress_app=ab.clone();
        let t0=std::time::Instant::now();
        let rr=codex::stream_run(
            claude::RunInput{prompt:ep,model:input.model.clone(),system_prompt:None,resume_token:None,project_path:pp},
            |event_type|{let _=progress_app.emit("codex:progress",ChunkPayload{message_id:progress_mid.clone(),text:format!("codex: {}",event_type)});},
            |accumulated|{let _=chunk_app.emit("codex:chunk",ChunkPayload{message_id:chunk_mid.clone(),text:accumulated.to_string()});},
        );
        let dur=t0.elapsed().as_millis();
        if let Ok(conn)=write_arc.lock(){let now=now_epoch_ms();match rr{
            Ok(out)=>{let c=if out.content.is_empty(){"(codex returned no output)".into()}else{out.content};
                let _=conn.execute("UPDATE messages SET content=?1,status='done',timestamp=?2 WHERE id=?3",params![c,now,mid]);
                let _=conn.execute("UPDATE conversations SET total_input_tokens=total_input_tokens+?1,total_output_tokens=total_output_tokens+?2,total_cost_usd=total_cost_usd+?3,updated_at=?4 WHERE id=?5",params![out.input_tokens,out.output_tokens,out.cost_usd,now/1000,cid]);
                insert_trace_log_with_context(&conn,&cid,out.input_tokens,out.output_tokens,out.cost_usd,now,&SpanInfo{trace_id:&new_trace_id(),span_id:new_span_id(),parent_span_id:None,operation:"agent.stream",engine:"codex",duration_ms:dur as i64,status:"ok"},&ep_meta);
                let _=jobs::complete_job(&conn,&jid,"done",None);
                let _=ab.emit("agent:completed",AgentDonePayload{message_id:mid,conversation_id:cid,engine:"codex".into()});}
            Err(ref e)=>{let em=guardrail::fallback_error("codex",e);
                let _=conn.execute("UPDATE messages SET content=?1,status='error',timestamp=?2 WHERE id=?3",params![em,now,mid]);
                let _=jobs::complete_job(&conn,&jid,"error",Some(&em));
                let _=ab.emit("agent:error",AgentErrorPayload{message_id:mid,conversation_id:cid,engine:"codex".into(),error:em});}
        }}
    });
    Ok(StartRunResult{message_id:r})
}

/// Eval-only: run a prompt through an engine synchronously, return content only.
/// Does NOT persist to conversations/messages — result goes to eval_results instead.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalAgentResult {
    pub content: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
    pub duration_ms: i64,
}

#[tauri::command]
pub fn run_eval_agent(
    engine: String,
    prompt: String,
    model: Option<String>,
    project_path: Option<String>,
) -> Result<EvalAgentResult, AppError> {
    let t0 = std::time::Instant::now();

    let run_input = claude::RunInput {
        prompt: prompt.clone(),
        model: model.clone(),
        system_prompt: None,
        resume_token: None,
        project_path: project_path.clone(),
    };

    let result = match engine.as_str() {
        "codex" => codex::run(run_input),
        "gemini" => gemini::run(run_input),
        "opencode" => opencode::run(run_input),
        _ => claude::run(run_input), // default to claude
    };

    let duration_ms = t0.elapsed().as_millis() as i64;

    match result {
        Ok(out) => Ok(EvalAgentResult {
            content: if out.content.is_empty() { "(no output)".into() } else { out.content },
            input_tokens: out.input_tokens,
            output_tokens: out.output_tokens,
            cost_usd: out.cost_usd,
            duration_ms,
        }),
        Err(e) => Err(AppError::Agent(format!("{} eval failed: {}", engine, e))),
    }
}

/// Background OpenCode run — returns immediately.
#[tauri::command]
pub fn start_opencode_run(input:SendWithClaudeInput,app:AppHandle,state:State<DbState>)->Result<StartRunResult,AppError>{
    use super::agents_helpers::send_common::*;
    let id_frag=identity_fragment(&input,"opencode");
    let(ep,pp,mid,ep_meta)={let conn=state.write.lock().map_err(|_|AppError::Lock)?;
        persist_user_message(&conn,&input.conversation_id,&input.prompt,&input.user_message_id)?;
        let pp=load_project_path(&conn,&input.project_key);let (ep,ep_meta)=build_normalized_prompt_with_budget(&conn,&input.conversation_id,&input.prompt,pp.as_deref(),&input.active_skills,&input.cross_session_ids,id_frag.as_deref(),input.context_mode_override.as_deref(),input.context_budget_cap);
        let mid=format!("msg-{}",Uuid::new_v4());let now=now_epoch_ms();
        conn.execute("INSERT INTO messages(id,conversation_id,role,content,timestamp,status,engine,model,persona)VALUES(?1,?2,'assistant','',?3,'streaming','opencode',?4,?5)",params![mid,input.conversation_id,now,input.model,input.persona_label])?;
        (ep,pp,mid,ep_meta)};
    let jid=format!("job-{}",Uuid::new_v4());
    {let conn=state.write.lock().map_err(|_|AppError::Lock)?;let _=jobs::create_job(&conn,&jid,&input.conversation_id,Some(&mid),"opencode","agent");}
    let write_arc=std::sync::Arc::clone(&state.write);
    let ab=app;let r=mid.clone();let cid=input.conversation_id;
    std::thread::spawn(move||{
        let _=ab.emit("opencode:progress",ChunkPayload{message_id:mid.clone(),text:"OpenCode starting...".into()});
        let t0=std::time::Instant::now();
        let rr=opencode::run(claude::RunInput{prompt:ep,model:input.model.clone(),system_prompt:None,resume_token:None,project_path:pp});
        let dur=t0.elapsed().as_millis();
        if let Ok(conn)=write_arc.lock(){let now=now_epoch_ms();match rr{
            Ok(out)=>{let c=if out.content.is_empty(){"(opencode returned no output)".into()}else{out.content};
                let _=conn.execute("UPDATE messages SET content=?1,status='done',timestamp=?2 WHERE id=?3",params![c,now,mid]);
                let _=conn.execute("UPDATE conversations SET total_input_tokens=total_input_tokens+?1,total_output_tokens=total_output_tokens+?2,updated_at=?3 WHERE id=?4",params![out.input_tokens,out.output_tokens,now/1000,cid]);
                insert_trace_log_with_context(&conn,&cid,out.input_tokens,out.output_tokens,out.cost_usd,now,&SpanInfo{trace_id:&new_trace_id(),span_id:new_span_id(),parent_span_id:None,operation:"agent.run",engine:"opencode",duration_ms:dur as i64,status:"ok"},&ep_meta);
                let _=jobs::complete_job(&conn,&jid,"done",None);
                let _=ab.emit("agent:completed",AgentDonePayload{message_id:mid,conversation_id:cid,engine:"opencode".into()});}
            Err(ref e)=>{let em=guardrail::fallback_error("opencode",e);
                let _=conn.execute("UPDATE messages SET content=?1,status='error',timestamp=?2 WHERE id=?3",params![em,now,mid]);
                let _=jobs::complete_job(&conn,&jid,"error",Some(&em));
                let _=ab.emit("agent:error",AgentErrorPayload{message_id:mid,conversation_id:cid,engine:"opencode".into(),error:em});}
        }}
    });
    Ok(StartRunResult{message_id:r})
}
