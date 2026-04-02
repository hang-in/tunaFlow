use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::agents::{anthropic_sdk, claude, codex, gemini, gemini_sdk, openai_compat, openai_sdk, opencode};
use crate::db::DbState;
use crate::errors::AppError;
use crate::guardrail;

use super::agents_helpers::context_pack::assemble_system_prompt;
use super::agents_helpers::send_common::{prepare_engine_run, finalize_engine_run, PreparedRun};

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

/// Wrap persona_fragment with identity framing block for a given engine.
fn identity_fragment(input: &SendWithClaudeInput, engine: &str) -> Option<String> {
    super::agents_helpers::send_common::build_identity_persona_fragment(
        input.persona_label.as_deref(),
        engine,
        input.persona_fragment.as_deref(),
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// Background start_* commands — async, DB work runs off main thread
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

/// Background Claude stream — DB prep runs off main thread, subprocess in background.
#[tauri::command]
pub async fn start_claude_stream(
    input: SendWithClaudeInput, app: AppHandle,
    state: State<'_, DbState>, cancel: State<'_, crate::CancelRegistry>,
) -> Result<StartRunResult, AppError> {
    let db = state.inner().clone();
    let id_frag = identity_fragment(&input, "claude-code");
    let cancel_arc = std::sync::Arc::clone(&cancel.0);
    let write_arc = db_write_arc(&state);

    // Extract values needed after spawn_blocking (input will be moved)
    let cid = input.conversation_id.clone();
    let pr = input.prompt.clone();
    let mo = input.model.clone();

    // DB-heavy work off main thread
    let (prep, resume_token, system_prompt) = tokio::task::spawn_blocking(move || -> Result<_, AppError> {
        let prep = prepare_engine_run("claude-code", &input, id_frag.as_deref(), &db)?;

        let (resume_token, system_prompt) = {
            let conn = db.write.lock().map_err(|_| AppError::Lock)?;
            let rt = conn.query_row(
                "SELECT resume_token, resume_token_engine FROM conversations WHERE id=?1",
                [&input.conversation_id],
                |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)),
            ).ok().and_then(|(t, e)| if e.as_deref() == Some("claude-code") { t } else { None });
            let agent_sp = assemble_system_prompt(
                input.agent_name.as_deref(), prep.project_path.as_deref(), input.system_prompt.as_deref(),
            );
            let sp = match (prep.system_context.clone(), agent_sp) {
                (Some(c), Some(a)) => Some(format!("{}\n\n{}", c, a)),
                (c @ Some(_), None) => c,
                (None, a @ Some(_)) => a,
                (None, None) => None,
            };
            (rt, sp)
        };

        Ok((prep, resume_token, system_prompt))
    }).await.map_err(|_| AppError::Lock)??;

    let ret = prep.msg_id.clone();
    let PreparedRun { msg_id, job_id, project_path, ctx_meta, .. } = prep;
    let plen = pr.len() + system_prompt.as_ref().map_or(0, |s| s.len());

    // Claude: CLI preferred (file editing, MCP, terminal).
    // SDK only when ANTHROPIC_API_KEY is set AND conversation is a branch (Developer/Reviewer).
    let use_sdk = anthropic_sdk::is_available() && cid.starts_with("branch:");

    if use_sdk {
        let sp = system_prompt;
        tokio::spawn(async move {
            let pa = app.clone(); let pi = msg_id.clone();
            let c2 = app.clone(); let ci = msg_id.clone();
            let t0 = std::time::Instant::now();
            let rr = anthropic_sdk::stream_run(
                claude::RunInput { prompt: pr, model: mo.clone(), system_prompt: sp, resume_token: None, project_path },
                move |t| { let _ = pa.emit("claude:progress", ChunkPayload { message_id: pi.clone(), text: t }); },
                move |t| { let _ = c2.emit("claude:chunk", ChunkPayload { message_id: ci.clone(), text: t }); },
            ).await;
            let dur = t0.elapsed().as_millis();
            guardrail::log_run("claude-sdk", mo.as_deref(), dur, plen, rr.is_ok());
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "claude-code", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app);
            }
        });
    } else {
        // CLI path — full Claude Code features
        std::thread::spawn(move || {
            let pa = app.clone(); let pi = msg_id.clone();
            let c2 = app.clone(); let ci = msg_id.clone();
            let t0 = std::time::Instant::now();
            let rr = claude::stream_run(
                claude::RunInput { prompt: pr, model: mo.clone(), system_prompt, resume_token, project_path },
                move |t| { let _ = pa.emit("claude:progress", ChunkPayload { message_id: pi.clone(), text: t }); },
                move |t| { let _ = c2.emit("claude:chunk", ChunkPayload { message_id: ci.clone(), text: t }); },
                { let c = cid.clone(); let r = cancel_arc; move || { if let Ok(mut s) = r.lock() { s.remove(&c) } else { false } } },
            );
            let dur = t0.elapsed().as_millis();
            guardrail::log_run("claude-bg", mo.as_deref(), dur, plen, rr.is_ok());
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "claude-code", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app);
            }
        });
    }
    Ok(StartRunResult { message_id: ret })
}

/// Background Gemini stream — async, DB prep off main thread.
#[tauri::command]
pub async fn start_gemini_stream(
    input: SendWithClaudeInput, app: AppHandle,
    state: State<'_, DbState>, cancel: State<'_, crate::CancelRegistry>,
) -> Result<StartRunResult, AppError> {
    let db = state.inner().clone();
    let id_frag = identity_fragment(&input, "gemini");
    let cancel_arc = std::sync::Arc::clone(&cancel.0);
    let write_arc = db_write_arc(&state);
    let cid = input.conversation_id.clone();
    let mo = input.model.clone();

    let prep = tokio::task::spawn_blocking(move || {
        prepare_engine_run("gemini", &input, id_frag.as_deref(), &db)
    }).await.map_err(|_| AppError::Lock)??;

    let ret = prep.msg_id.clone();
    let PreparedRun { msg_id, job_id, enriched_prompt, project_path, ctx_meta, .. } = prep;

    if gemini_sdk::is_available() {
        // SDK path — async, native streaming, accurate token tracking
        let system_prompt = prep.system_context;
        tokio::spawn(async move {
            let pa = app.clone(); let pi = msg_id.clone();
            let c2 = app.clone(); let ci = msg_id.clone();
            let t0 = std::time::Instant::now();
            let rr = gemini_sdk::stream_run(
                claude::RunInput { prompt: enriched_prompt, model: mo.clone(), system_prompt, resume_token: None, project_path },
                move |t| { let _ = pa.emit("gemini:progress", ChunkPayload { message_id: pi.clone(), text: t }); },
                move |t| { let _ = c2.emit("gemini:chunk", ChunkPayload { message_id: ci.clone(), text: t }); },
            ).await;
            let dur = t0.elapsed().as_millis();
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "gemini", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app);
            }
        });
    } else {
        // CLI fallback
        std::thread::spawn(move || {
            let pa = app.clone(); let pi = msg_id.clone();
            let c2 = app.clone(); let ci = msg_id.clone();
            let t0 = std::time::Instant::now();
            let rr = gemini::stream_run(
                claude::RunInput { prompt: enriched_prompt, model: mo.clone(), system_prompt: None, resume_token: None, project_path },
                move |t| { let _ = pa.emit("gemini:progress", ChunkPayload { message_id: pi.clone(), text: t }); },
                move |t| { let _ = c2.emit("gemini:chunk", ChunkPayload { message_id: ci.clone(), text: t }); },
                { let c = cid.clone(); let r = cancel_arc; move || { if let Ok(mut s) = r.lock() { s.remove(&c) } else { false } } },
            );
            let dur = t0.elapsed().as_millis();
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "gemini", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app);
            }
        });
    }
    Ok(StartRunResult { message_id: ret })
}

/// Background Codex run — async, DB prep off main thread.
#[tauri::command]
pub async fn start_codex_run(
    input: SendWithClaudeInput, app: AppHandle, state: State<'_, DbState>,
) -> Result<StartRunResult, AppError> {
    let db = state.inner().clone();
    let id_frag = identity_fragment(&input, "codex");
    let write_arc = db_write_arc(&state);
    let cid = input.conversation_id.clone();
    let mo = input.model.clone();

    let prep = tokio::task::spawn_blocking(move || {
        prepare_engine_run("codex", &input, id_frag.as_deref(), &db)
    }).await.map_err(|_| AppError::Lock)??;

    let ret = prep.msg_id.clone();
    let PreparedRun { msg_id, job_id, enriched_prompt, project_path, ctx_meta, .. } = prep;

    if openai_sdk::is_available() {
        // SDK path — OpenAI Chat Completions API
        let system_prompt = prep.system_context;
        tokio::spawn(async move {
            let pa = app.clone(); let pi = msg_id.clone();
            let c2 = app.clone(); let ci = msg_id.clone();
            let t0 = std::time::Instant::now();
            let rr = openai_sdk::stream_run(
                claude::RunInput { prompt: enriched_prompt, model: mo.clone(), system_prompt, resume_token: None, project_path },
                move |t| { let _ = pa.emit("codex:progress", ChunkPayload { message_id: pi.clone(), text: t }); },
                move |t| { let _ = c2.emit("codex:chunk", ChunkPayload { message_id: ci.clone(), text: t }); },
            ).await;
            let dur = t0.elapsed().as_millis();
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "codex", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app);
            }
        });
    } else {
        // Codex CLI fallback
        std::thread::spawn(move || {
            let chunk_mid = msg_id.clone(); let chunk_app = app.clone();
            let progress_mid = msg_id.clone(); let progress_app = app.clone();
            let t0 = std::time::Instant::now();
            let rr = codex::stream_run(
                claude::RunInput { prompt: enriched_prompt, model: mo.clone(), system_prompt: None, resume_token: None, project_path },
                |event_type| { let _ = progress_app.emit("codex:progress", ChunkPayload { message_id: progress_mid.clone(), text: format!("codex: {}", event_type) }); },
                |accumulated| { let _ = chunk_app.emit("codex:chunk", ChunkPayload { message_id: chunk_mid.clone(), text: accumulated.to_string() }); },
            );
            let dur = t0.elapsed().as_millis();
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "codex", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app);
            }
        });
    }
    Ok(StartRunResult { message_id: ret })
}

/// Background OpenCode run — async, DB prep off main thread.
#[tauri::command]
pub async fn start_opencode_run(
    input: SendWithClaudeInput, app: AppHandle, state: State<'_, DbState>,
) -> Result<StartRunResult, AppError> {
    let db = state.inner().clone();
    let id_frag = identity_fragment(&input, "opencode");
    let write_arc = db_write_arc(&state);
    let cid = input.conversation_id.clone();
    let mo = input.model.clone();

    let prep = tokio::task::spawn_blocking(move || {
        prepare_engine_run("opencode", &input, id_frag.as_deref(), &db)
    }).await.map_err(|_| AppError::Lock)??;

    let ret = prep.msg_id.clone();
    let PreparedRun { msg_id, job_id, enriched_prompt, project_path, ctx_meta, .. } = prep;

    std::thread::spawn(move || {
        let _ = app.emit("opencode:progress", ChunkPayload { message_id: msg_id.clone(), text: "OpenCode starting...".into() });
        let t0 = std::time::Instant::now();
        let rr = opencode::run(
            claude::RunInput { prompt: enriched_prompt, model: mo.clone(), system_prompt: None, resume_token: None, project_path },
        );
        let dur = t0.elapsed().as_millis();
        if let Ok(conn) = write_arc.lock() {
            finalize_engine_run(&conn, "opencode", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app);
        }
    });
    Ok(StartRunResult { message_id: ret })
}

/// Background OpenAI-compatible stream — Ollama, LM Studio, vLLM, etc.
#[tauri::command]
pub async fn start_openai_compat_stream(
    input: SendWithClaudeInput, app: AppHandle, state: State<'_, DbState>,
) -> Result<StartRunResult, AppError> {
    let db = state.inner().clone();
    let id_frag = identity_fragment(&input, "ollama");
    let write_arc = db_write_arc(&state);
    let cid = input.conversation_id.clone();
    let mo = input.model.clone();

    let prep = tokio::task::spawn_blocking(move || {
        prepare_engine_run("ollama", &input, id_frag.as_deref(), &db)
    }).await.map_err(|_| AppError::Lock)??;

    let ret = prep.msg_id.clone();
    let PreparedRun { msg_id, job_id, enriched_prompt, project_path, ctx_meta, .. } = prep;
    let system_prompt = prep.system_context;

    tokio::spawn(async move {
        let pa = app.clone(); let pi = msg_id.clone();
        let c2 = app.clone(); let ci = msg_id.clone();
        let t0 = std::time::Instant::now();
        let rr = openai_compat::stream_run(
            claude::RunInput { prompt: enriched_prompt, model: mo.clone(), system_prompt, resume_token: None, project_path },
            move |t| { let _ = pa.emit("ollama:progress", ChunkPayload { message_id: pi.clone(), text: t }); },
            move |t| { let _ = c2.emit("ollama:chunk", ChunkPayload { message_id: ci.clone(), text: t }); },
        ).await;
        let dur = t0.elapsed().as_millis();
        guardrail::log_run("ollama", mo.as_deref(), dur, 0, rr.is_ok());
        if let Ok(conn) = write_arc.lock() {
            finalize_engine_run(&conn, "ollama", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app);
        }
    });
    Ok(StartRunResult { message_id: ret })
}

/// Helper: clone write Arc from state for background thread use.
fn db_write_arc(state: &State<DbState>) -> std::sync::Arc<std::sync::Mutex<rusqlite::Connection>> {
    std::sync::Arc::clone(&state.write)
}

/// Eval-only: run a prompt through an engine synchronously, return content only.
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
        prompt, model, system_prompt: None, resume_token: None, project_path,
    };
    let result = match engine.as_str() {
        "codex" => codex::run(run_input),
        "gemini" => gemini::run(run_input),
        "opencode" => opencode::run(run_input),
        "ollama" => openai_compat::run(run_input),
        _ => claude::run(run_input),
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
