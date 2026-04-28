use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::agents::{anthropic_sdk, claude, claude_sdk_session, codex, codex_app_server, gemini, gemini_sdk, openai_compat, openai_sdk, opencode};
use crate::db::DbState;
use crate::errors::AppError;
use crate::guardrail;

use super::agents_helpers::context_pack::assemble_system_prompt;
use super::agents_helpers::send_common::{prepare_engine_run, finalize_engine_run, spawn_post_completion_tasks, PreparedRun};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkPayload {
    pub message_id: String,
    pub conversation_id: String,
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
    /// Serialized user profile JSON (from frontend settings store).
    #[serde(default)]
    pub user_profile_json: Option<String>,
    /// Engine key from frontend (e.g. "ollama", "lmstudio") for backend routing.
    #[serde(default)]
    pub engine: Option<String>,
    /// Absolute paths of image attachments. Used by Codex for `-i <path>` argv
    /// (CLI path supports vision via argv, not prompt text). Other engines can
    /// read the image via `Read` tool from the prompt path section.
    #[serde(default)]
    pub image_paths: Vec<String>,
    /// Base URL override for OpenAI-compatible engines (ollama / lmstudio).
    /// Empty string and None both mean "fallback to env / hardcoded default".
    /// Issue #175: lets users point at remote Ollama (Tailscale / NAS) or an
    /// alternate LM Studio port without relying on env vars.
    #[serde(default)]
    pub custom_base_url: Option<String>,
}

/// Wrap persona_fragment with identity framing block for a given engine.
fn identity_fragment(input: &SendWithClaudeInput, engine: &str) -> Option<String> {
    super::agents_helpers::send_common::build_identity_persona_fragment(
        input.persona_label.as_deref(),
        engine,
        input.persona_fragment.as_deref(),
        input.model.as_deref(),
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// Mode query
// ═══════════════════════════════════════════════════════════════════════════

/// 현재 conversation에 적용될 claude 전송 모드를 반환한다.
/// - "cli"     : `-p --session-id`/`--resume` stream-json (기본, claude 2.1.121+ 정책 호환)
/// - "sdk-url" : --sdk-url WS 세션 (TUNAFLOW_USE_SDK_URL=1 명시 시만, Anthropic 차단됨)
/// - "sdk"     : Anthropic SDK 직접 (API 키 + branch)
///
/// 2026-04-29 default flip — claude CLI 2.1.121 의 `--sdk-url` localhost reject
/// 정책으로 sdk-session 영구 차단. CLI -p `--resume` 가 동등 stateful path
/// (`docs/plans/claudeResumeSessionTransitionPlan_2026-04-29.md`).
fn resolve_claude_mode(conversation_id: &str) -> &'static str {
    let is_branch = conversation_id.starts_with("branch:");
    if anthropic_sdk::is_available() && is_branch {
        "sdk"
    } else if std::env::var("TUNAFLOW_USE_SDK_URL").as_deref() == Ok("1") {
        "sdk-url"
    } else {
        "cli"
    }
}

#[tauri::command]
pub fn get_claude_mode(conversation_id: String) -> String {
    resolve_claude_mode(&conversation_id).to_string()
}

/// Hard-restart the claude session — process kill + resume_token DB clear.
///
/// 이 명령은 **session kill** 의미다. UI cancel 버튼과는 분리된 경로이며,
/// 명시적 시나리오 (engine/model 변경, 외부 process 망실 복구 등) 에서만
/// 호출해야 한다. 일반 cancel 은 `cancel_running` (stream abort only) 사용
/// — `docs/plans/branchCancelSemanticsPlan_2026-04-25.md` 참조.
///
/// 2026-04-29 — resume-session path 에서도 DB resume_token NULL 처리. CLI -p
/// path 는 process 가 매 message spawn 이라 *kill* 의미는 DB clear 만으로 충분
/// (다음 send 가 신규 session 으로 시작).
#[tauri::command]
pub fn restart_sdk_session(conversation_id: String, state: State<DbState>) {
    let mode = resolve_claude_mode(&conversation_id);
    if mode == "sdk-url" {
        // Legacy sdk-session — 메모리 RESUME_IDS clear + process kill + DB clear
        claude_sdk_session::kill_session_clear_resume(&conversation_id);
    } else if mode == "cli" {
        // resume-session path — DB resume_token NULL. 다음 send 시 RunInput.resume_token=None
        // → claude 가 신규 session_id 생성 → finalize_engine_run 이 새로 DB 저장.
        // 메모리 RESUME_IDS 도 clear (CLI path 가 사용 안 하지만 sdk-url 와 공유 cache).
        claude_sdk_session::kill_session_clear_resume(&conversation_id);
        if let Ok(conn) = state.write.lock() {
            let _ = conn.execute(
                "UPDATE conversations SET resume_token = NULL \
                 WHERE id = ?1 AND resume_token_engine IN ('claude','claude-code')",
                [&conversation_id],
            );
        }
    }
}

#[tauri::command]
pub async fn prewarm_sdk_session(
    conversation_id: String,
    project_path: Option<String>,
    model: Option<String>,
    state: State<'_, DbState>,
) -> Result<(), ()> {
    if resolve_claude_mode(&conversation_id) == "sdk-url" {
        // Bootstrap RESUME_IDS 메모리 레지스트리 — 앱 재시작 후 첫 접근 시
        // `--resume` 연속성 회복 (sessionContinuityFixPlan task-02 / INV-4).
        claude_sdk_session::bootstrap_resume_id_from_db(&conversation_id, state.inner());
        claude_sdk_session::prewarm_session(
            &conversation_id,
            project_path.as_deref(),
            model.as_deref(),
        ).await;
    }
    Ok(())
}

#[tauri::command]
pub fn has_active_sdk_session(conversation_id: String) -> bool {
    claude_sdk_session::has_active_session(&conversation_id)
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
    let db_post = state.inner().clone();
    let id_frag = identity_fragment(&input, "claude-code");
    let cancel_arc = std::sync::Arc::clone(&cancel.0);
    let write_arc = db_write_arc(&state);

    // Extract values needed after spawn_blocking (input will be moved)
    let cid = input.conversation_id.clone();
    let pr = input.prompt.clone();
    let mo = input.model.clone();

    // DB-heavy work off main thread
    let (prep, system_prompt) = tokio::task::spawn_blocking(move || -> Result<_, AppError> {
        let prep = prepare_engine_run("claude-code", &input, id_frag.as_deref(), &db)?;

        let system_prompt = {
            let _conn = db.write.lock().map_err(|_| AppError::Lock)?;
            let agent_sp = assemble_system_prompt(
                input.agent_name.as_deref(), prep.project_path.as_deref(), input.system_prompt.as_deref(),
            );
            match (prep.system_context.clone(), agent_sp) {
                (Some(c), Some(a)) => Some(format!("{}\n\n{}", c, a)),
                (c @ Some(_), None) => c,
                (None, a @ Some(_)) => a,
                (None, None) => None,
            }
        };

        Ok((prep, system_prompt))
    }).await.map_err(|_| AppError::Lock)??;

    let ret = prep.msg_id.clone();
    let ep = prep.enriched_prompt.clone();
    let PreparedRun { msg_id, job_id, project_path, ctx_meta, audit_session_id, .. } = prep;
    let plen = pr.len() + system_prompt.as_ref().map_or(0, |s| s.len());

    // Claude 실행 경로 선택 (2026-04-29 default flip):
    // 1. ANTHROPIC_API_KEY + branch → Anthropic SDK 직접 호출
    // 2. 기본 → CLI -p `--session-id`/`--resume` stream-json (정책 호환 stateful)
    // 3. TUNAFLOW_USE_SDK_URL=1 → --sdk-url WS 세션 (Anthropic 차단됨, 명시 활성화 시만)
    //
    // 2.1.121 의 --sdk-url localhost reject 로 sdk-session 영구 차단 →
    // CLI -p path 가 default. resume_token DB 에서 읽어 conversation
    // continuity 유지. SSOT: claudeResumeSessionTransitionPlan_2026-04-29.md
    let use_sdk = anthropic_sdk::is_available() && cid.starts_with("branch:");
    let use_sdk_url = !use_sdk && std::env::var("TUNAFLOW_USE_SDK_URL").as_deref() == Ok("1");

    if use_sdk {
        let sp = system_prompt;
        let cid2 = cid.clone();
        let db_p = db_post.clone();
        let aid = audit_session_id.clone();
        tokio::spawn(async move {
            let pa = app.clone(); let pi = msg_id.clone(); let pc = cid2.clone();
            let c2 = app.clone(); let ci = msg_id.clone(); let cc = cid2.clone();
            let t0 = std::time::Instant::now();
            let rr = anthropic_sdk::stream_run(
                claude::RunInput { prompt: pr, model: mo.clone(), system_prompt: sp, resume_token: None, project_path, image_paths: Vec::new() },
                move |t| { let _ = pa.emit("claude:progress", ChunkPayload { message_id: pi.clone(), conversation_id: pc.clone(), text: t }); },
                move |t| { let _ = c2.emit("claude:chunk", ChunkPayload { message_id: ci.clone(), conversation_id: cc.clone(), text: t }); },
            ).await;
            let dur = t0.elapsed().as_millis();
            guardrail::log_run("claude-sdk", mo.as_deref(), dur, plen, rr.is_ok());
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "claude-code", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app, aid.as_deref());
            }
            if rr.is_ok() { spawn_post_completion_tasks(db_p, cid); }
        });
    } else if use_sdk_url {
        // --sdk-url WS 세션: 구조화 JSON + 슬래시 커맨드 지원
        //
        // Bootstrap RESUME_IDS 메모리 레지스트리 (sessionContinuityFixPlan task-02).
        // 앱 재시작 후 첫 send 에서 `--resume` 연속성을 회복. 이미 메모리에 있으면
        // no-op. DB read 만 사용해 write 경합 없음.
        //
        // `db` 는 spawn_blocking 으로 이미 move 됐으므로 동일 scope 에 살아있는
        // `db_post` 참조를 빌려 쓴다 (DbState 는 Arc 기반 clone 저렴, 참조만 사용).
        claude_sdk_session::bootstrap_resume_id_from_db(&cid, &db_post);

        let cid2 = cid.clone();
        let db_p = db_post.clone();
        let aid = audit_session_id.clone();
        tokio::spawn(async move {
            let pa = app.clone(); let pi = msg_id.clone(); let pc = cid2.clone();
            let c2 = app.clone(); let ci = msg_id.clone(); let cc = cid2.clone();
            let t0 = std::time::Instant::now();
            let rr = claude_sdk_session::stream_run_sdk(
                &cid2,
                claude::RunInput { prompt: ep, model: mo.clone(), system_prompt: None, resume_token: None, project_path, image_paths: Vec::new() },
                move |t| { let _ = pa.emit("claude:progress", ChunkPayload { message_id: pi.clone(), conversation_id: pc.clone(), text: t }); },
                move |t| { let _ = c2.emit("claude:chunk", ChunkPayload { message_id: ci.clone(), conversation_id: cc.clone(), text: t }); },
                { let c = cid2.clone(); let r = cancel_arc; move || { r.lock().remove(&c) } },
            ).await;
            let dur = t0.elapsed().as_millis();
            guardrail::log_run("claude-sdk-url", mo.as_deref(), dur, plen, rr.is_ok());
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "claude-code", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app, aid.as_deref());
            }
            if rr.is_ok() { spawn_post_completion_tasks(db_p, cid); }
        });
    } else {
        // CLI -p path (default 2026-04-29) — `--resume <id>` stateful conversation.
        // DB 의 resume_token 읽어 RunInput 에 전달 → claude internal session store
        // 가 prior history reload + 응답 + finalize_engine_run 이 새 session_id 를
        // DB 에 다시 저장 (persistence.rs:356-359).
        // 첫 send 면 resume_token=None → claude 가 신규 session 생성.
        let resume_token = claude_sdk_session::bootstrap_resume_id_from_db(&cid, &db_post);
        let aid = audit_session_id.clone();
        std::thread::spawn(move || {
            let pa = app.clone(); let pi = msg_id.clone(); let pc = cid.clone();
            let c2 = app.clone(); let ci = msg_id.clone(); let cc = cid.clone();
            let t0 = std::time::Instant::now();
            let rr = claude::stream_run(
                claude::RunInput { prompt: pr, model: mo.clone(), system_prompt, resume_token, project_path, image_paths: Vec::new() },
                move |t| { let _ = pa.emit("claude:progress", ChunkPayload { message_id: pi.clone(), conversation_id: pc.clone(), text: t }); },
                move |t| { let _ = c2.emit("claude:chunk", ChunkPayload { message_id: ci.clone(), conversation_id: cc.clone(), text: t }); },
                { let c = cid.clone(); let r = cancel_arc; move || { r.lock().remove(&c) } },
            );
            let dur = t0.elapsed().as_millis();
            guardrail::log_run("claude-resume", mo.as_deref(), dur, plen, rr.is_ok());
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "claude-code", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app, aid.as_deref());
            }
            if rr.is_ok() { spawn_post_completion_tasks(db_post, cid); }
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
    let db_post = state.inner().clone();
    let id_frag = identity_fragment(&input, "gemini");
    let cancel_arc = std::sync::Arc::clone(&cancel.0);
    let write_arc = db_write_arc(&state);
    let cid = input.conversation_id.clone();
    let mo = input.model.clone();

    let prep = tokio::task::spawn_blocking(move || {
        prepare_engine_run("gemini", &input, id_frag.as_deref(), &db)
    }).await.map_err(|_| AppError::Lock)??;

    let ret = prep.msg_id.clone();
    let system_prompt_opt = prep.system_context.clone();
    let PreparedRun { msg_id, job_id, enriched_prompt, project_path, ctx_meta, audit_session_id, .. } = prep;

    if gemini_sdk::is_available() {
        // SDK path — async, native streaming, accurate token tracking
        let system_prompt = system_prompt_opt;
        let cid2 = cid.clone();
        let db_p = db_post.clone();
        let aid = audit_session_id.clone();
        tokio::spawn(async move {
            let pa = app.clone(); let pi = msg_id.clone(); let pc = cid2.clone();
            let c2 = app.clone(); let ci = msg_id.clone(); let cc = cid2.clone();
            let t0 = std::time::Instant::now();
            let rr = gemini_sdk::stream_run(
                claude::RunInput { prompt: enriched_prompt, model: mo.clone(), system_prompt, resume_token: None, project_path, image_paths: Vec::new() },
                move |t| { let _ = pa.emit("gemini:progress", ChunkPayload { message_id: pi.clone(), conversation_id: pc.clone(), text: t }); },
                move |t| { let _ = c2.emit("gemini:chunk", ChunkPayload { message_id: ci.clone(), conversation_id: cc.clone(), text: t }); },
            ).await;
            let dur = t0.elapsed().as_millis();
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "gemini", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app, aid.as_deref());
            }
            if rr.is_ok() { spawn_post_completion_tasks(db_p, cid); }
        });
    } else {
        // CLI fallback
        let aid = audit_session_id.clone();
        std::thread::spawn(move || {
            let pa = app.clone(); let pi = msg_id.clone(); let pc = cid.clone();
            let c2 = app.clone(); let ci = msg_id.clone(); let cc = cid.clone();
            let t0 = std::time::Instant::now();
            let rr = gemini::stream_run(
                claude::RunInput { prompt: enriched_prompt, model: mo.clone(), system_prompt: None, resume_token: None, project_path, image_paths: Vec::new() },
                move |t| { let _ = pa.emit("gemini:progress", ChunkPayload { message_id: pi.clone(), conversation_id: pc.clone(), text: t }); },
                move |t| { let _ = c2.emit("gemini:chunk", ChunkPayload { message_id: ci.clone(), conversation_id: cc.clone(), text: t }); },
                { let c = cid.clone(); let r = cancel_arc; move || { r.lock().remove(&c) } },
            );
            let dur = t0.elapsed().as_millis();
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "gemini", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app, aid.as_deref());
            }
            if rr.is_ok() { spawn_post_completion_tasks(db_post, cid); }
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
    let db_post = state.inner().clone();
    let id_frag = identity_fragment(&input, "codex");
    let write_arc = db_write_arc(&state);
    let cid = input.conversation_id.clone();
    let mo = input.model.clone();
    let image_paths = input.image_paths.clone();

    let prep = tokio::task::spawn_blocking(move || {
        prepare_engine_run("codex", &input, id_frag.as_deref(), &db)
    }).await.map_err(|_| AppError::Lock)??;

    let ret = prep.msg_id.clone();
    let system_prompt_opt = prep.system_context.clone();
    let PreparedRun { msg_id, job_id, enriched_prompt, project_path, ctx_meta, audit_session_id, .. } = prep;

    if openai_sdk::is_available() {
        // SDK path — OpenAI Chat Completions API
        let system_prompt = system_prompt_opt;
        let cid2 = cid.clone();
        let db_p = db_post.clone();
        let aid = audit_session_id.clone();
        tokio::spawn(async move {
            let pa = app.clone(); let pi = msg_id.clone(); let pc = cid2.clone();
            let c2 = app.clone(); let ci = msg_id.clone(); let cc = cid2.clone();
            let t0 = std::time::Instant::now();
            let rr = openai_sdk::stream_run(
                claude::RunInput { prompt: enriched_prompt, model: mo.clone(), system_prompt, resume_token: None, project_path, image_paths: Vec::new() },
                move |t| { let _ = pa.emit("codex:progress", ChunkPayload { message_id: pi.clone(), conversation_id: pc.clone(), text: t }); },
                move |t| { let _ = c2.emit("codex:chunk", ChunkPayload { message_id: ci.clone(), conversation_id: cc.clone(), text: t }); },
            ).await;
            let dur = t0.elapsed().as_millis();
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "codex", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app, aid.as_deref());
            }
            if rr.is_ok() { spawn_post_completion_tasks(db_p, cid); }
        });
    } else if codex_app_server::is_available() {
        // Codex app-server 지속 세션 (매번 재스폰 없음)
        let cid2 = cid.clone();
        let aid = audit_session_id.clone();
        tokio::spawn(async move {
            let pa = app.clone(); let pi = msg_id.clone(); let pc = cid2.clone();
            let c2 = app.clone(); let ci = msg_id.clone(); let cc = cid2.clone();
            let t0 = std::time::Instant::now();
            let rr = codex_app_server::stream_run_app_server(
                &cid,
                claude::RunInput { prompt: enriched_prompt, model: mo.clone(), system_prompt: None, resume_token: None, project_path, image_paths: Vec::new() },
                move |t| { let _ = pa.emit("codex:progress", ChunkPayload { message_id: pi.clone(), conversation_id: pc.clone(), text: t }); },
                move |t| { let _ = c2.emit("codex:chunk", ChunkPayload { message_id: ci.clone(), conversation_id: cc.clone(), text: t }); },
                || false,
            ).await;
            let dur = t0.elapsed().as_millis();
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "codex", &msg_id, &cid2, &job_id, &rr, dur, &ctx_meta, &app, aid.as_deref());
            }
            if rr.is_ok() { spawn_post_completion_tasks(db_post, cid2); }
        });
    } else {
        // Codex CLI fallback
        let aid = audit_session_id.clone();
        std::thread::spawn(move || {
            let chunk_mid = msg_id.clone(); let chunk_app = app.clone(); let chunk_cid = cid.clone();
            let progress_mid = msg_id.clone(); let progress_app = app.clone(); let progress_cid = cid.clone();
            let t0 = std::time::Instant::now();
            let rr = codex::stream_run(
                claude::RunInput { prompt: enriched_prompt, model: mo.clone(), system_prompt: None, resume_token: None, project_path, image_paths },
                |event_type| { let _ = progress_app.emit("codex:progress", ChunkPayload { message_id: progress_mid.clone(), conversation_id: progress_cid.clone(), text: event_type.to_string() }); },
                |accumulated| { let _ = chunk_app.emit("codex:chunk", ChunkPayload { message_id: chunk_mid.clone(), conversation_id: chunk_cid.clone(), text: accumulated.to_string() }); },
            );
            let dur = t0.elapsed().as_millis();
            if let Ok(conn) = write_arc.lock() {
                finalize_engine_run(&conn, "codex", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app, aid.as_deref());
            }
            if rr.is_ok() { spawn_post_completion_tasks(db_post, cid); }
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
    let db_post = state.inner().clone();
    let id_frag = identity_fragment(&input, "opencode");
    let write_arc = db_write_arc(&state);
    let cid = input.conversation_id.clone();
    let mo = input.model.clone();

    let prep = tokio::task::spawn_blocking(move || {
        prepare_engine_run("opencode", &input, id_frag.as_deref(), &db)
    }).await.map_err(|_| AppError::Lock)??;

    let ret = prep.msg_id.clone();
    let PreparedRun { msg_id, job_id, enriched_prompt, project_path, ctx_meta, audit_session_id, .. } = prep;

    std::thread::spawn(move || {
        let _ = app.emit("opencode:progress", ChunkPayload { message_id: msg_id.clone(), conversation_id: cid.clone(), text: "OpenCode starting...".into() });
        let t0 = std::time::Instant::now();
        let rr = opencode::run(
            claude::RunInput { prompt: enriched_prompt, model: mo.clone(), system_prompt: None, resume_token: None, project_path, image_paths: Vec::new() },
        );
        let dur = t0.elapsed().as_millis();
        if let Ok(conn) = write_arc.lock() {
            finalize_engine_run(&conn, "opencode", &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app, audit_session_id.as_deref());
        }
        if rr.is_ok() { spawn_post_completion_tasks(db_post, cid); }
    });
    Ok(StartRunResult { message_id: ret })
}

/// Background OpenAI-compatible stream — Ollama, LM Studio, vLLM, etc.
#[tauri::command]
pub async fn start_openai_compat_stream(
    input: SendWithClaudeInput, app: AppHandle, state: State<'_, DbState>,
) -> Result<StartRunResult, AppError> {
    let db = state.inner().clone();
    let db_post = state.inner().clone();
    let is_lmstudio = input.engine.as_deref() == Some("lmstudio");
    let engine_label = if is_lmstudio { "lmstudio" } else { "ollama" };
    eprintln!("[openai-compat] engine={:?} model={:?} is_lmstudio={}", input.engine, input.model, is_lmstudio);
    let id_frag = identity_fragment(&input, engine_label);
    let write_arc = db_write_arc(&state);
    let cid = input.conversation_id.clone();
    let mo = input.model.clone();
    // Preserve custom base URL override before `input` is moved into spawn_blocking.
    let custom_base_url_override = input.custom_base_url.clone();

    let prep = tokio::task::spawn_blocking(move || {
        // Local models have limited context — cap budget to avoid exceeding model limits.
        // Default 60k is for cloud models (200k+ context). Local models are typically 4k-32k.
        let mut input = input;
        if input.context_budget_cap.is_none() {
            input.context_budget_cap = Some(8000); // ~2k tokens, safe for 4k context models
        }
        prepare_engine_run(engine_label, &input, id_frag.as_deref(), &db)
    }).await.map_err(|_| AppError::Lock)??;

    let ret = prep.msg_id.clone();
    let system_prompt = prep.system_context.clone();
    let PreparedRun { msg_id, job_id, enriched_prompt, project_path, ctx_meta, audit_session_id, .. } = prep;
    // Priority: UI override (non-empty) → env var → hardcoded default.
    // INV-1: env-var users keep working when UI override is empty.
    let base_url = custom_base_url_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if is_lmstudio {
                openai_compat::lmstudio_base_url()
            } else {
                std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into())
            }
        });

    let cid2 = cid.clone();
    let el = engine_label.to_string();
    tokio::spawn(async move {
        let pa = app.clone(); let pi = msg_id.clone(); let pc = cid2.clone();
        let c2 = app.clone(); let ci = msg_id.clone(); let cc = cid2.clone();
        let t0 = std::time::Instant::now();
        let rr = openai_compat::stream_run_with_base(
            claude::RunInput { prompt: enriched_prompt, model: mo.clone(), system_prompt, resume_token: None, project_path, image_paths: Vec::new() },
            base_url,
            move |t| { let _ = pa.emit("ollama:progress", ChunkPayload { message_id: pi.clone(), conversation_id: pc.clone(), text: t }); },
            move |t| { let _ = c2.emit("ollama:chunk", ChunkPayload { message_id: ci.clone(), conversation_id: cc.clone(), text: t }); },
        ).await;
        let dur = t0.elapsed().as_millis();
        guardrail::log_run(&el, mo.as_deref(), dur, 0, rr.is_ok());
        if let Ok(conn) = write_arc.lock() {
            finalize_engine_run(&conn, &el, &msg_id, &cid, &job_id, &rr, dur, &ctx_meta, &app, audit_session_id.as_deref());
        }
        if rr.is_ok() { spawn_post_completion_tasks(db_post, cid); }
    });
    Ok(StartRunResult { message_id: ret })
}

/// Persist a system message (tool-request result, workflow trigger) and return its ID.
/// Unlike sendWithEngine, this does NOT trigger an agent run — the caller
/// is responsible for sending the next agent message separately.
#[tauri::command]
pub fn persist_system_msg(
    conversation_id: String,
    content: String,
    state: State<DbState>,
    app: AppHandle,
) -> Result<String, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let msg_id = super::agents_helpers::send_common::persist_system_message(&conn, &conversation_id, &content)?;
    let _ = app.emit("message:new", serde_json::json!({
        "conversationId": conversation_id, "messageId": msg_id, "role": "system",
    }));
    Ok(msg_id)
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
        prompt, model, system_prompt: None, resume_token: None, project_path, image_paths: Vec::new(),
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
