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
use super::agents_helpers::compression::maybe_compress_section;
use super::agents_helpers::trace_log::{insert_trace_log, new_span_id, new_trace_id, SpanInfo};

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
}

const CONTEXT_MESSAGES_LIMIT: i64 = 6;
const PARENT_CONTEXT_MESSAGES_LIMIT: i64 = 4;
const CROSS_SESSION_MESSAGES_LIMIT: i64 = 3;

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
        let conn = state.0.lock().map_err(|_| AppError::Lock)?;

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
    let context_summary = maybe_compress_section(
        build_context_summary(&current_context, &parent_context, is_branch),
        guardrail::MAX_CONTEXT_SECTION,
    );

    // Standard+ 섹션 (plan, findings, artifacts)
    let (plan_s, findings_s, artifacts_s) = if ctx_mode >= ContextMode::Standard {
        (plan_section, findings_section, artifacts_section)
    } else {
        (None, None, None)
    };

    // Full 섹션 (skills, rawq, cross-session)
    let (skills_s, rawq_s, cross_s) = if ctx_mode >= ContextMode::Full {
        (
            guardrail::truncate_section(build_skills_section(&input.active_skills), guardrail::MAX_SKILLS_SECTION),
            guardrail::truncate_section(build_rawq_section(project_path.as_deref(), &input.prompt), guardrail::MAX_RAWQ_SECTION),
            maybe_compress_section(build_cross_session_section(&cross_session_data), guardrail::MAX_CROSS_SESSION_SECTION),
        )
    } else {
        (None, None, None)
    };

    let system_prompt = guardrail::enforce_total_limit(
        combine_prompt_parts([project_context, base_system_prompt, plan_s, findings_s, artifacts_s, skills_s, rawq_s, cross_s, thread_inheritance.clone(), context_summary]),
        guardrail::MAX_TOTAL_PROMPT,
    );

    // Step 2: run claude subprocess in background thread — prevents UI freeze
    let prompt_len = input.prompt.len() + system_prompt.as_ref().map_or(0, |s| s.len());
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
    let conn = state.0.lock().map_err(|_| AppError::Lock)?;
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

    insert_trace_log(&conn, &input.conversation_id, in_tokens, out_tokens, cost_usd, now, &SpanInfo {
        trace_id: &new_trace_id(),
        span_id: new_span_id(),
        parent_span_id: None,
        operation: "agent.send",
        engine: "claude-code",
        duration_ms: duration_ms as i64,
        status: if status == "done" { "ok" } else { "error" },
    });

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

    // Step 1: persist user message + build lite context (single lock block)
    let (enriched_prompt, project_path) = {
        let conn = state.0.lock().map_err(|_| AppError::Lock)?;
        persist_user_message(&conn, &input.conversation_id, &input.prompt, &input.user_message_id)?;
        let pp = load_project_path(&conn, &input.project_key);
        let prompt = build_lite_enriched_prompt(&conn, &input.conversation_id, &input.prompt, pp.as_deref());
        (prompt, pp)
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
    let conn = state.0.lock().map_err(|_| AppError::Lock)?;
    persist_assistant_message(&conn, &input.conversation_id, "codex", &input.model, &run, duration_ms)
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

    // Step 1: persist user message + build lite context (single lock block)
    let (enriched_prompt, project_path) = {
        let conn = state.0.lock().map_err(|_| AppError::Lock)?;
        persist_user_message(&conn, &input.conversation_id, &input.prompt, &input.user_message_id)?;
        let pp = load_project_path(&conn, &input.project_key);
        let prompt = build_lite_enriched_prompt(&conn, &input.conversation_id, &input.prompt, pp.as_deref());
        (prompt, pp)
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
    let conn = state.0.lock().map_err(|_| AppError::Lock)?;
    persist_assistant_message(&conn, &input.conversation_id, "gemini", &input.model, &run, duration_ms)
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

    let (enriched_prompt, project_path) = {
        let conn = state.0.lock().map_err(|_| AppError::Lock)?;
        persist_user_message(&conn, &input.conversation_id, &input.prompt, &input.user_message_id)?;
        let pp = load_project_path(&conn, &input.project_key);
        let prompt = build_lite_enriched_prompt(&conn, &input.conversation_id, &input.prompt, pp.as_deref());
        (prompt, pp)
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

    let conn = state.0.lock().map_err(|_| AppError::Lock)?;
    persist_assistant_message(&conn, &input.conversation_id, "opencode", &input.model, &run, duration_ms)
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
        let conn = state.0.lock().map_err(|_| AppError::Lock)?;

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
    let context_summary = maybe_compress_section(
        build_context_summary(&current_context, &parent_context, is_branch),
        guardrail::MAX_CONTEXT_SECTION,
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
            maybe_compress_section(build_cross_session_section(&cross_session_data), guardrail::MAX_CROSS_SESSION_SECTION),
        )
    } else {
        (None, None, None)
    };

    let system_prompt = guardrail::enforce_total_limit(
        combine_prompt_parts([project_context, base_system_prompt, plan_s, findings_s, artifacts_s, skills_s, rawq_s, cross_s, thread_inheritance.clone(), context_summary]),
        guardrail::MAX_TOTAL_PROMPT,
    );

    // Step 3: run streaming subprocess — DB lock must NOT be held
    let prompt_len = input.prompt.len() + system_prompt.as_ref().map_or(0, |s| s.len());
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
    let conn = state.0.lock().map_err(|_| AppError::Lock)?;
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

    insert_trace_log(&conn, &input.conversation_id, in_tokens, out_tokens, cost_usd, now, &SpanInfo {
        trace_id: &new_trace_id(),
        span_id: new_span_id(),
        parent_span_id: None,
        operation: "agent.stream",
        engine: "claude-code",
        duration_ms: duration_ms as i64,
        status: if status == "done" { "ok" } else { "error" },
    });

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
