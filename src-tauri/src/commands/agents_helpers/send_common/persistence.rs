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

static COMPRESS_SEMAPHORE: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::OnceLock::new();

fn compress_semaphore() -> std::sync::Arc<tokio::sync::Semaphore> {
    COMPRESS_SEMAPHORE
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(1)))
        .clone()
}

/// Global post-completion serialization. **전체 task (compression + session link
/// + vector index) 를 직렬화**. 2026-04-22 재현 분석 결과, 매 turn 마다
/// `std::thread::spawn` 으로 새 thread 를 띄우는데 이 thread 들이 compression
/// / session link / vector index 를 각자 병렬로 진행하면서 write lock 을
/// 쉴새 없이 번갈아 잡는다. 결과: 다음 user turn 의 `prepare_engine_run` 의
/// A1 write lock 획득이 starvation 으로 영원히 실패. 이 세마포어로 한 번에
/// 하나의 post-completion 만 돌도록 제한 → 다음 turn 의 A1 에게 틈을 제공.
static POST_COMPLETION_LOCK: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<()>>> =
    std::sync::OnceLock::new();

fn post_completion_lock() -> std::sync::Arc<std::sync::Mutex<()>> {
    POST_COMPLETION_LOCK
        .get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(())))
        .clone()
}

use super::super::trace_log::{insert_trace_log, insert_trace_log_with_context, new_span_id, new_trace_id, SpanInfo, ContextPackMeta};
use super::agent_session_tx;
use super::context_loading::{load_context_data, load_project_path};
use super::prompt_assembly::assemble_prompt;
use super::session_freshness;

/// 엔진 전환 감지: 동일 대화 안에서 마지막 assistant 응답의 engine 이 현재
/// 요청의 engine 과 다르면 handoff 블록을 생성한다.
///
/// 시나리오: Claude 구독량 초과 → Codex 로 전환 후 같은 역할로 계속. 새 엔진이
/// "이전 엔진의 분석" 을 자신이 반복해야 할 작업으로 오인하지 않도록 명시한다.
pub fn detect_engine_handoff(
    conn: &Connection,
    conversation_id: &str,
    current_engine: &str,
    _current_persona: Option<&str>,
) -> Option<String> {
    // 마지막 assistant 메시지 (done 상태) — engine / persona / content preview
    let row: Option<(String, Option<String>, String)> = conn
        .query_row(
            "SELECT COALESCE(engine,''), persona, content
             FROM messages
             WHERE conversation_id = ?1 AND role = 'assistant' AND status = 'done'
             ORDER BY timestamp DESC LIMIT 1",
            [conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();
    let (prev_engine, prev_persona, prev_content) = row?;
    if prev_engine.is_empty() { return None; }

    // 엔진 식별자 정규화 — claude-code/claude/codex/gemini/ollama 수준으로 비교
    let norm = |e: &str| -> String {
        let l = e.to_ascii_lowercase();
        if l.starts_with("claude") { "claude".into() }
        else if l.starts_with("codex") || l.starts_with("openai") { "codex".into() }
        else if l.starts_with("gemini") { "gemini".into() }
        else if l.starts_with("ollama") { "ollama".into() }
        else if l.starts_with("lmstudio") || l.starts_with("openai_compat") || l.starts_with("openai-compat") { "lmstudio".into() }
        else { l }
    };
    if norm(&prev_engine) == norm(current_engine) { return None; }

    let persona_label = prev_persona.as_deref().filter(|s| !s.is_empty()).unwrap_or("the previous agent");
    // 이전 응답 preview — 앞 400자
    let preview: String = prev_content.chars().take(400).collect();
    let preview = if prev_content.chars().count() > 400 { format!("{}…", preview) } else { preview };

    Some(format!(
        "## Handoff Notice\n\
         In this conversation the engine just switched from **{prev}** to **{curr}** (same role: {role}).\n\
         Reason: quota exhaustion or user-initiated engine change — NOT a restart of the task.\n\n\
         **Rules for this turn:**\n\
         1. Treat {prev}'s previous conclusions as established context — do NOT redo the same diagnosis, file lookup, or plan drafting that is already captured below.\n\
         2. Continue from where {prev} stopped. If its last message was a plan/proposal, evaluate or extend it instead of re-deriving it.\n\
         3. If something in {prev}'s output looks wrong, say so explicitly and point to the specific claim — don't silently restart.\n\
         4. Your persona and goal remain the same ({role}). Do not rebrand yourself as a fresh reviewer.\n\n\
         **Last message from {prev} ({role}):**\n\
         > {preview}\n",
        prev = prev_engine,
        curr = current_engine,
        role = persona_label,
        preview = preview.replace('\n', "\n> "),
    ))
}

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
    /// Phase 3b-part1: session audit id for lifecycle tracking. `None` when
    /// `TUNAFLOW_AGENT_SESSION_AUDIT=0` disables auditing. Populated alongside
    /// the Phase A3 write so only one lock acquisition is paid.
    pub audit_session_id: Option<String>,
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
    // Phase A 를 3개로 분할 (2026-04-22 audit): write lock 을 최소한만 잡도록.
    //
    // 기존에는 user msg INSERT + load_context_data + streaming msg INSERT 를
    // **한 write lock 안에서** 수행했다. 문제는 load_context_data 가 read-heavy
    // (여러 SELECT · retrieval · memory) 라 수백 ms 걸리는데 그동안 write lock
    // 을 hold. 이전 turn 의 post-completion hook (vector indexing) 이 이미
    // write lock 을 잡고 있으면 prepare_engine_run 이 통째로 대기 → 다음 user
    // turn 진입 실패 (sdk-session 로그 자체가 안 뜸, 45s orphan-recovery).
    //
    // 분할:
    //   A0 (read):  detect_engine_handoff — 이전 assistant turn 조회
    //   A1 (write, 수 ms): persist_user_message
    //   A2 (read):  load_project_path + load_context_data
    //   A3 (write, 수 ms): pre-create streaming assistant msg
    // WAL 에서 read 는 writer 와 무관. write 를 짧게 두 번 잡는 것으로 충분.

    // Phase A0: read — engine handoff 감지
    let handoff_block = {
        let conn = state.read.lock().map_err(|_| crate::errors::AppError::Lock)?;
        detect_engine_handoff(&conn, &input.conversation_id, engine_key, input.persona_label.as_deref())
    };

    // Phase A1: short write — user message persist
    {
        let conn = state.write.lock().map_err(|_| crate::errors::AppError::Lock)?;
        persist_user_message(&conn, &input.conversation_id, &input.prompt, &input.user_message_id)?;
    }

    // Phase A2: read — project path + context data (가장 시간이 많이 걸리는 구간)
    let (mut data, project_path) = {
        let conn = state.read.lock().map_err(|_| crate::errors::AppError::Lock)?;
        let pp = load_project_path(&conn, &input.project_key);
        let mut ctx_data = load_context_data(
            &conn, &input.conversation_id, &input.prompt, pp.as_deref(),
            &input.active_skills, &input.cross_session_ids, identity_frag,
            input.context_mode_override.as_deref(), input.context_budget_cap,
            input.user_profile_json.as_deref(),
        );
        // Layer B (branchInheritsMainSessionPlan): brand 가 main 의 sdk-url WS
        // 세션을 그대로 이어받을 수 있는 조건 (same engine) 이면 dynamic
        // ContextPack 섹션을 비운다. 정적 레이어 (identity/persona/project) 는
        // 매 send 마다 system_prompt 로 다시 들어가야 하므로 유지.
        super::context_loading::apply_branch_session_inheritance(
            &conn,
            &mut ctx_data,
            engine_key,
        );
        (ctx_data, pp)
    };

    // Phase A3: short write — pre-create streaming assistant message + audit begin
    // Audit row is inserted under the same write lock as the streaming message to
    // amortize lock-acquisition cost. If auditing is disabled, `audit_session_id`
    // stays None and no extra write happens.
    let (msg_id, audit_session_id) = {
        let conn = state.write.lock().map_err(|_| crate::errors::AppError::Lock)?;
        let mid = Uuid::new_v4().to_string();
        let now = now_epoch_ms();
        conn.execute(
            "INSERT INTO messages(id,conversation_id,role,content,timestamp,status,engine,model,persona)\
             VALUES(?1,?2,'assistant','',?3,'streaming',?4,?5,?6)",
            params![mid, input.conversation_id, now, engine_key, input.model, input.persona_label],
        )?;
        let sid = if agent_session_tx::audit_enabled() {
            match agent_session_tx::audit_begin(&conn, Some(&input.conversation_id)) {
                Ok(s) => Some(s),
                Err(e) => {
                    // Audit is observability only — never fail the user turn for it
                    eprintln!("[agent-session-tx] audit_begin failed: {:?}", e);
                    None
                }
            }
        } else {
            None
        };
        (mid, sid)
    };

    // Session freshness: stateful 엔진(sdk-url, app-server)에서
    // 같은 세션이 연속되면 recent_context + compressed_memory 섹션을 **drop** 한다.
    // 이유: Claude 자체 세션 히스토리가 source-of-truth. tunaFlow 가 truncate 된 prepend 를
    // 넣으면 오염 유발(사용자 지적). 에이전트는 필요시 `tool-request:recent_turns:N` 로 명시 조회.
    //
    // fresh 세션 (엔진 전환/첫 send/session crash 후 re-spawn) 이면 full 모드 + anchor 2 turn 으로
    // tunaFlow 가 history 제공자 역할.
    let session_key = session_freshness::current_session_key(&input.conversation_id, engine_key);
    if let Some(ref key) = session_key {
        session_freshness::stash_pending(&msg_id, key);
        if session_freshness::is_session_continuation(&input.conversation_id, engine_key) {
            data.is_session_continuation = true;
            eprintln!("[session_freshness] continuation conv={} engine={} → drop recent_context + compressed_memory (rely on Claude session)",
                &input.conversation_id[..input.conversation_id.len().min(12)], engine_key);
        } else {
            eprintln!("[session_freshness] new session conv={} engine={} → full mode + anchor 2 turns",
                &input.conversation_id[..input.conversation_id.len().min(12)], engine_key);
        }
    }

    // Phase B: Pure prompt assembly — no DB lock held
    let (mut enriched_prompt, system_context, ctx_meta) = assemble_prompt(&data, identity_frag);

    // 엔진 전환 감지 시 handoff 블록을 enriched_prompt 최상단에 주입.
    // 새 엔진(Codex 등) 이 "이전 엔진이 내린 결정" 을 그대로 이어받고, 검증 루프에
    // 빠지지 않도록 명시적으로 안내.
    if let Some(block) = handoff_block {
        enriched_prompt = format!("{}\n\n{}", block, enriched_prompt);
    }

    let job_id = format!("job-{}", Uuid::new_v4());
    {
        let conn = state.write.lock().map_err(|_| crate::errors::AppError::Lock)?;
        let _ = super::super::super::jobs::create_job(&conn, &job_id, &input.conversation_id, Some(&msg_id), engine_key, "agent");
    }

    Ok(PreparedRun { msg_id, job_id, enriched_prompt, system_context, project_path, ctx_meta, audit_session_id })
}

/// Phase 3: Persist engine result, update conversation usage, emit events.
///
/// Called from the background thread after the engine finishes.
/// `audit_session_id` is the id returned from `prepare_engine_run` — `None`
/// when auditing is disabled. The audit row is finalized (committed / rolled
/// back) here based on `result`.
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
    audit_session_id: Option<&str>,
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
            if let Some(sid) = audit_session_id {
                let _ = agent_session_tx::audit_commit(conn, sid);
            }
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
            if let Some(sid) = audit_session_id {
                let _ = agent_session_tx::audit_rollback(conn, sid, &em);
            }
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

        // ⚠️ Global serialization — 한 번에 하나의 post-completion 만 실행.
        // 기존에는 매 turn 마다 새 thread 가 compression / session link /
        // vector index 를 각자 병렬 실행 → write lock 쉴 새 없이 잡힘 →
        // 다음 turn 의 prepare_engine_run A1 이 영구 starvation. 이 lock 으로
        // serialize 하면 A1 이 post-completion 사이 틈에 확실히 진입 가능.
        let lock_arc = post_completion_lock();
        let _guard = match lock_arc.lock() {
            Ok(g) => g,
            Err(e) => {
                eprintln!("[post-completion] global lock poisoned: {}", e);
                return;
            }
        };

        // 1. Memory compression — eager trigger (P1 #3 option B, two-turn window).
        //    Runs Haiku summarization in background after every completion.
        //    `compress_memory_blocking` prefers Anthropic SDK (no CLI conflict with
        //    an active sdk-url session) and falls back to CLI -p. Internal
        //    `needs_compression` threshold check keeps this cheap when not needed.
        //    Semaphore ensures only one compression runs at a time — concurrent
        //    completions (RT, rapid sends) queue or skip; the next completion will
        //    reattempt if threshold is still met.
        //    If compression completes before the next user turn, that turn's
        //    ContextPack sees fresh topics and `memory:TOPIC` tool-requests can
        //    hit them immediately. If not, the batch/idle path still catches up.
        {
            let sem = compress_semaphore();
            if sem.try_acquire().is_ok() {
                match crate::commands::memory_compression::compress_memory_blocking(&db, &conversation_id) {
                    Ok(true) => eprintln!("[post-completion] memory compressed for {}", cid_short),
                    Ok(false) => {} // threshold not met or empty transcript — silent
                    Err(e) => eprintln!("[post-completion] compression error: {}", e),
                }
            } else {
                eprintln!("[post-completion] compression semaphore busy, skipping for {}", cid_short);
            }
        }

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

        // 3. Vector indexing (rawq embed — skip if daemon not ready).
        // Semaphore ensures only 1 ONNX embedding job runs at a time, preventing CPU spikes
        // when multiple agents complete concurrently (e.g. RT or rapid sequential sends).
        if crate::agents::rawq::is_daemon_ready() {
            let sem = embed_semaphore();
            let acquired = sem.try_acquire();
            if acquired.is_ok() {
                match crate::commands::vector_search::index_chunks_blocking(&db, &conversation_id) {
                    Ok(n) if n > 0 => eprintln!("[post-completion] indexed {} new chunks for {}", n, cid_short),
                    Ok(_) => {}
                    Err(e) => eprintln!("[post-completion] vector indexing error: {}", e),
                }
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

#[cfg(test)]
mod handoff_tests {
    use super::detect_engine_handoff;
    use rusqlite::Connection;

    fn setup_conv(engine: &str, content: &str) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL, role TEXT NOT NULL,
                content TEXT NOT NULL, timestamp INTEGER NOT NULL, status TEXT NOT NULL,
                engine TEXT, model TEXT, persona TEXT
            );",
        ).unwrap();
        conn.execute(
            "INSERT INTO messages(id,conversation_id,role,content,timestamp,status,engine,persona)
             VALUES('m1','c1','user','hi',1000,'done',NULL,NULL)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO messages(id,conversation_id,role,content,timestamp,status,engine,persona)
             VALUES('m2','c1','assistant',?1,2000,'done',?2,'Architect')",
            rusqlite::params![content, engine],
        ).unwrap();
        conn
    }

    #[test]
    fn no_handoff_when_engine_matches() {
        let conn = setup_conv("claude-code", "previous analysis");
        assert!(detect_engine_handoff(&conn, "c1", "claude-code", Some("Architect")).is_none());
    }

    #[test]
    fn no_handoff_when_only_aliases_differ() {
        let conn = setup_conv("claude-code", "previous analysis");
        // claude-code vs claude → both normalize to "claude"
        assert!(detect_engine_handoff(&conn, "c1", "claude", Some("Architect")).is_none());
    }

    #[test]
    fn handoff_emitted_when_engine_differs() {
        let conn = setup_conv("claude-code", "Plan draft: use JWT auth, split migration into two phases.");
        let block = detect_engine_handoff(&conn, "c1", "codex", Some("Architect")).unwrap();
        assert!(block.contains("Handoff"));
        assert!(block.contains("claude-code"));
        assert!(block.contains("codex"));
        assert!(block.contains("Architect"));
        assert!(block.contains("Plan draft"));
    }

    #[test]
    fn no_handoff_when_no_previous_assistant() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL, role TEXT NOT NULL,
                content TEXT NOT NULL, timestamp INTEGER NOT NULL, status TEXT NOT NULL,
                engine TEXT, model TEXT, persona TEXT
            );",
        ).unwrap();
        assert!(detect_engine_handoff(&conn, "c1", "codex", None).is_none());
    }
}
