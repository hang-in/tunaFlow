use tauri::Emitter;

use crate::agents::{claude, codex, gemini, opencode};
use crate::db::{models::Message, DbState};
use crate::errors::AppError;
use crate::CancelRegistry;

use super::prompt::{build_round_prompt, PromptSources};
use super::persist::persist_single;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundtableParticipant {
    pub name: String,
    pub model: Option<String>,
    pub engine: Option<String>,
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
        },
    }
}

/// Run all participants in a single round, persisting and emitting each result.
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
    let mut messages: Vec<Message> = Vec::new();
    let mut round_responses: Vec<(String, String)> = Vec::new();

    for p in participants {
        if cancel.check_and_consume(conversation_id) {
            return Err(AppError::Agent("cancelled by user".into()));
        }

        let (prior_refs, current_refs, prompt) = match strategy {
            RoundStrategy::Sequential => (
                transcript.iter().map(|(n, _)| n.clone()).collect(),
                round_responses.iter().map(|(n, _)| n.clone()).collect(),
                build_round_prompt(topic, transcript, &round_responses),
            ),
            RoundStrategy::Deliberative => (
                transcript.iter().map(|(n, _)| n.clone()).collect(),
                Vec::new(),
                build_round_prompt(topic, transcript, &[]),
            ),
        };

        let sources = PromptSources {
            round: round_num,
            total_rounds,
            mode: rt_mode.to_string(),
            prior_round_refs: prior_refs,
            current_round_refs: current_refs,
        };
        let sources_json = serde_json::to_string(&sources).unwrap_or_default();

        let r = run_participant(p, prompt, sources_json, project_path.map(|s| s.to_string()));

        let msg = {
            let conn = state.0.lock().map_err(|_| AppError::Lock)?;
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
