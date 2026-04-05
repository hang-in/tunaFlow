use tauri::Emitter;
use serde::{Deserialize, Serialize};

use crate::agents::{claude, codex, gemini, openai_compat, opencode};
use crate::db::{models::Message, DbState};
use crate::errors::AppError;
use crate::CancelRegistry;
use crate::commands::agents_helpers::send_common::build_normalized_prompt_with_budget;

use super::prompt::{build_round_prompt_with_identity, PromptSources};
use super::persist::persist_single;

/// Budget settings for local models (ollama, opencode) — smaller context window.
const LOCAL_MODE: &str = "lite";
const LOCAL_BUDGET_CAP: usize = 15_000;

/// Cached ContextPack results — built once per round, reused across participants.
/// Two variants: auto mode (commercial engines) and lite mode (local engines).
struct RtContextCache {
    auto_context: Option<String>,
    lite_context: Option<String>,
}

impl RtContextCache {
    /// Build both variants once from DB. Subsequent lookups are free.
    fn build(
        state: &DbState,
        conversation_id: &str,
        topic: &str,
        project_path: Option<&str>,
        has_local: bool,
    ) -> Self {
        let conn = match state.read.lock() {
            Ok(c) => c,
            Err(_) => return Self { auto_context: None, lite_context: None },
        };

        let auto_context = Self::extract_context(
            &conn, conversation_id, topic, project_path, None, None,
        );

        let lite_context = if has_local {
            Self::extract_context(
                &conn, conversation_id, topic, project_path,
                Some(LOCAL_MODE), Some(LOCAL_BUDGET_CAP),
            )
        } else {
            None
        };

        Self { auto_context, lite_context }
    }

    fn extract_context(
        conn: &rusqlite::Connection,
        conversation_id: &str,
        topic: &str,
        project_path: Option<&str>,
        mode: Option<&str>,
        cap: Option<usize>,
    ) -> Option<String> {
        let (enriched, _, _) = build_normalized_prompt_with_budget(
            conn, conversation_id, topic, project_path,
            &[], &[], None, mode, cap,
        );
        if let Some(pos) = enriched.rfind("\n\n---\n\n") {
            let context = enriched[..pos].trim();
            if !context.is_empty() {
                return Some(format!("## Project Context\n\n{}", context));
            }
        }
        None
    }

    /// Get cached context for a given engine.
    fn get(&self, engine_key: &str) -> Option<&str> {
        let is_local = matches!(engine_key, "ollama" | "opencode");
        let ctx = if is_local { &self.lite_context } else { &self.auto_context };
        ctx.as_deref()
    }
}

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
/// Uses `spawn_blocking` to run the synchronous subprocess without blocking the tokio runtime.
pub async fn run_participant(
    p: &RoundtableParticipant,
    prompt: String,
    sources_json: String,
    project_path: Option<String>,
) -> ParticipantResult {
    let engine_key = p.engine.as_deref().unwrap_or("claude");
    let max_tok = effective_max_tokens(p);
    eprintln!("[rt] running participant={} engine={} role={:?} max_tokens={:?}", p.name, engine_key, p.role, max_tok);

    let prompt = format!("{}{}", output_cap_directive(max_tok), prompt);

    let run_input = claude::RunInput {
        prompt,
        model: p.model.clone(),
        system_prompt: None,
        resume_token: None,
        project_path,
    };

    let engine_key_owned = engine_key.to_string();
    let result = tokio::task::spawn_blocking(move || -> (Result<crate::agents::claude::RunOutput, AppError>, &'static str) {
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
    .await
    .unwrap_or_else(|_| (Err(AppError::Agent("participant task panicked".into())), "unknown"));

    let (run_result, engine_label) = result;
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
pub async fn execute_round(
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
        ).await,
        RoundStrategy::Deliberative => execute_parallel(
            participants, transcript, &prior_refs, round_num, total_rounds, topic, rt_mode,
            conversation_id, state, app, cancel, trace_id, root_span_id, project_path,
        ).await,
    }
}

/// Sequential: run participants one by one. Each sees prior-round + current-round context.
async fn execute_sequential(
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

    // Build ContextPack cache once for all participants (auto + lite variants)
    let has_local = participants.iter().any(|p| matches!(p.engine.as_deref(), Some("ollama" | "opencode")));
    let ctx_cache = RtContextCache::build(state, conversation_id, topic, project_path, has_local);

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

        // Build prompt with participant identity + cached ContextPack
        let identity = participant_identity(p);
        let mut prompt = if p.blind {
            eprintln!("[rt] blind verifier: {} — no transcript", p.name);
            build_round_prompt_with_identity(topic, &[], &[], Some(&identity))
        } else {
            build_round_prompt_with_identity(topic, transcript, &round_responses, Some(&identity))
        };
        if let Some(ctx) = ctx_cache.get(engine_key) {
            prompt = format!("{}\n\n---\n\n{}", ctx, prompt);
        }
        let r = run_participant(p, prompt, sources_json, project_path.map(|s| s.to_string())).await;

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

/// Deliberative: run all participants in parallel via tokio tasks, then persist results.
/// Each sees prior-round context but not current-round peers.
async fn execute_parallel(
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

    // Build ContextPack cache once for all participants (auto + lite variants)
    let has_local = participants.iter().any(|p| matches!(p.engine.as_deref(), Some("ollama" | "opencode")));
    let ctx_cache = RtContextCache::build(state, conversation_id, topic, project_path, has_local);

    // Spawn all participants as tokio tasks, collect results in completion-order via channel.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ParticipantResult>(participants.len());
    let participant_count = participants.len();

    let transcript_owned: Vec<(String, String)> = transcript.to_vec();
    let topic_owned = topic.to_string();

    for p in participants {
        let p_clone = p.clone();
        let identity = participant_identity(p);
        let engine_key = p.engine.as_deref().unwrap_or("claude");
        let tr = transcript_owned.clone();
        let tp = topic_owned.clone();
        let mut pr = if p.blind {
            eprintln!("[rt] blind verifier: {} — no transcript (deliberative)", p.name);
            build_round_prompt_with_identity(&tp, &[], &[], Some(&identity))
        } else {
            build_round_prompt_with_identity(&tp, &tr, &[], Some(&identity))
        };
        if let Some(ctx) = ctx_cache.get(engine_key) {
            pr = format!("{}\n\n---\n\n{}", ctx, pr);
        }
        let sj = sources_json.clone();
        let pp = project_path.map(|s| s.to_string());
        let tx = tx.clone();
        tokio::spawn(async move {
            let result = run_participant(&p_clone, pr, sj, pp).await;
            let _ = tx.send(result).await;
        });
    }
    drop(tx); // Close sender so rx terminates after all tasks finish

    // Collect results in completion-order
    let mut messages = Vec::new();
    let mut round_responses: Vec<(String, String)> = Vec::new();
    let mut received = 0;

    while let Some(r) = rx.recv().await {
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

// ─── Unit tests for pure helper functions ────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_participant(name: &str, engine: Option<&str>, blind: bool, role: Option<&str>) -> RoundtableParticipant {
        RoundtableParticipant {
            name: name.into(),
            model: None,
            engine: engine.map(|s| s.into()),
            blind,
            role: role.map(|s| s.into()),
            max_tokens: None,
        }
    }

    // ─── participant_identity ────────────────────────────────────────────

    #[test]
    fn identity_basic() {
        let p = make_participant("Alice", Some("claude"), false, None);
        let id = participant_identity(&p);
        assert!(id.contains("Alice"));
        assert!(id.contains("claude"));
        assert!(!id.contains("blind verifier"));
    }

    #[test]
    fn identity_blind_verifier() {
        let p = make_participant("Bob", Some("gemini"), true, Some("verifier"));
        let id = participant_identity(&p);
        assert!(id.contains("Bob"));
        assert!(id.contains("blind verifier"));
        assert!(id.contains("verifier"));
    }

    #[test]
    fn identity_with_role() {
        let p = make_participant("Charlie", Some("codex"), false, Some("proposer"));
        let id = participant_identity(&p);
        assert!(id.contains("proposer"));
    }

    #[test]
    fn identity_default_engine() {
        let p = make_participant("Default", None, false, None);
        let id = participant_identity(&p);
        assert!(id.contains("claude")); // defaults to claude
    }

    #[test]
    fn identity_has_anti_impersonation_rule() {
        let p = make_participant("X", Some("gemini"), false, None);
        let id = participant_identity(&p);
        assert!(id.contains("Do NOT claim to be a different agent"));
    }

    // ─── effective_max_tokens ────────────────────────────────────────────

    #[test]
    fn max_tokens_explicit_override() {
        let mut p = make_participant("A", None, false, Some("proposer"));
        p.max_tokens = Some(2000);
        assert_eq!(effective_max_tokens(&p), Some(2000));
    }

    #[test]
    fn max_tokens_proposer_default() {
        let p = make_participant("A", None, false, Some("proposer"));
        assert_eq!(effective_max_tokens(&p), Some(1200));
    }

    #[test]
    fn max_tokens_reviewer_default() {
        let p = make_participant("A", None, false, Some("reviewer"));
        assert_eq!(effective_max_tokens(&p), Some(900));
    }

    #[test]
    fn max_tokens_critic_alias() {
        let p = make_participant("A", None, false, Some("critic"));
        assert_eq!(effective_max_tokens(&p), Some(900));
    }

    #[test]
    fn max_tokens_verifier_default() {
        let p = make_participant("A", None, false, Some("verifier"));
        assert_eq!(effective_max_tokens(&p), Some(800));
    }

    #[test]
    fn max_tokens_synthesizer_default() {
        let p = make_participant("A", None, false, Some("synthesizer"));
        assert_eq!(effective_max_tokens(&p), Some(1500));
    }

    #[test]
    fn max_tokens_lead_alias() {
        let p = make_participant("A", None, false, Some("lead"));
        assert_eq!(effective_max_tokens(&p), Some(1500));
    }

    #[test]
    fn max_tokens_no_role_none() {
        let p = make_participant("A", None, false, None);
        assert_eq!(effective_max_tokens(&p), None);
    }

    #[test]
    fn max_tokens_unknown_role_none() {
        let p = make_participant("A", None, false, Some("custom-role"));
        assert_eq!(effective_max_tokens(&p), None);
    }

    // ─── output_cap_directive ────────────────────────────────────────────

    #[test]
    fn cap_directive_with_cap() {
        let d = output_cap_directive(Some(800));
        assert!(d.contains("800 tokens"));
        assert!(d.contains("Output limit"));
    }

    #[test]
    fn cap_directive_without_cap() {
        let d = output_cap_directive(None);
        assert!(d.is_empty());
    }

    // ─── RtContextCache::get routing ─────────────────────────────────────

    #[test]
    fn context_cache_routing_local_vs_commercial() {
        let cache = RtContextCache {
            auto_context: Some("auto ctx".into()),
            lite_context: Some("lite ctx".into()),
        };
        assert_eq!(cache.get("claude"), Some("auto ctx"));
        assert_eq!(cache.get("gemini"), Some("auto ctx"));
        assert_eq!(cache.get("codex"), Some("auto ctx"));
        assert_eq!(cache.get("ollama"), Some("lite ctx"));
        assert_eq!(cache.get("opencode"), Some("lite ctx"));
    }

    #[test]
    fn context_cache_none_for_missing() {
        let cache = RtContextCache {
            auto_context: None,
            lite_context: None,
        };
        assert_eq!(cache.get("claude"), None);
        assert_eq!(cache.get("ollama"), None);
    }
}
