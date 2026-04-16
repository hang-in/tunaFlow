use rusqlite::{params, Connection};
use uuid::Uuid;

use tauri::Emitter;

use crate::db::{migrations::now_epoch_ms, models::Message};
use crate::db::DbState;
use crate::errors::AppError;

// ─── Embedding semaphore ─────────────────────────────────────────────────────

/// Limits concurrent ONNX inference to 1 at a time.
/// Without this, multiple agent completions spawn multiple threads each running
/// bge-m3 Level3 inference, causing CPU spikes (observed: 558% on 6-core machine).
static EMBED_SEMAPHORE: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::OnceLock::new();

fn embed_semaphore() -> std::sync::Arc<tokio::sync::Semaphore> {
    EMBED_SEMAPHORE
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(1)))
        .clone()
}

use super::super::trace_log::{insert_trace_log, insert_trace_log_with_context, new_span_id, new_trace_id, SpanInfo, ContextPackMeta};
use super::context_loading::{load_context_data, load_project_path};
use super::prompt_assembly::assemble_prompt;
use super::session_freshness;

/// Persist a system message (tool-request results, workflow triggers, etc.)
/// Returns the message ID.
pub fn persist_system_message(
    conn: &Connection,
    conversation_id: &str,
    content: &str,
) -> Result<String, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, status)
         VALUES (?1, ?2, 'system', ?3, ?4, 'done')",
        params![id, conversation_id, content, now],
    )?;
    Ok(id)
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

// ═══════════════════════════════════════════════════════════════════════════
// Shared Phase 1 (prepare) + Phase 3 (finalize) for start_* commands
// ═══════════════════════════════════════════════════════════════════════════

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
    input: &super::super::super::agents::SendWithClaudeInput,
    identity_frag: Option<&str>,
    state: &DbState,
) -> Result<PreparedRun, crate::errors::AppError> {
    // Phase A: DB operations under lock — persist user msg, load context data, pre-create streaming msg
    let (mut data, project_path, msg_id) = {
        let conn = state.write.lock().map_err(|_| crate::errors::AppError::Lock)?;
        persist_user_message(&conn, &input.conversation_id, &input.prompt, &input.user_message_id)?;
        let pp = load_project_path(&conn, &input.project_key);
        let ctx_data = load_context_data(
            &conn, &input.conversation_id, &input.prompt, pp.as_deref(),
            &input.active_skills, &input.cross_session_ids, identity_frag,
            input.context_mode_override.as_deref(), input.context_budget_cap,
            input.user_profile_json.as_deref(),
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

    // Session freshness: stateful 엔진(sdk-url, app-server)에서
    // 같은 세션이 연속되면 ContextPack을 minimal(lite)로 전환.
    let session_key = session_freshness::current_session_key(&input.conversation_id, engine_key);
    if let Some(ref key) = session_key {
        session_freshness::stash_pending(&msg_id, key);
        if session_freshness::is_session_continuation(&input.conversation_id, engine_key) {
            data.context_mode_override = Some("lite".into());
            eprintln!("[session_freshness] continuation detected for conv={} engine={} → lite mode",
                &input.conversation_id[..input.conversation_id.len().min(12)], engine_key);
        } else {
            eprintln!("[session_freshness] new session for conv={} engine={} → full mode",
                &input.conversation_id[..input.conversation_id.len().min(12)], engine_key);
        }
    }

    // Phase B: Pure prompt assembly — no DB lock held
    let (enriched_prompt, system_context, ctx_meta) = assemble_prompt(&data, identity_frag);

    let job_id = format!("job-{}", Uuid::new_v4());
    {
        let conn = state.write.lock().map_err(|_| crate::errors::AppError::Lock)?;
        let _ = super::super::super::jobs::create_job(&conn, &job_id, &input.conversation_id, Some(&msg_id), engine_key, "agent");
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
            // Session freshness: 성공 시 pending → delivered 승격
            session_freshness::promote_pending_to_delivered(msg_id, conversation_id, engine_key);

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
                ctx_meta, Some(msg_id));
            let _ = super::super::super::jobs::complete_job(conn, job_id, "done", None);
            let _ = app.emit("agent:completed", serde_json::json!({
                "messageId": msg_id, "conversationId": conversation_id, "engine": engine_key,
                "durationMs": duration_ms as i64, "inputTokens": out.input_tokens,
                "outputTokens": out.output_tokens, "costUsd": out.cost_usd
            }));
            // Fire-and-forget: update code-review-graph if available
            if let Ok(pp) = conn.query_row(
                "SELECT p.path FROM projects p JOIN conversations c ON c.project_key = p.key WHERE c.id = ?1",
                [conversation_id], |row| row.get::<_, Option<String>>(0),
            ) {
                if let Some(path) = pp {
                    std::thread::spawn(move || { let _ = crate::agents::crg::update(&path); });
                }
            }
        }
        Err(ref e) => {
            // Session freshness: 실패 시 pending 정리 (delivered 승격 안 함)
            session_freshness::discard_pending(msg_id);

            let em = crate::guardrail::fallback_error(engine_key, e);
            let _ = conn.execute(
                "UPDATE messages SET content=?1,status='error',timestamp=?2 WHERE id=?3",
                params![em, now, msg_id],
            );
            // Record error span too — without this, trace history is blind to
            // every failed run (observed: trace_log status=err = 0 across 1058 rows).
            insert_trace_log_with_context(conn, conversation_id, 0, 0, 0.0, now,
                &SpanInfo {
                    trace_id: &new_trace_id(), span_id: new_span_id(), parent_span_id: None,
                    operation: "agent.stream", engine: engine_key,
                    duration_ms: duration_ms as i64, status: "error",
                }, ctx_meta, Some(msg_id));
            let _ = super::super::super::jobs::complete_job(conn, job_id, "error", Some(&em));
            let _ = app.emit("agent:error", serde_json::json!({
                "messageId": msg_id, "conversationId": conversation_id, "engine": engine_key, "error": em
            }));
        }
    }
}

/// Fire-and-forget: spawn post-completion tasks after a successful agent run.
/// Triggers memory compression, session link discovery, and vector indexing.
/// All errors are logged and swallowed — never affects the main response.
pub fn spawn_post_completion_tasks(db: crate::db::DbState, conversation_id: String) {
    std::thread::spawn(move || {
        let cid_short = if conversation_id.len() >= 8 { &conversation_id[..8] } else { &conversation_id };

        // 1. Memory compression — DEFERRED to conversation switch / idle.
        // Running claude -p here while sdk-url session is active causes exit 1 (CLI lock conflict).
        // Compression is triggered by `compress_conversation_memory` Tauri command instead.

        // 2. Session link discovery — read lock for discovery, write lock only for save
        let project_key: Option<String> = {
            if let Ok(conn) = db.read.lock() {
                conn.query_row(
                    "SELECT project_key FROM conversations WHERE id = ?1",
                    [&conversation_id], |r| r.get(0),
                ).ok()
            } else { None }
        };
        if let Some(ref pk) = project_key {
            // Discover with read lock (FTS5 search + rawq embed — can be slow)
            let discovered = {
                if let Ok(conn) = db.read.lock() {
                    crate::commands::session_discovery::discover_related_sessions(
                        &conn, &conversation_id, pk, 5,
                    )
                } else { Vec::new() }
            };
            // Save with short write lock (fast — just INSERTs)
            if !discovered.is_empty() {
                if let Ok(conn) = db.write.lock() {
                    let now = crate::db::migrations::now_epoch_ms();
                    for (linked_id, score) in &discovered {
                        let id = uuid::Uuid::new_v4().to_string();
                        let _ = conn.execute(
                            "INSERT INTO session_links (id, conversation_id, linked_conv_id, score, method, created_at)
                             VALUES (?1, ?2, ?3, ?4, 'fts5', ?5)
                             ON CONFLICT(conversation_id, linked_conv_id) DO UPDATE SET score = ?4, created_at = ?5
                             WHERE method = 'fts5'",
                            rusqlite::params![id, conversation_id, linked_id, score, now],
                        );
                    }
                }
            }
        }

        // 3. Vector indexing (rawq embed — skip if daemon not ready)
        // Semaphore ensures only 1 ONNX embedding job runs at a time, preventing CPU spikes
        // when multiple agents complete concurrently (e.g. RT or rapid sequential sends).
        if crate::agents::rawq::is_daemon_ready() {
            let sem = embed_semaphore();
            // try_acquire — if another thread is already embedding, skip this cycle.
            // The next agent completion will re-index anyway (incremental, low overhead).
            let acquired = sem.try_acquire();
            if acquired.is_ok() {
                match crate::commands::vector_search::index_chunks_blocking(&db, &conversation_id) {
                    Ok(n) if n > 0 => eprintln!("[post-completion] indexed {} new chunks for {}", n, cid_short),
                    Ok(_) => {}
                    Err(e) => eprintln!("[post-completion] vector indexing error: {}", e),
                }
                // acquired (SemaphorePermit) dropped here — releases semaphore
            } else {
                eprintln!("[post-completion] embed semaphore busy, skipping indexing for {} (will catch up next completion)", cid_short);
            }
        }

        // 4. Document re-indexing is NOT triggered here — too expensive for every agent completion.
        //    Triggered by: project selection (1x) + fs watcher (on file change) + manual API call.
    });
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
        insert_trace_log_with_context(conn, conversation_id, run.in_tokens, run.out_tokens, run.cost_usd, now, &span, meta, Some(&msg_id));
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
        duration_ms: None, input_tokens: None, output_tokens: None, cost_usd: None,
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
        insert_trace_log_with_context(conn, conversation_id, run.in_tokens, run.out_tokens, run.cost_usd, now, &span, meta, Some(msg_id));
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
        duration_ms: None, input_tokens: None, output_tokens: None, cost_usd: None,
    })
}
