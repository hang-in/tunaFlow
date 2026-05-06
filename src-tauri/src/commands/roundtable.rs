use rusqlite::params;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;
use std::sync::Arc;

use crate::db::{migrations::now_epoch_ms, models::Message, DbState};
use crate::errors::AppError;
use crate::CancelRegistry;
use super::agents::{StartRunResult, AgentDonePayload, AgentErrorPayload};
use super::jobs;

use super::roundtable_helpers::executor::{
    execute_round, RoundStrategy, RoundtableParticipant, SessionMap,
};
use super::roundtable_helpers::persist::{
    archive_transcript, extract_consensus_items, load_consensus, persist_header_with_round,
    save_consensus, save_shared_brief,
};
use super::agents_helpers::trace_log::{insert_trace_log, new_trace_id, new_span_id, SpanInfo};
use super::agents_helpers::context_pack::build_rt_inheritance_section;
use super::context_queries::project_path_for_conversation;

// ─── Input type ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundtableRunInput {
    pub conversation_id: String,
    pub prompt: String,
    pub participants: Vec<RoundtableParticipant>,
    /// Ignored — kept for backward compat. Each invocation runs exactly 1 round.
    #[allow(dead_code)]
    pub rounds: Option<u32>,
    pub mode: Option<String>,
    /// When true AND there are ≥2 reviewer participants, an additional
    /// synthesizer participant runs after the main round completes. Defaults to
    /// false to preserve existing behavior; callers opt in explicitly.
    /// See `docs/ideas/rtAlgorithmEnhancementIdeas.md` P1 for rationale.
    #[serde(default)]
    pub auto_synthesize: Option<bool>,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn parse_strategy(mode: &str) -> (RoundStrategy, &'static str) {
    match mode {
        "deliberative" => (RoundStrategy::Deliberative, "Deliberative"),
        _ => (RoundStrategy::Sequential, "Sequential"),
    }
}

fn participant_names(participants: &[RoundtableParticipant]) -> String {
    participants.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ")
}

/// Build a single-participant synthesizer "round" to summarize reviewers'
/// verdicts. Returns (messages, responses) so callers can append them to the
/// main round's output and persist them uniformly.
///
/// Gating:
///  - Requires ≥2 participants with role == "reviewer" | "critic" in the
///    prior round. Otherwise returns None (nothing to synthesize).
///  - Picks synthesizer engine from the first reviewer's engine for cost
///    parity. Model is left None so the engine picks its own default.
#[allow(clippy::too_many_arguments)]
async fn run_synthesizer_after_round(
    original_participants: &[RoundtableParticipant],
    round_responses: &[(String, String)],
    prior_round_refs: &[String],
    topic: &str,
    rt_mode: &str,
    round_num: u32,
    conversation_id: &str,
    state: &crate::db::DbState,
    app: &tauri::AppHandle,
    cancel: &crate::CancelRegistry,
    trace_id: &str,
    root_span_id: &str,
    project_path: Option<&str>,
    session_map: &mut SessionMap,
) -> Option<(Vec<Message>, Vec<(String, String)>)> {
    let reviewer_count = original_participants
        .iter()
        .filter(|p| matches!(p.role.as_deref(), Some("reviewer" | "critic")))
        .count();
    if reviewer_count < 2 {
        eprintln!("[rt-synth] skipped: only {} reviewer(s)", reviewer_count);
        return None;
    }

    // Inherit engine from first reviewer. Keep model=None so we don't accidentally
    // pin a reviewer's fine-tuned model as the synthesizer.
    let engine = original_participants
        .iter()
        .find(|p| matches!(p.role.as_deref(), Some("reviewer" | "critic")))
        .and_then(|p| p.engine.clone())
        .unwrap_or_else(|| "claude".into());

    let synthesizer = RoundtableParticipant {
        name: "Synthesizer".into(),
        model: None,
        engine: Some(engine),
        blind: false,
        role: Some("synthesizer".into()),
        max_tokens: None,
    };

    // Emit a header message to delimit the synthesizer output in the transcript.
    // Synthesizer 는 round_num + 1 의 결과로 취급 — single dispatch 시 ContextPack
    // 이 RT 메시지로 인지 가능 (devbug #263 Task 03).
    let header_content = format!("--- Synthesizer · {} reviewer verdicts ---", round_responses.len());
    let _ = {
        let conn = state.write.lock().ok()?;
        super::roundtable_helpers::persist::persist_header_with_round(
            &conn, conversation_id, &header_content, Some(round_num + 1),
        )
            .ok()
            .map(|h| {
                let _ = app.emit("roundtable:progress", &h);
                h
            })
    };

    // Load any prior consensus reached in earlier rounds so the synthesizer
    // doesn't try to re-litigate already-agreed axes (devbug #263 시나리오 B).
    let prior_consensus = {
        let conn = state.write.lock().ok()?;
        load_consensus(&conn, conversation_id)
    };
    let prior_consensus_text = if prior_consensus.is_empty() {
        String::new()
    } else {
        let lines: Vec<String> = prior_consensus
            .iter()
            .map(|(round, item)| {
                let decision = if item.decision.len() > 600 {
                    let safe = item
                        .decision
                        .char_indices()
                        .map(|(i, _)| i)
                        .take_while(|&i| i <= 597)
                        .last()
                        .unwrap_or(0);
                    format!("{}...", &item.decision[..safe])
                } else {
                    item.decision.clone()
                };
                format!("- **{}** (R{}): {}", item.axis, round, decision)
            })
            .collect();
        format!(
            "\n\n## Consensus reached so far\n\n\
             These axes are *already agreed* in prior rounds — do NOT re-litigate them.\n\
             Build on top of these, or address only *new* axes:\n\n{}\n",
            lines.join("\n")
        )
    };

    // Prepend a structured directive so the synthesizer knows *what* to produce.
    // The existing role_guidance (types.rs `synthesizer`) already requests
    // consensus / contested / dissent sections — we add the vote-tally instruction
    // and ask for a machine-readable consensus marker (Plan §3 Task 02).
    let directive = format!(
        "## Synthesizer Task\n\
         The {} reviewer verdicts above are the input. Do NOT overwrite them.\n\
         Produce a summary with these required sections:\n\
         1. **Vote tally** — count pass/fail/conditional verdicts.\n\
         2. **Consensus** — points where all reviewers agreed.\n\
         3. **Contested** — points where reviewers disagreed (cite which reviewer said what).\n\
         4. **Dissent** — any minority position worth preserving.\n\
         5. **Final recommendation** — one of: accept / revise / reject. Justify briefly.\n\n\
         If the tally is split (e.g. 2 pass / 1 fail), do not rubber-stamp the majority —\n\
         state the reasoning for your final recommendation.\n\n\
         ### Machine-readable consensus marker\n\n\
         At the END of your response, append the following block exactly so the\n\
         system can persist agreed axes across rounds:\n\n\
         ```\n\
         <!-- tunaflow:consensus -->\n\
         [\n  \
           {{\"axis\":\"<short keyword>\",\"decision\":\"<1-3 sentence agreed outcome>\",\"participants\":[\"<name>\",...],\"confidence\":<0.0-1.0>}}\n\
         ]\n\
         <!-- /tunaflow:consensus -->\n\
         ```\n\n\
         Output an EMPTY array `[]` if no axis reached consensus this round.\n\
         Use ONLY ASCII straight quotes inside the JSON, valid JSON syntax.{}",
        round_responses.len(),
        prior_consensus_text,
    );
    let synth_topic = format!("{}\n\n---\n\n{}", topic, directive);

    // The synthesizer is in its own "round" (round_num + 1) but sees the prior
    // round's responses as transcript. Use Sequential strategy (1 participant).
    let result = super::roundtable_helpers::executor::execute_round(
        std::slice::from_ref(&synthesizer),
        round_responses,
        round_num + 1,
        round_num + 1,
        &synth_topic,
        crate::commands::roundtable_helpers::executor::RoundStrategy::Sequential,
        rt_mode,
        conversation_id,
        state,
        app,
        cancel,
        trace_id,
        root_span_id,
        project_path,
        session_map,
    )
    .await;

    let _ = prior_round_refs; // Reserved for future deliberative-mode integration.

    match result {
        Ok((msgs, responses)) => {
            // Extract consensus items from the synthesizer's response and persist
            // them so round N+1 can inject *"already agreed"* axes into prompts
            // (devbug #263 시나리오 B 회복 path).
            //
            // INV-RTC-2 (synthesizer 본체 알고리즘 보존): 본 영역은 *추출 + 저장*
            // 만 — voting / dissent 판단은 synthesizer 의 응답 본문 그대로 보존됨.
            // Failure 는 silent (consensus persist 가 RT 본 흐름 깨면 안 됨).
            for (_, content) in responses.iter() {
                let items = extract_consensus_items(content);
                if !items.is_empty() {
                    if let Ok(conn) = state.write.lock() {
                        save_consensus(&conn, conversation_id, round_num + 1, &items);
                    }
                }
            }
            Some((msgs, responses))
        }
        Err(e) => {
            eprintln!("[rt-synth] synthesizer run failed: {e}");
            None
        }
    }
}

/// Count existing round headers to determine the next round number.
fn next_round_number(conn: &rusqlite::Connection, conversation_id: &str) -> u32 {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages
             WHERE conversation_id = ?1 AND engine = 'system' AND content LIKE '--- Round %'",
            [conversation_id],
            |row| row.get(0),
        )
        .unwrap_or(0);
    (count as u32) + 1
}

// ─── Commands ────────────────────────────────────────────────────────────────

/// Start a roundtable: always executes exactly 1 round (round 1).
#[tauri::command]
pub async fn roundtable_run(
    input: RoundtableRunInput,
    state: State<'_, DbState>,
    app: tauri::AppHandle,
    cancel: State<'_, CancelRegistry>,
) -> Result<Vec<Message>, AppError> {
    let rt_mode = input.mode.as_deref().unwrap_or("sequential");
    let (strategy, mode_label) = parse_strategy(rt_mode);
    let names = participant_names(&input.participants);

    // Insert user message + emit round header + load project path + inheritance context
    let mut all_messages: Vec<Message> = Vec::new();
    let project_path: Option<String>;
    let enriched_prompt: String;
    {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;

        // Load project path for RT participants
        project_path = project_path_for_conversation(&conn, &input.conversation_id);

        // Build RT inheritance context (anchor + recent parent turns)
        let inheritance = build_rt_inheritance_section(&conn, &input.conversation_id, None);
        enriched_prompt = if let Some(ctx) = inheritance {
            format!("{}\n\n---\n\n{}", ctx, input.prompt)
        } else {
            input.prompt.clone()
        };

        let id = Uuid::new_v4().to_string();
        let now = now_epoch_ms();
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, timestamp, status)
             VALUES (?1, ?2, 'user', ?3, ?4, 'done')",
            params![id, input.conversation_id, input.prompt, now],
        )?;

        let header = format!("--- Round 1 · {} · {} ---", mode_label, names);
        let header_msg = persist_header_with_round(&conn, &input.conversation_id, &header, Some(1))?;
        let _ = app.emit("roundtable:progress", &header_msg);
        all_messages.push(header_msg);
    }

    // Execute 1 round with OTel tracing
    let trace_id = new_trace_id();
    let root_span_id = new_span_id();
    let t0 = std::time::Instant::now();
    let mut session_map = SessionMap::new();

    let (msgs, round_responses) = execute_round(
        &input.participants,
        &[],
        1,
        1,
        &enriched_prompt,
        strategy,
        rt_mode,
        &input.conversation_id,
        &state,
        &app,
        &cancel,
        &trace_id,
        &root_span_id,
        project_path.as_deref(),
        &mut session_map,
    ).await?;
    all_messages.extend(msgs);

    // Archive + root span
    {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;
        let _ = archive_transcript(
            &conn,
            &input.conversation_id,
            &input.prompt,
            &round_responses,
            1,
            rt_mode,
        );
        save_shared_brief(&conn, &input.conversation_id, &input.prompt, &round_responses, rt_mode);
        insert_trace_log(&conn, &input.conversation_id, 0, 0, 0.0, now_epoch_ms(), &SpanInfo {
            trace_id: &trace_id,
            span_id: root_span_id,
            parent_span_id: None,
            operation: "roundtable.run",
            engine: "system",
            duration_ms: t0.elapsed().as_millis() as i64,
            status: "ok",
        });
    }

    Ok(all_messages)
}

/// Follow-up on an existing roundtable: loads prior transcript, runs 1 round
/// with the given participants (which may differ from previous rounds).
#[tauri::command]
pub async fn roundtable_followup(
    input: RoundtableRunInput,
    state: State<'_, DbState>,
    app: tauri::AppHandle,
    cancel: State<'_, CancelRegistry>,
) -> Result<Vec<Message>, AppError> {
    let rt_mode = input.mode.as_deref().unwrap_or("sequential");
    let (strategy, mode_label) = parse_strategy(rt_mode);
    let names = participant_names(&input.participants);

    // Load prior transcript + insert user message + emit round header
    let prior_transcript: Vec<(String, String)>;
    let round_num: u32;
    let mut all_messages: Vec<Message> = Vec::new();
    let project_path: Option<String>;
    {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;

        // Load project path for RT participants
        project_path = project_path_for_conversation(&conn, &input.conversation_id);

        let mut stmt = conn.prepare(
            "SELECT persona, content FROM messages
             WHERE conversation_id = ?1
               AND role = 'assistant'
               AND persona IS NOT NULL
               AND status = 'done'
             ORDER BY timestamp",
        )?;
        prior_transcript = stmt
            .query_map([&input.conversation_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let id = Uuid::new_v4().to_string();
        let now = now_epoch_ms();
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, timestamp, status)
             VALUES (?1, ?2, 'user', ?3, ?4, 'done')",
            params![id, input.conversation_id, input.prompt, now],
        )?;

        round_num = next_round_number(&conn, &input.conversation_id);
        let header = format!("--- Round {} · {} · {} ---", round_num, mode_label, names);
        let header_msg = persist_header_with_round(&conn, &input.conversation_id, &header, Some(round_num))?;
        let _ = app.emit("roundtable:progress", &header_msg);
        all_messages.push(header_msg);
    }

    // Execute 1 round with OTel tracing
    let trace_id = new_trace_id();
    let root_span_id = new_span_id();
    let t0 = std::time::Instant::now();
    let mut session_map = SessionMap::new();

    let (msgs, followup_responses) = execute_round(
        &input.participants,
        &prior_transcript,
        round_num,
        round_num,
        &input.prompt,
        strategy,
        rt_mode,
        &input.conversation_id,
        &state,
        &app,
        &cancel,
        &trace_id,
        &root_span_id,
        project_path.as_deref(),
        &mut session_map,
    ).await?;
    all_messages.extend(msgs);

    // Archive + root span
    {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;
        let _ = archive_transcript(
            &conn,
            &input.conversation_id,
            &input.prompt,
            &followup_responses,
            1,
            rt_mode,
        );
        save_shared_brief(&conn, &input.conversation_id, &input.prompt, &followup_responses, rt_mode);
        insert_trace_log(&conn, &input.conversation_id, 0, 0, 0.0, now_epoch_ms(), &SpanInfo {
            trace_id: &trace_id,
            span_id: root_span_id,
            parent_span_id: None,
            operation: "roundtable.followup",
            engine: "system",
            duration_ms: t0.elapsed().as_millis() as i64,
            status: "ok",
        });
    }

    Ok(all_messages)
}

/// Stream-abort the in-flight run for a specific conversation/thread.
///
/// 의미 (옵션 X, `branchCancelSemanticsPlan_2026-04-25.md`):
/// - 진행 중 stream 만 abort. session / SESSIONS / RESUME_IDS / process 보존.
/// - 다음 send 는 동일 session 위에서 history 보존하며 이어진다.
/// - session kill 이 필요하면 별도 `restart_sdk_session` 호출 (engine/model
///   변경 등 명시적 시나리오).
///
/// brand/main 식별: 호출자가 정확한 conv_id 를 넘겨야 한다 — brand 모드면
/// `branch:<branch_id>` shadow conv_id, main 모드면 main conv_id. 두 키는
/// CancelRegistry 안에서 분리 관리된다 (PR #198 의 SESSIONS/RESUME_IDS
/// normalize 와 다른 정책).
#[tauri::command]
pub fn cancel_running(
    conversation_id: String,
    cancel: State<CancelRegistry>,
) -> Result<(), AppError> {
    cancel.cancel(&conversation_id);
    eprintln!("[cancel] stream abort registered for {}", conversation_id);
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Background RT commands
// ═══════════════════════════════════════════════════════════════════════════

/// Background roundtable run — returns immediately, executes in background task.
#[tauri::command]
pub async fn start_roundtable_run(
    input: RoundtableRunInput,
    app: AppHandle,
    state: State<'_, DbState>,
    cancel: State<'_, CancelRegistry>,
) -> Result<StartRunResult, AppError> {
    let rt_mode = input.mode.as_deref().unwrap_or("sequential").to_string();
    let (strategy, mode_label) = parse_strategy(&rt_mode);
    let names = participant_names(&input.participants);

    // Synchronous: DB prep
    let (enriched_prompt, project_path, header_msg_id) = {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;
        let pp = project_path_for_conversation(&conn, &input.conversation_id);
        let inheritance = build_rt_inheritance_section(&conn, &input.conversation_id, None);
        let ep = if let Some(ctx) = inheritance { format!("{}\n\n---\n\n{}", ctx, input.prompt) } else { input.prompt.clone() };

        let id = Uuid::new_v4().to_string();
        let now = now_epoch_ms();
        conn.execute("INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES (?1, ?2, 'user', ?3, ?4, 'done')", params![id, input.conversation_id, input.prompt, now])?;

        let header = format!("--- Round 1 · {} · {} ---", mode_label, names);
        let header_msg = persist_header_with_round(&conn, &input.conversation_id, &header, Some(1))?;
        let _ = app.emit("roundtable:progress", &header_msg);
        (ep, pp, header_msg.id)
    };

    // Create job record
    let job_id = format!("job-{}", Uuid::new_v4());
    { let conn = state.write.lock().map_err(|_| AppError::Lock)?;
      let _ = jobs::create_job(&conn, &job_id, &input.conversation_id, Some(&header_msg_id), "system", "roundtable"); }

    let write_arc = Arc::clone(&state.write);
    let read_arc = Arc::clone(&state.read);
    let cancel_arc = Arc::clone(&cancel.0);
    let cid = input.conversation_id.clone();
    let prompt = input.prompt.clone();
    let ret = header_msg_id.clone();

    tokio::spawn(async move {
        let bg_state = DbState { write: Arc::clone(&write_arc), read: read_arc };
        let bg_cancel = CancelRegistry(cancel_arc);
        let trace_id = new_trace_id();
        let root_span_id = new_span_id();
        let t0 = std::time::Instant::now();
        let mut session_map = SessionMap::new();
        let auto_synth = input.auto_synthesize.unwrap_or(false);

        let result = execute_round(
            &input.participants, &[], 1, 1, &enriched_prompt, strategy, &rt_mode,
            &cid, &bg_state, &app, &bg_cancel, &trace_id, &root_span_id, project_path.as_deref(),
            &mut session_map,
        ).await;

        // Optionally run a synthesizer participant after the main round succeeds.
        // Scope: only when caller opted in AND the main round ran to completion.
        // Failures inside the synthesizer are logged but do not fail the whole RT.
        let (result, synth_responses): (Result<_, AppError>, Vec<(String, String)>) = match result {
            Ok((msgs, round_responses)) if auto_synth => {
                let extra = run_synthesizer_after_round(
                    &input.participants,
                    &round_responses,
                    &[],
                    &prompt,
                    &rt_mode,
                    1,
                    &cid,
                    &bg_state,
                    &app,
                    &bg_cancel,
                    &trace_id,
                    &root_span_id,
                    project_path.as_deref(),
                    &mut session_map,
                )
                .await;
                let mut all_responses = round_responses;
                let extra_responses = extra.map(|(_, r)| r).unwrap_or_default();
                all_responses.extend(extra_responses.iter().cloned());
                (Ok((msgs, all_responses)), extra_responses)
            }
            other => (other, Vec::new()),
        };
        let _ = synth_responses;

        if let Ok(conn) = write_arc.lock() {
            let now = now_epoch_ms();
            match result {
                Ok((_msgs, round_responses)) => {
                    let _ = archive_transcript(&conn, &cid, &prompt, &round_responses, 1, &rt_mode);
                    save_shared_brief(&conn, &cid, &prompt, &round_responses, &rt_mode);
                    insert_trace_log(&conn, &cid, 0, 0, 0.0, now, &SpanInfo {
                        trace_id: &trace_id, span_id: root_span_id, parent_span_id: None,
                        operation: "roundtable.run", engine: "system", duration_ms: t0.elapsed().as_millis() as i64, status: "ok",
                    });
                    let _ = jobs::complete_job(&conn, &job_id, "done", None);
                    let _ = app.emit("agent:completed", AgentDonePayload { message_id: header_msg_id, conversation_id: cid, engine: "system".into() });
                }
                Err(ref e) => {
                    let em = format!("{}", e);
                    let _ = jobs::complete_job(&conn, &job_id, "error", Some(&em));
                    let _ = app.emit("agent:error", AgentErrorPayload { message_id: header_msg_id, conversation_id: cid, engine: "system".into(), error: em });
                }
            }
        }
    });

    Ok(StartRunResult { message_id: ret })
}

/// Background roundtable followup — returns immediately.
#[tauri::command]
pub async fn start_roundtable_followup(
    input: RoundtableRunInput,
    app: AppHandle,
    state: State<'_, DbState>,
    cancel: State<'_, CancelRegistry>,
) -> Result<StartRunResult, AppError> {
    let rt_mode = input.mode.as_deref().unwrap_or("sequential").to_string();
    let (strategy, mode_label) = parse_strategy(&rt_mode);
    let names = participant_names(&input.participants);

    let (prior_transcript, round_num, project_path, header_msg_id) = {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;
        let pp = project_path_for_conversation(&conn, &input.conversation_id);

        let mut stmt = conn.prepare(
            "SELECT persona, content FROM messages WHERE conversation_id = ?1 AND role = 'assistant' AND persona IS NOT NULL AND status = 'done' ORDER BY timestamp",
        )?;
        let prior: Vec<(String, String)> = stmt.query_map([&input.conversation_id], |row| Ok((row.get(0)?, row.get(1)?)))?.filter_map(|r| r.ok()).collect();

        let id = Uuid::new_v4().to_string();
        let now = now_epoch_ms();
        conn.execute("INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES (?1, ?2, 'user', ?3, ?4, 'done')", params![id, input.conversation_id, input.prompt, now])?;

        let rn = next_round_number(&conn, &input.conversation_id);
        let header = format!("--- Round {} · {} · {} ---", rn, mode_label, names);
        let header_msg = persist_header_with_round(&conn, &input.conversation_id, &header, Some(rn))?;
        let _ = app.emit("roundtable:progress", &header_msg);
        (prior, rn, pp, header_msg.id)
    };

    let job_id = format!("job-{}", Uuid::new_v4());
    { let conn = state.write.lock().map_err(|_| AppError::Lock)?;
      let _ = jobs::create_job(&conn, &job_id, &input.conversation_id, Some(&header_msg_id), "system", "roundtable"); }

    let write_arc = Arc::clone(&state.write);
    let read_arc = Arc::clone(&state.read);
    let cancel_arc = Arc::clone(&cancel.0);
    let cid = input.conversation_id.clone();
    let prompt = input.prompt.clone();
    let ret = header_msg_id.clone();

    tokio::spawn(async move {
        let bg_state = DbState { write: Arc::clone(&write_arc), read: read_arc };
        let bg_cancel = CancelRegistry(cancel_arc);
        let trace_id = new_trace_id();
        let root_span_id = new_span_id();
        let t0 = std::time::Instant::now();
        let mut session_map = SessionMap::new();
        let auto_synth = input.auto_synthesize.unwrap_or(false);

        let result = execute_round(
            &input.participants, &prior_transcript, round_num, round_num, &prompt, strategy, &rt_mode,
            &cid, &bg_state, &app, &bg_cancel, &trace_id, &root_span_id, project_path.as_deref(),
            &mut session_map,
        ).await;

        // Optional synthesizer pass — see run_synthesizer_after_round for gating.
        let result: Result<_, AppError> = match result {
            Ok((msgs, responses)) if auto_synth => {
                let extra = run_synthesizer_after_round(
                    &input.participants,
                    &responses,
                    &[],
                    &prompt,
                    &rt_mode,
                    round_num,
                    &cid,
                    &bg_state,
                    &app,
                    &bg_cancel,
                    &trace_id,
                    &root_span_id,
                    project_path.as_deref(),
                    &mut session_map,
                )
                .await;
                let mut all_responses = responses;
                if let Some((_, r)) = extra { all_responses.extend(r); }
                Ok((msgs, all_responses))
            }
            other => other,
        };

        if let Ok(conn) = write_arc.lock() {
            let now = now_epoch_ms();
            match result {
                Ok((_msgs, responses)) => {
                    let _ = archive_transcript(&conn, &cid, &prompt, &responses, 1, &rt_mode);
                    save_shared_brief(&conn, &cid, &prompt, &responses, &rt_mode);
                    insert_trace_log(&conn, &cid, 0, 0, 0.0, now, &SpanInfo {
                        trace_id: &trace_id, span_id: root_span_id, parent_span_id: None,
                        operation: "roundtable.followup", engine: "system", duration_ms: t0.elapsed().as_millis() as i64, status: "ok",
                    });
                    let _ = jobs::complete_job(&conn, &job_id, "done", None);
                    let _ = app.emit("agent:completed", AgentDonePayload { message_id: header_msg_id, conversation_id: cid, engine: "system".into() });
                }
                Err(ref e) => {
                    let em = format!("{}", e);
                    let _ = jobs::complete_job(&conn, &job_id, "error", Some(&em));
                    let _ = app.emit("agent:error", AgentErrorPayload { message_id: header_msg_id, conversation_id: cid, engine: "system".into(), error: em });
                }
            }
        }
    });

    Ok(StartRunResult { message_id: ret })
}
