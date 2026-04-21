//! Agent status, message send, and roundtable endpoints.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Deserialize;

use super::{ApiState, db_error, lock_conn};
use crate::commands::roundtable_helpers::executor::RoundtableParticipant;

pub async fn agents_status(State(state): State<ApiState>) -> impl IntoResponse {
    let conn = lock_conn(&state.db.read);
    let mut stmt = match conn.prepare(
        "SELECT id, conversation_id, engine, kind, status FROM agent_jobs WHERE status = 'running'"
    ) { Ok(s) => s, Err(e) => return db_error(e) };
    let jobs: Vec<serde_json::Value> = match stmt.query_map([], |r| Ok(serde_json::json!({
        "id": r.get::<_, String>(0)?, "conversationId": r.get::<_, String>(1)?,
        "engine": r.get::<_, Option<String>>(2)?, "kind": r.get::<_, String>(3)?,
        "status": r.get::<_, String>(4)?,
    }))) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => return db_error(e),
    };
    let running = !jobs.is_empty();
    Json(serde_json::json!({"running": running, "jobs": jobs})).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageInput {
    pub engine: Option<String>,
    pub prompt: String,
    pub model: Option<String>,
    pub dry_run: Option<bool>,
}

pub async fn send_message(
    State(state): State<ApiState>,
    Path(conv_id): Path<String>,
    Json(input): Json<SendMessageInput>,
) -> impl IntoResponse {
    let engine = input.engine.unwrap_or_else(|| "claude".into());

    // Save user message
    let user_msg_id = uuid::Uuid::new_v4().to_string();
    let now = crate::db::migrations::now_epoch_ms();
    {
        let conn = lock_conn(&state.db.write);
        if let Err(e) = conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES (?1, ?2, 'user', ?3, ?4, 'done')",
            rusqlite::params![user_msg_id, conv_id, input.prompt, now],
        ) {
            return db_error(e);
        }
    }

    // Notify WS clients of the new user message
    let _ = state.event_tx.send(serde_json::json!({
        "type": "message:new",
        "conversationId": conv_id,
        "messageId": user_msg_id,
        "role": "user",
    }).to_string());

    if input.dry_run.unwrap_or(false) {
        return (StatusCode::OK, Json(serde_json::json!({
            "messageId": user_msg_id, "dryRun": true,
            "info": "User message saved. Agent execution skipped (dry_run mode)."
        }))).into_response();
    }

    // Resolve project path once. The previous resume_token is re-read at the
    // top of every loop iteration below (it can change between iterations
    // because we persist the new session_id after each agent turn), so we
    // don't need to capture it here.
    let project_path: Option<String> = {
        let conn = lock_conn(&state.db.read);
        conn.query_row(
            "SELECT p.path FROM projects p JOIN conversations c ON c.project_key = p.key WHERE c.id = ?1",
            [&conv_id],
            |r| r.get::<_, Option<String>>(0),
        ).unwrap_or(None)
    };

    let db = state.db.clone();
    let db_post = state.db.clone();
    let conv_id_clone = conv_id.clone();
    let prompt = input.prompt.clone();
    // The desktop client runs tool-request handling in JS; the HTTP API
    // path needs to do it server-side (see tool_request.rs). We loop the
    // agent call up to MAX_TOOL_LOOP_DEPTH times, re-entering with a
    // synthesized follow-up system message whenever the response contains
    // one or more `<!-- tunaflow:tool-request:… -->` markers.
    let model = input.model.clone();
    let event_tx = state.event_tx.clone();
    let engine_outer = engine.clone();
    let conv_id_for_ctx = conv_id.clone();
    tokio::spawn(async move {
        use crate::commands::agents_helpers::tool_request::{
            execute_tool_requests, extract_tool_requests,
        };

        const MAX_TOOL_LOOP_DEPTH: u32 = 3;

        let mut current_prompt = prompt;
        let mut depth: u32 = 0;
        // Tracks the most recent assistant msg id across loop iterations so
        // the final `agent:completed` event can point at the last turn. The
        // `None` initializer is intentional: if `spawn_blocking` errors on
        // iteration 0 we return early via the `Ok(Err(_))` / `Err(_)` arms
        // and the completed event is never emitted.
        #[allow(unused_assignments)]
        let mut last_assistant_msg_id: Option<String> = None;

        loop {
            // Per-iteration clones — spawn_blocking takes ownership of these
            // values, and the next loop iteration would otherwise see them
            // moved.
            let db_iter = db.clone();
            let conv_ctx_iter = conv_id_for_ctx.clone();
            let engine_iter = engine_outer.clone();
            let model_iter = model.clone();
            let project_iter = project_path.clone();
            let prompt_iter = current_prompt.clone();

            // Re-read resume_token at the top of each iteration — the previous
            // iteration may have just written it. Doing this outside the
            // blocking task keeps the lock short.
            let prior_resume_token_iter: Option<String> = {
                let conn = match db_iter.read.lock() {
                    Ok(c) => c,
                    Err(p) => p.into_inner(),
                };
                conn.query_row(
                    "SELECT resume_token FROM conversations WHERE id = ?1",
                    [&conv_ctx_iter],
                    |r| r.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten()
            };

            let result = tokio::task::spawn_blocking(move || {
                use crate::agents::claude;
                use crate::commands::agents_helpers::send_common::build_normalized_prompt_with_budget;

                let (enriched_prompt, system_prompt, _meta) = {
                    let conn = match db_iter.read.lock() {
                        Ok(c) => c,
                        Err(p) => p.into_inner(),
                    };
                    build_normalized_prompt_with_budget(
                        &conn,
                        &conv_ctx_iter,
                        &prompt_iter,
                        project_iter.as_deref(),
                        &[],
                        &[],
                        None,
                        None,
                        None,
                        None,
                    )
                };

                eprintln!(
                    "[http-api] ContextPack built: prompt={}chars system={}chars resume={} depth={}",
                    enriched_prompt.len(),
                    system_prompt.as_ref().map(|s| s.len()).unwrap_or(0),
                    prior_resume_token_iter.as_deref().unwrap_or("<none>"),
                    depth,
                );

                let run_input = claude::RunInput {
                    prompt: enriched_prompt,
                    model: model_iter,
                    system_prompt,
                    resume_token: prior_resume_token_iter,
                    project_path: project_iter.clone(),
                    image_paths: Vec::new(),
                };
                match engine_iter.as_str() {
                    "claude" => claude::run(run_input),
                    "codex" => crate::agents::codex::run(run_input),
                    "gemini" => crate::agents::gemini::run(run_input),
                    "ollama" => crate::agents::openai_compat::run(run_input),
                    _ => claude::run(run_input),
                }
            })
            .await;

            match result {
                Ok(Ok(out)) => {
                    let assistant_msg_id = uuid::Uuid::new_v4().to_string();
                    let now = crate::db::migrations::now_epoch_ms();
                    {
                        let conn = match db_post.write.lock() {
                            Ok(c) => c,
                            Err(p) => p.into_inner(),
                        };
                        conn.execute(
                            "INSERT INTO messages (id, conversation_id, role, content, engine, model, timestamp, status) VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6, 'done')",
                            rusqlite::params![
                                assistant_msg_id,
                                conv_id_clone,
                                out.content,
                                engine_outer,
                                model,
                                now,
                            ],
                        )
                        .ok();
                        if let Some(sid) = &out.session_id {
                            conn.execute(
                                "UPDATE conversations SET resume_token = ?1 WHERE id = ?2",
                                rusqlite::params![sid, conv_id_clone],
                            )
                            .ok();
                        }
                    }
                    let _ = event_tx.send(
                        serde_json::json!({
                            "type": "message:new",
                            "conversationId": conv_id_clone,
                            "messageId": assistant_msg_id,
                            "role": "assistant",
                        })
                        .to_string(),
                    );
                    last_assistant_msg_id = Some(assistant_msg_id.clone());

                    // Did the agent end its turn with tool-request markers?
                    // If so — and we haven't busted the recursion depth —
                    // synthesize the follow-up system message and re-enter.
                    let requests = extract_tool_requests(&out.content);
                    if requests.is_empty() || depth >= MAX_TOOL_LOOP_DEPTH {
                        break;
                    }

                    let follow_up = {
                        let conn = match db_post.read.lock() {
                            Ok(c) => c,
                            Err(p) => p.into_inner(),
                        };
                        execute_tool_requests(&conn, &conv_id_clone, &requests)
                    };
                    let Some(follow_up_text) = follow_up else {
                        break;
                    };

                    let sys_msg_id = uuid::Uuid::new_v4().to_string();
                    let now2 = crate::db::migrations::now_epoch_ms();
                    {
                        let conn = match db_post.write.lock() {
                            Ok(c) => c,
                            Err(p) => p.into_inner(),
                        };
                        conn.execute(
                            "INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES (?1, ?2, 'system', ?3, ?4, 'done')",
                            rusqlite::params![sys_msg_id, conv_id_clone, follow_up_text, now2],
                        )
                        .ok();
                    }
                    let _ = event_tx.send(
                        serde_json::json!({
                            "type": "message:new",
                            "conversationId": conv_id_clone,
                            "messageId": sys_msg_id,
                            "role": "system",
                        })
                        .to_string(),
                    );

                    current_prompt = follow_up_text;
                    depth += 1;
                }
                Ok(Err(e)) => {
                    eprintln!("[http-api] agent error: {}", e);
                    let _ = event_tx.send(
                        serde_json::json!({
                            "type": "agent:error",
                            "conversationId": conv_id_clone,
                            "error": format!("{}", e),
                        })
                        .to_string(),
                    );
                    return;
                }
                Err(e) => {
                    eprintln!("[http-api] agent task panicked: {:?}", e);
                    return;
                }
            }
        }

        // Exited the loop because the last response contained no markers
        // (or we hit the depth ceiling). Emit a single agent:completed
        // pointing at the final assistant message for any WS consumers
        // waiting on that signal.
        if let Some(final_id) = last_assistant_msg_id {
            let _ = event_tx.send(
                serde_json::json!({
                    "type": "agent:completed",
                    "conversationId": conv_id_clone,
                    "messageId": final_id,
                })
                .to_string(),
            );
        }
        crate::commands::agents_helpers::send_common::spawn_post_completion_tasks(
            db_post,
            conv_id_clone,
        );
    });

    (StatusCode::ACCEPTED, Json(serde_json::json!({
        "messageId": user_msg_id, "status": "running",
        "info": "Agent execution started. Listen on /ws/events for completion."
    }))).into_response()
}

// ─── Roundtable endpoints ───────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtRunInput {
    pub conversation_id: String,
    pub prompt: String,
    pub participants: Vec<RoundtableParticipant>,
    pub mode: Option<String>,
}

pub async fn start_rt_run(
    State(state): State<ApiState>,
    Json(input): Json<RtRunInput>,
) -> impl IntoResponse {
    use crate::commands::roundtable::RoundtableRunInput;

    let rt_input = RoundtableRunInput {
        conversation_id: input.conversation_id.clone(),
        prompt: input.prompt,
        participants: input.participants,
        rounds: None,
        mode: input.mode,
        auto_synthesize: None,
    };

    let db = state.db.clone();
    let event_tx = state.event_tx.clone();
    let write_arc = std::sync::Arc::clone(&db.write);

    tokio::task::spawn_blocking(move || {
        use crate::commands::agents_helpers::context_pack::build_rt_inheritance_section;
        use crate::commands::context_queries::project_path_for_conversation;
        use crate::db::migrations::now_epoch_ms;

        let result: Result<(), String> = (|| {
            let conn = match write_arc.lock() { Ok(c) => c, Err(p) => p.into_inner() };
            let _pp = project_path_for_conversation(&conn, &rt_input.conversation_id);
            let inheritance = build_rt_inheritance_section(&conn, &rt_input.conversation_id, None);
            let enriched = if let Some(ctx) = inheritance {
                format!("{}\n\n---\n\n{}", ctx, rt_input.prompt)
            } else {
                rt_input.prompt.clone()
            };

            let names: Vec<&str> = rt_input.participants.iter().map(|p| p.name.as_str()).collect();
            let mode_label = rt_input.mode.as_deref().unwrap_or("sequential");
            let header = format!("--- Round 1 · {} · {} ---", mode_label, names.join(", "));

            let user_id = uuid::Uuid::new_v4().to_string();
            let now = now_epoch_ms();
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES (?1, ?2, 'user', ?3, ?4, 'done')",
                rusqlite::params![user_id, rt_input.conversation_id, rt_input.prompt, now],
            ).map_err(|e| e.to_string())?;

            let header_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES (?1, ?2, 'system', ?3, ?4, 'done')",
                rusqlite::params![header_id, rt_input.conversation_id, header, now_epoch_ms()],
            ).map_err(|e| e.to_string())?;

            drop(conn); // Release write lock before running agents

            for participant in &rt_input.participants {
                let engine = participant.engine.as_deref().unwrap_or("claude");
                let model = participant.model.clone();
                let name = &participant.name;

                let _ = event_tx.send(serde_json::json!({
                    "type": "roundtable:participant_status",
                    "payload": {"conversationId": rt_input.conversation_id, "name": name, "status": "running"}
                }).to_string());

                let run_result = {
                    use crate::agents::claude;
                    let run_input = claude::RunInput {
                        prompt: enriched.clone(),
                        model,
                        system_prompt: Some(format!("You are {} participating in a roundtable discussion. Be concise.", name)),
                        resume_token: None,
                        project_path: None,
                        image_paths: Vec::new(),
                    };
                    match engine {
                        "claude" => claude::run(run_input),
                        "codex" => crate::agents::codex::run(run_input),
                        "gemini" => crate::agents::gemini::run(run_input),
                        "ollama" => crate::agents::openai_compat::run(run_input),
                        _ => claude::run(run_input),
                    }
                };

                match run_result {
                    Ok(out) => {
                        let msg_id = uuid::Uuid::new_v4().to_string();
                        let conn = match write_arc.lock() { Ok(c) => c, Err(p) => p.into_inner() };
                        conn.execute(
                            "INSERT INTO messages (id, conversation_id, role, content, engine, model, persona, timestamp, status) VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6, ?7, 'done')",
                            rusqlite::params![msg_id, rt_input.conversation_id, out.content, engine, out.session_id, name, now_epoch_ms()],
                        ).ok();
                        let _ = event_tx.send(serde_json::json!({
                            "type": "roundtable:participant_status",
                            "payload": {"conversationId": rt_input.conversation_id, "name": name, "status": "done"}
                        }).to_string());
                    }
                    Err(e) => {
                        eprintln!("[http-api] RT participant {} error: {}", name, e);
                        let err_msg_id = uuid::Uuid::new_v4().to_string();
                        let conn = match write_arc.lock() { Ok(c) => c, Err(p) => p.into_inner() };
                        conn.execute(
                            "INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES (?1, ?2, 'system', ?3, ?4, 'done')",
                            rusqlite::params![err_msg_id, rt_input.conversation_id, format!("[{}] 에이전트 실패: {}", name, e), now_epoch_ms()],
                        ).ok();
                        let _ = event_tx.send(serde_json::json!({
                            "type": "agent:error",
                            "payload": {"conversationId": rt_input.conversation_id, "name": name, "error": format!("{}", e)}
                        }).to_string());
                    }
                }
            }

            let _ = event_tx.send(serde_json::json!({
                "type": "agent:completed",
                "payload": {"conversationId": rt_input.conversation_id}
            }).to_string());

            Ok(())
        })();

        if let Err(e) = result {
            eprintln!("[http-api] RT run failed: {}", e);
        }
    });

    (StatusCode::ACCEPTED, Json(serde_json::json!({
        "status": "running",
        "conversationId": input.conversation_id,
        "info": "Roundtable started. Listen on /ws/events for progress."
    }))).into_response()
}

pub async fn cancel_rt(
    State(state): State<ApiState>,
    Path(conv_id): Path<String>,
) -> impl IntoResponse {
    let mut set = state.cancel.lock();
    set.insert(conv_id.clone());
    Json(serde_json::json!({"cancelled": true, "conversationId": conv_id})).into_response()
}
