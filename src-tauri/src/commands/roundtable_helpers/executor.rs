use tauri::Emitter;
use serde::{Deserialize, Serialize};

use crate::agents::{claude, codex, gemini, openai_compat, opencode};
use crate::db::{models::Message, DbState};
use crate::errors::AppError;
use crate::CancelRegistry;

use super::prompt::{build_round_prompt_with_identity, PromptSources};
use super::persist::persist_single;

/// Real-time participant execution status — emitted at actual subprocess lifecycle points.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtParticipantStatus {
    pub conversation_id: String,
    pub name: String,
    pub engine: String,
    pub model: Option<String>,
    pub round: u32,
    pub status: String, // "running" | "done" | "error"
    #[serde(default)]
    pub blind: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundtableParticipant {
    pub name: String,
    pub model: Option<String>,
    pub engine: Option<String>,
    /// Blind verifier — receives only the topic, no prior/current transcript.
    #[serde(default)]
    pub blind: bool,
    /// RT role — affects output cap and prompt directive.
    /// "proposer" | "reviewer" | "verifier" | "synthesizer" | null (default)
    #[serde(default)]
    pub role: Option<String>,
    /// Explicit output token cap. If not set, derived from role.
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

/// Build identity string for a RT participant.
fn participant_identity(p: &RoundtableParticipant) -> String {
    let engine = p.engine.as_deref().unwrap_or("claude");
    let mut lines = vec![format!("## Your Identity in this Roundtable\n\nYou are **{}** (engine: {}).", p.name, engine)];
    if let Some(role) = &p.role {
        lines.push(format!("Your role: {}.", role));
    }
    if p.blind {
        lines.push("You are a blind verifier — you have NOT seen other participants' responses. Judge independently.".into());
    }
    lines.push("Do NOT claim to be a different agent. Do NOT use other participants' names as your own.".into());
    lines.join("\n")
}

/// Get the effective output token cap for a participant based on role.
fn effective_max_tokens(p: &RoundtableParticipant) -> Option<u32> {
    if let Some(cap) = p.max_tokens {
        return Some(cap);
    }
    // Role-based defaults
    match p.role.as_deref() {
        Some("proposer") => Some(1200),
        Some("reviewer" | "critic") => Some(900),
        Some("verifier" | "judge") => Some(800),
        Some("synthesizer" | "lead") => Some(1500),
        _ => None, // no cap for unspecified roles
    }
}

/// Build output cap directive to prepend to prompt.
fn output_cap_directive(max_tokens: Option<u32>) -> String {
    match max_tokens {
        Some(cap) => format!(
            "[Output limit: Keep your response under approximately {} tokens. Be concise and focused.]\n\n",
            cap
        ),
        None => String::new(),
    }
}

pub struct ParticipantResult {
    pub name: String,
    pub engine: String,
    pub model: Option<String>,
    pub content: String,
    pub status: String,
    pub cost_usd: f64,
    pub in_tokens: i64,
    pub out_tokens: i64,
    pub prompt_sources: String,
    pub blind: bool,
}

/// Controls how participants see context within and across rounds.
#[derive(Clone, Copy)]
pub enum RoundStrategy {
    Sequential,
    Deliberative,
}

/// Run a single participant against a prompt. No DB lock held.
pub fn run_participant(
    p: &RoundtableParticipant,
    prompt: String,
    sources_json: String,
    project_path: Option<String>,
) -> ParticipantResult {
    let engine_key = p.engine.as_deref().unwrap_or("claude");
    let max_tok = effective_max_tokens(p);
    eprintln!("[rt] running participant={} engine={} role={:?} max_tokens={:?}", p.name, engine_key, p.role, max_tok);

    // Prepend output cap directive if applicable
    let prompt = format!("{}{}", output_cap_directive(max_tok), prompt);

    let run_input = claude::RunInput {
        prompt,
        model: p.model.clone(),
        system_prompt: None,
        resume_token: None,
        project_path,
    };

    // Run subprocess in background thread to prevent UI freeze
    let engine_key_owned = engine_key.to_string();
    let (run_result, engine_label) = std::thread::spawn(move || -> (Result<crate::agents::claude::RunOutput, AppError>, &'static str) {
        match engine_key_owned.as_str() {
            "claude" => (claude::run(run_input), "claude-code"),
            "codex" => (codex::run(run_input), "codex"),
            "gemini" => (gemini::run(run_input), "gemini"),
            "opencode" => (opencode::run(run_input), "opencode"),
            "ollama" => (openai_compat::run(run_input), "ollama"),
            _ => (
                Err(AppError::Agent(format!("unsupported engine: {}", engine_key_owned))),
                "unknown",
            ),
        }
    })
    .join()
    .unwrap_or_else(|_| (Err(AppError::Agent("participant thread panicked".into())), "unknown"));

    match run_result {
        Ok(out) => ParticipantResult {
            name: p.name.clone(),
            engine: engine_label.to_string(),
            model: p.model.clone(),
            content: out.content,
            status: "done".into(),
            cost_usd: out.cost_usd,
            in_tokens: out.input_tokens,
            out_tokens: out.output_tokens,
            prompt_sources: sources_json,
            blind: p.blind,
        },
        Err(e) => ParticipantResult {
            name: p.name.clone(),
            engine: engine_label.to_string(),
            model: p.model.clone(),
            content: format!("Error: {}", e),
            status: "error".into(),
            cost_usd: 0.0,
            in_tokens: 0,
            out_tokens: 0,
            prompt_sources: sources_json,
            blind: p.blind,
        },
    }
}

/// Run all participants in a single round, persisting and emitting each result.
///
/// - **Sequential**: serial execution. Each participant runs after the previous finishes.
/// - **Deliberative**: parallel execution. All participants run simultaneously.
///
/// Prompt is passed through as-is — no forced context injection.
/// Users control what context to include in their prompt per round.
pub fn execute_round(
    participants: &[RoundtableParticipant],
    transcript: &[(String, String)],
    round_num: u32,
    total_rounds: u32,
    topic: &str,
    strategy: RoundStrategy,
    rt_mode: &str,
    conversation_id: &str,
    state: &DbState,
    app: &tauri::AppHandle,
    cancel: &CancelRegistry,
    trace_id: &str,
    root_span_id: &str,
    project_path: Option<&str>,
) -> Result<(Vec<Message>, Vec<(String, String)>), AppError> {
    let prior_refs: Vec<String> = transcript.iter().map(|(n, _)| n.clone()).collect();

    match strategy {
        RoundStrategy::Sequential => execute_sequential(
            participants, transcript, &prior_refs, round_num, total_rounds, topic, rt_mode,
            conversation_id, state, app, cancel, trace_id, root_span_id, project_path,
        ),
        RoundStrategy::Deliberative => execute_parallel(
            participants, transcript, &prior_refs, round_num, total_rounds, topic, rt_mode,
            conversation_id, state, app, cancel, trace_id, root_span_id, project_path,
        ),
    }
}

/// Sequential: run participants one by one. Each sees prior-round + current-round context.
fn execute_sequential(
    participants: &[RoundtableParticipant],
    transcript: &[(String, String)],
    prior_refs: &[String],
    round_num: u32, total_rounds: u32,
    topic: &str, rt_mode: &str,
    conversation_id: &str, state: &DbState, app: &tauri::AppHandle,
    cancel: &CancelRegistry, trace_id: &str, root_span_id: &str,
    project_path: Option<&str>,
) -> Result<(Vec<Message>, Vec<(String, String)>), AppError> {
    let mut messages = Vec::new();
    let mut round_responses: Vec<(String, String)> = Vec::new();

    for p in participants {
        if cancel.check_and_consume(conversation_id) {
            return Err(AppError::Agent("cancelled by user".into()));
        }

        let sources = PromptSources {
            round: round_num, total_rounds,
            mode: rt_mode.to_string(),
            prior_round_refs: prior_refs.to_vec(),
            current_round_refs: round_responses.iter().map(|(n, _)| n.clone()).collect(),
        };
        let sources_json = serde_json::to_string(&sources).unwrap_or_default();

        let engine_key = p.engine.as_deref().unwrap_or("claude");
        let _ = app.emit("roundtable:participant_status", RtParticipantStatus {
            conversation_id: conversation_id.to_string(),
            name: p.name.clone(), engine: engine_key.to_string(), model: p.model.clone(),
            round: round_num, status: "running".into(), blind: p.blind,
        });

        // Build prompt with participant identity
        let identity = participant_identity(p);
        let prompt = if p.blind {
            eprintln!("[rt] blind verifier: {} — no transcript", p.name);
            build_round_prompt_with_identity(topic, &[], &[], Some(&identity))
        } else {
            build_round_prompt_with_identity(topic, transcript, &round_responses, Some(&identity))
        };
        let r = run_participant(p, prompt, sources_json, project_path.map(|s| s.to_string()));

        let _ = app.emit("roundtable:participant_status", RtParticipantStatus {
            conversation_id: conversation_id.to_string(),
            name: r.name.clone(), engine: r.engine.clone(), model: r.model.clone(),
            round: round_num, status: r.status.clone(), blind: r.blind,
        });

        let msg = {
            let conn = state.write.lock().map_err(|_| AppError::Lock)?;
            persist_single(&conn, conversation_id, &r, trace_id, root_span_id)?
        };
        let _ = app.emit("roundtable:progress", &msg);
        messages.push(msg);

        if r.status == "done" {
            round_responses.push((r.name.clone(), r.content.clone()));
        }
    }

    Ok((messages, round_responses))
}

/// Deliberative: run all participants in parallel, then persist results.
/// Each sees prior-round context but not current-round peers.
fn execute_parallel(
    participants: &[RoundtableParticipant],
    transcript: &[(String, String)],
    prior_refs: &[String],
    round_num: u32, total_rounds: u32,
    topic: &str, rt_mode: &str,
    conversation_id: &str, state: &DbState, app: &tauri::AppHandle,
    cancel: &CancelRegistry, trace_id: &str, root_span_id: &str,
    project_path: Option<&str>,
) -> Result<(Vec<Message>, Vec<(String, String)>), AppError> {
    if cancel.check_and_consume(conversation_id) {
        return Err(AppError::Agent("cancelled by user".into()));
    }

    // Emit "running" for all participants at once
    for p in participants {
        let engine_key = p.engine.as_deref().unwrap_or("claude");
        let _ = app.emit("roundtable:participant_status", RtParticipantStatus {
            conversation_id: conversation_id.to_string(),
            name: p.name.clone(), engine: engine_key.to_string(), model: p.model.clone(),
            round: round_num, status: "running".into(), blind: p.blind,
        });
    }

    // Build sources metadata (same for all — no current-round refs in deliberative)
    let sources = PromptSources {
        round: round_num, total_rounds,
        mode: rt_mode.to_string(),
        prior_round_refs: prior_refs.to_vec(),
        current_round_refs: Vec::new(),
    };
    let sources_json = serde_json::to_string(&sources).unwrap_or_default();

    // Spawn all participants in parallel, collect results in completion-order via channel.
    let (tx, rx) = std::sync::mpsc::channel::<ParticipantResult>();
    let participant_count = participants.len();

    // Each participant gets their own identity + prompt
    let transcript_owned: Vec<(String, String)> = transcript.to_vec();
    let topic_owned = topic.to_string();

    let _handles: Vec<_> = participants.iter().map(|p| {
        let p_clone = p.clone();
        let identity = participant_identity(p);
        let tr = transcript_owned.clone();
        let tp = topic_owned.clone();
        // Build per-participant prompt with identity
        let pr = if p.blind {
            eprintln!("[rt] blind verifier: {} — no transcript (deliberative)", p.name);
            build_round_prompt_with_identity(&tp, &[], &[], Some(&identity))
        } else {
            build_round_prompt_with_identity(&tp, &tr, &[], Some(&identity))
        };
        let sj = sources_json.clone();
        let pp = project_path.map(|s| s.to_string());
        let tx = tx.clone();
        std::thread::spawn(move || {
            let result = run_participant(&p_clone, pr, sj, pp);
            let _ = tx.send(result);
        })
    }).collect();
    drop(tx); // Close sender so rx iterator terminates after all threads finish

    // Collect results in completion-order — first to finish is first to persist/emit
    let mut messages = Vec::new();
    let mut round_responses: Vec<(String, String)> = Vec::new();
    let mut received = 0;

    for r in rx {
        received += 1;
        eprintln!("[rt] deliberative result {}/{}: {} ({})", received, participant_count, r.name, r.status);

        let _ = app.emit("roundtable:participant_status", RtParticipantStatus {
            conversation_id: conversation_id.to_string(),
            name: r.name.clone(), engine: r.engine.clone(), model: r.model.clone(),
            round: round_num, status: r.status.clone(), blind: r.blind,
        });

        let msg = {
            let conn = state.write.lock().map_err(|_| AppError::Lock)?;
            persist_single(&conn, conversation_id, &r, trace_id, root_span_id)?
        };
        let _ = app.emit("roundtable:progress", &msg);
        messages.push(msg);

        if r.status == "done" {
            round_responses.push((r.name.clone(), r.content.clone()));
        }
    }

    Ok((messages, round_responses))
}
