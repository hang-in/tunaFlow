use tauri::Emitter;
use serde::{Deserialize, Serialize};

use crate::agents::{claude, codex, gemini, openai_compat, opencode};
use crate::db::{models::Message, DbState};
use crate::errors::AppError;
use crate::CancelRegistry;

use super::prompt::{build_round_prompt_with_identity, build_round_prompt_with_vector_context, PromptSources};
use super::persist::{persist_streaming_start, persist_streaming_done};

/// Budget settings for local models (ollama, opencode) — smaller context window.
#[allow(dead_code)]
const LOCAL_MODE: &str = "lite";
#[allow(dead_code)]
const LOCAL_BUDGET_CAP: usize = 15_000;

/// Lightweight RT context — Tier 0+1 only.
/// Instead of running the full ContextPack pipeline (identity, skills, rawq,
/// memory, cross-session, retrieval — ~15k chars), we load only what RT
/// participants actually need: project path + active plan.
/// This reduces per-participant context from ~5-7k tokens to ~1-2k tokens.
struct RtContextCache {
    context: Option<String>,
}

impl RtContextCache {
    /// Build minimal Tier 0+1 context once per round.
    fn build(
        state: &DbState,
        conversation_id: &str,
        _topic: &str,
        project_path: Option<&str>,
        _has_local: bool,
    ) -> Self {
        let conn = match state.read.lock() {
            Ok(c) => c,
            Err(_) => return Self { context: None },
        };

        let mut sections: Vec<String> = Vec::new();

        // Tier 0: Project path
        if let Some(p) = project_path {
            sections.push(format!("Project: {}", p));
        }

        // Tier 1: Active plan (if exists) — the "source of truth" for current work
        let plan_conv_id = Self::resolve_plan_conv_id(&conn, conversation_id);
        if let Some(plan) = Self::load_plan_summary(&conn, &plan_conv_id) {
            sections.push(plan);
        }

        // Tier 1: Review findings (if phase=review)
        if let Some(findings) = Self::load_review_findings(&conn, &plan_conv_id) {
            sections.push(findings);
        }

        if sections.is_empty() {
            Self { context: None }
        } else {
            Self { context: Some(format!("## Project Context\n\n{}", sections.join("\n\n"))) }
        }
    }

    /// Resolve plan conversation ID (handles branch shadow conversations).
    fn resolve_plan_conv_id(conn: &rusqlite::Connection, conversation_id: &str) -> String {
        if conversation_id.starts_with("branch:") {
            // Find parent conversation for branch
            conn.query_row(
                "SELECT parent_id FROM conversations WHERE id = ?1",
                [conversation_id], |row| row.get::<_, Option<String>>(0),
            ).ok().flatten().unwrap_or_else(|| conversation_id.to_string())
        } else {
            conversation_id.to_string()
        }
    }

    /// Load compact plan summary (title + phase + subtask status).
    fn load_plan_summary(conn: &rusqlite::Connection, conversation_id: &str) -> Option<String> {
        let (title, phase): (String, String) = conn.query_row(
            "SELECT title, phase FROM plans
             WHERE conversation_id = ?1 AND status = 'active'
             ORDER BY updated_at DESC LIMIT 1",
            [conversation_id], |row| Ok((row.get(0)?, row.get(1)?)),
        ).ok()?;

        let plan_id: String = conn.query_row(
            "SELECT id FROM plans WHERE conversation_id = ?1 AND status = 'active' ORDER BY updated_at DESC LIMIT 1",
            [conversation_id], |row| row.get(0),
        ).ok()?;

        let mut out = format!("### Active Plan (phase: {})\n{}", phase, title);

        // Compact subtask list
        if let Ok(mut stmt) = conn.prepare(
            "SELECT title, status FROM plan_subtasks WHERE plan_id = ?1 ORDER BY idx"
        ) {
            let subtasks: Vec<(String, String)> = stmt.query_map([&plan_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            }).ok()?.filter_map(|r| r.ok()).collect();

            if !subtasks.is_empty() {
                out.push('\n');
                for (st_title, st_status) in &subtasks {
                    let icon = match st_status.as_str() { "done" => "✅", "in_progress" => "🔧", _ => "⬜" };
                    out.push_str(&format!("{} {}\n", icon, st_title));
                }
            }
        }

        Some(out)
    }

    /// Load review findings (only if plan is in review phase).
    fn load_review_findings(conn: &rusqlite::Connection, conversation_id: &str) -> Option<String> {
        let phase: String = conn.query_row(
            "SELECT phase FROM plans WHERE conversation_id = ?1 AND status = 'active' ORDER BY updated_at DESC LIMIT 1",
            [conversation_id], |row| row.get(0),
        ).ok()?;

        if phase != "review" && phase != "review_conditional" {
            return None;
        }

        // Load latest review findings from failure_lessons
        let mut stmt = conn.prepare(
            "SELECT finding FROM failure_lessons
             WHERE project_key = (SELECT project_key FROM conversations WHERE id = ?1)
             AND resolution IS NULL
             ORDER BY created_at DESC LIMIT 5"
        ).ok()?;

        let findings: Vec<String> = stmt.query_map([conversation_id], |row| row.get(0))
            .ok()?.filter_map(|r| r.ok()).collect();

        if findings.is_empty() { return None; }

        let mut out = String::from("### Open Review Findings\n");
        for f in &findings {
            out.push_str(&format!("- {}\n", f));
        }
        Some(out)
    }

    /// Get cached context (same for all engines — minimal is always small enough).
    fn get(&self, _engine_key: &str) -> Option<&str> {
        self.context.as_deref()
    }
}

/// In-memory vector index for RT transcript sharing.
/// Instead of copying full responses (~4000 chars each) to every participant,
/// embeds responses and retrieves only relevant chunks via cosine similarity.
/// Saves ~80% tokens in multi-round RT discussions.
struct RtVectorIndex {
    entries: Vec<RtVectorEntry>,
}

struct RtVectorEntry {
    name: String,
    text: String,       // truncated text (~800 chars)
    embedding: Vec<f32>,
}

impl RtVectorIndex {
    fn new() -> Self { Self { entries: Vec::new() } }

    /// Add a participant's response to the index. Embeds via rawq daemon.
    fn add(&mut self, name: &str, content: &str) {
        use crate::agents::rawq;
        let text = super::prompt::truncate(content, 800);
        match rawq::embed_text(&text, false) {
            Ok(emb) => {
                self.entries.push(RtVectorEntry {
                    name: name.to_string(), text, embedding: emb,
                });
            }
            Err(e) => eprintln!("[rt-vec] embed failed for {}: {:?}", name, e),
        }
    }

    /// Search for top-K most relevant chunks given a topic query.
    /// Returns (name, relevant_text) pairs.
    fn search(&self, topic: &str, limit: usize) -> Vec<(String, String)> {
        use crate::agents::rawq;
        let query_emb = match rawq::embed_text(topic, true) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let mut scored: Vec<(f32, &RtVectorEntry)> = self.entries.iter()
            .map(|e| (rawq::cosine_similarity(&query_emb, &e.embedding), e))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);

        scored.into_iter()
            .filter(|(score, _)| *score > 0.2) // minimum relevance threshold
            .map(|(_, e)| (e.name.clone(), e.text.clone()))
            .collect()
    }

    fn is_empty(&self) -> bool { self.entries.is_empty() }
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

/// Payload for real-time streaming chunks during RT participant execution.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtChunkPayload {
    pub message_id: String,
    pub conversation_id: String,
    pub text: String,
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
/// Retained for non-streaming fallback (opencode).
#[allow(dead_code)]
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

/// Run a single participant with real-time streaming. Emits `roundtable:chunk` events
/// as text arrives. Falls back to `run()` for engines without `stream_run()` (opencode).
pub async fn stream_participant(
    p: &RoundtableParticipant,
    prompt: String,
    sources_json: String,
    project_path: Option<String>,
    msg_id: String,
    conversation_id: String,
    app: tauri::AppHandle,
    cancel_arc: std::sync::Arc<parking_lot::Mutex<std::collections::HashSet<String>>>,
) -> ParticipantResult {
    let engine_key = p.engine.as_deref().unwrap_or("claude");
    let max_tok = effective_max_tokens(p);
    eprintln!("[rt-stream] running participant={} engine={} role={:?}", p.name, engine_key, p.role);

    let prompt = format!("{}{}", output_cap_directive(max_tok), prompt);
    let run_input = claude::RunInput {
        prompt,
        model: p.model.clone(),
        system_prompt: None,
        resume_token: None,
        project_path,
    };

    let name = p.name.clone();
    let model = p.model.clone();
    let blind = p.blind;
    let engine_key_owned = engine_key.to_string();

    let result: (Result<claude::RunOutput, AppError>, &'static str) = match engine_key {
        "claude" | "gemini" => {
            // Sync CLI engines with cancel support
            let a = app.clone(); let mi = msg_id.clone(); let ci = conversation_id.clone();
            let ca = std::sync::Arc::clone(&cancel_arc);
            let ci2 = conversation_id.clone();
            let is_claude = engine_key == "claude";
            tokio::task::spawn_blocking(move || {
                let on_chunk = {
                    let a = a.clone(); let mi = mi.clone(); let ci = ci.clone();
                    move |text: String| {
                        let _ = a.emit("roundtable:chunk", RtChunkPayload {
                            message_id: mi.clone(), conversation_id: ci.clone(), text,
                        });
                    }
                };
                let on_progress = |_: String| {};
                let is_cancelled = move || ca.lock().contains(&ci2);
                if is_claude {
                    (claude::stream_run(run_input, on_progress, on_chunk, is_cancelled), "claude-code")
                } else {
                    (gemini::stream_run(run_input, on_progress, on_chunk, is_cancelled), "gemini")
                }
            })
            .await
            .unwrap_or_else(|_| (Err(AppError::Agent("participant task panicked".into())), "unknown"))
        }
        "codex" => {
            // Sync CLI engine without cancel
            let a = app.clone(); let mi = msg_id.clone(); let ci = conversation_id.clone();
            tokio::task::spawn_blocking(move || {
                let on_chunk = {
                    let a = a.clone(); let mi = mi.clone(); let ci = ci.clone();
                    move |text: &str| {
                        let _ = a.emit("roundtable:chunk", RtChunkPayload {
                            message_id: mi.clone(), conversation_id: ci.clone(), text: text.to_string(),
                        });
                    }
                };
                let on_progress = |_: &str| {};
                (codex::stream_run(run_input, on_progress, on_chunk), "codex")
            })
            .await
            .unwrap_or_else(|_| (Err(AppError::Agent("participant task panicked".into())), "unknown"))
        }
        "ollama" => {
            // Async HTTP engine
            let a = app.clone(); let mi = msg_id.clone(); let ci = conversation_id.clone();
            let on_chunk = {
                let a = a.clone(); let mi = mi.clone(); let ci = ci.clone();
                move |text: String| {
                    let _ = a.emit("roundtable:chunk", RtChunkPayload {
                        message_id: mi.clone(), conversation_id: ci.clone(), text,
                    });
                }
            };
            let on_progress = |_: String| {};
            (openai_compat::stream_run(run_input, on_progress, on_chunk).await, "ollama")
        }
        "opencode" => {
            // No stream_run — fallback to sync run (no chunk events)
            tokio::task::spawn_blocking(move || {
                (opencode::run(run_input), "opencode")
            })
            .await
            .unwrap_or_else(|_| (Err(AppError::Agent("participant task panicked".into())), "unknown"))
        }
        _ => {
            (Err(AppError::Agent(format!("unsupported engine: {}", engine_key_owned))), "unknown")
        }
    };

    let (run_result, engine_label) = result;
    match run_result {
        Ok(out) => ParticipantResult {
            name, engine: engine_label.to_string(), model, content: out.content,
            status: "done".into(), cost_usd: out.cost_usd,
            in_tokens: out.input_tokens, out_tokens: out.output_tokens,
            prompt_sources: sources_json, blind,
        },
        Err(e) => ParticipantResult {
            name, engine: engine_label.to_string(), model, content: format!("Error: {}", e),
            status: "error".into(), cost_usd: 0.0, in_tokens: 0, out_tokens: 0,
            prompt_sources: sources_json, blind,
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

    // Vector index: embed prior transcript for selective retrieval (Tier 2 optimization)
    let mut vec_index = RtVectorIndex::new();
    if crate::agents::rawq::is_daemon_ready() {
        for (name, content) in transcript {
            vec_index.add(name, content);
        }
        if !vec_index.is_empty() {
            eprintln!("[rt] vector index built: {} entries from prior transcript", vec_index.entries.len());
        }
    }

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
        let engine_label = match engine_key {
            "claude" => "claude-code",
            "ollama" => "ollama",
            other => other,
        };

        // Phase 1: persist streaming placeholder + emit to frontend
        let streaming_msg = {
            let conn = state.write.lock().map_err(|_| AppError::Lock)?;
            persist_streaming_start(&conn, conversation_id, &p.name, engine_label, p.model.as_deref(), &sources_json)?
        };
        let msg_id = streaming_msg.id.clone();
        let _ = app.emit("roundtable:progress", &streaming_msg);

        let _ = app.emit("roundtable:participant_status", RtParticipantStatus {
            conversation_id: conversation_id.to_string(),
            name: p.name.clone(), engine: engine_key.to_string(), model: p.model.clone(),
            round: round_num, status: "running".into(), blind: p.blind,
        });

        // Phase 2: stream participant (emits roundtable:chunk events during execution)
        // Use vector context if available (Tier 2 optimization: ~80% token savings)
        let identity = participant_identity(p);
        let mut prompt = if p.blind {
            eprintln!("[rt] blind verifier: {} — no transcript", p.name);
            build_round_prompt_with_identity(topic, &[], &[], Some(&identity))
        } else if !vec_index.is_empty() {
            let vec_ctx = vec_index.search(topic, 5);
            eprintln!("[rt] {} using vector context: {} chunks (vs {} full transcript)", p.name, vec_ctx.len(), transcript.len());
            build_round_prompt_with_vector_context(topic, &vec_ctx, &round_responses, Some(&identity))
        } else {
            build_round_prompt_with_identity(topic, transcript, &round_responses, Some(&identity))
        };
        if let Some(ctx) = ctx_cache.get(engine_key) {
            prompt = format!("{}\n\n---\n\n{}", ctx, prompt);
        }
        let r = stream_participant(
            p, prompt, sources_json, project_path.map(|s| s.to_string()),
            msg_id.clone(), conversation_id.to_string(), app.clone(), std::sync::Arc::clone(&cancel.0),
        ).await;

        // Phase 3: finalize in DB + emit final message
        let _ = app.emit("roundtable:participant_status", RtParticipantStatus {
            conversation_id: conversation_id.to_string(),
            name: r.name.clone(), engine: r.engine.clone(), model: r.model.clone(),
            round: round_num, status: r.status.clone(), blind: r.blind,
        });

        let final_msg = {
            let conn = state.write.lock().map_err(|_| AppError::Lock)?;
            persist_streaming_done(&conn, conversation_id, &msg_id, &r, trace_id, root_span_id)?
        };
        let _ = app.emit("roundtable:progress", &final_msg);
        messages.push(final_msg);

        if r.status == "done" {
            // Add to vector index for next participant (sequential: each sees prior responses)
            if crate::agents::rawq::is_daemon_ready() {
                vec_index.add(&r.name, &r.content);
            }
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

    // Pre-create streaming placeholders for all participants (single DB lock)
    let mut msg_ids: Vec<String> = Vec::with_capacity(participants.len());
    {
        let conn = state.write.lock().map_err(|_| AppError::Lock)?;
        for p in participants {
            let engine_key = p.engine.as_deref().unwrap_or("claude");
            let engine_label = match engine_key {
                "claude" => "claude-code",
                "ollama" => "ollama",
                other => other,
            };
            let streaming_msg = persist_streaming_start(
                &conn, conversation_id, &p.name, engine_label, p.model.as_deref(), &sources_json,
            )?;
            msg_ids.push(streaming_msg.id.clone());
            let _ = app.emit("roundtable:progress", &streaming_msg);
        }
    }

    // Spawn all participants as tokio tasks, collect results in completion-order via channel.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, ParticipantResult)>(participants.len());
    let participant_count = participants.len();

    let transcript_owned: Vec<(String, String)> = transcript.to_vec();
    let topic_owned = topic.to_string();

    // Vector index for prior transcript (shared across all parallel participants)
    let mut vec_index = RtVectorIndex::new();
    if crate::agents::rawq::is_daemon_ready() {
        for (name, content) in transcript {
            vec_index.add(name, content);
        }
        if !vec_index.is_empty() {
            eprintln!("[rt-parallel] vector index built: {} entries", vec_index.entries.len());
        }
    }

    // Pre-compute vector context for all participants (same for deliberative — no current-round)
    let vec_ctx: Vec<(String, String)> = if !vec_index.is_empty() {
        vec_index.search(&topic_owned, 5)
    } else {
        Vec::new()
    };

    for (i, p) in participants.iter().enumerate() {
        let p_clone = p.clone();
        let identity = participant_identity(p);
        let engine_key = p.engine.as_deref().unwrap_or("claude");
        let tr = transcript_owned.clone();
        let tp = topic_owned.clone();
        let vc = vec_ctx.clone();
        let mut pr = if p.blind {
            eprintln!("[rt] blind verifier: {} — no transcript (deliberative)", p.name);
            build_round_prompt_with_identity(&tp, &[], &[], Some(&identity))
        } else if !vc.is_empty() {
            eprintln!("[rt-parallel] {} using vector context: {} chunks", p.name, vc.len());
            build_round_prompt_with_vector_context(&tp, &vc, &[], Some(&identity))
        } else {
            build_round_prompt_with_identity(&tp, &tr, &[], Some(&identity))
        };
        if let Some(ctx) = ctx_cache.get(engine_key) {
            pr = format!("{}\n\n---\n\n{}", ctx, pr);
        }
        let sj = sources_json.clone();
        let pp = project_path.map(|s| s.to_string());
        let tx = tx.clone();
        let mid = msg_ids[i].clone();
        let cid = conversation_id.to_string();
        let a = app.clone();
        let ca = std::sync::Arc::clone(&cancel.0);
        tokio::spawn(async move {
            let result = stream_participant(&p_clone, pr, sj, pp, mid.clone(), cid, a, ca).await;
            let _ = tx.send((mid, result)).await;
        });
    }
    drop(tx); // Close sender so rx terminates after all tasks finish

    // Collect results in completion-order
    let mut messages = Vec::new();
    let mut round_responses: Vec<(String, String)> = Vec::new();
    let mut received = 0;

    while let Some((mid, r)) = rx.recv().await {
        received += 1;
        eprintln!("[rt] deliberative result {}/{}: {} ({})", received, participant_count, r.name, r.status);

        let _ = app.emit("roundtable:participant_status", RtParticipantStatus {
            conversation_id: conversation_id.to_string(),
            name: r.name.clone(), engine: r.engine.clone(), model: r.model.clone(),
            round: round_num, status: r.status.clone(), blind: r.blind,
        });

        let final_msg = {
            let conn = state.write.lock().map_err(|_| AppError::Lock)?;
            persist_streaming_done(&conn, conversation_id, &mid, &r, trace_id, root_span_id)?
        };
        let _ = app.emit("roundtable:progress", &final_msg);
        messages.push(final_msg);

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

    // ─── RtContextCache (Tier 0+1 minimal) ─────────────────────────────

    #[test]
    fn context_cache_returns_same_for_all_engines() {
        let cache = RtContextCache {
            context: Some("plan ctx".into()),
        };
        // Tier 0+1 minimal: same context for all engines (no auto/lite split)
        assert_eq!(cache.get("claude"), Some("plan ctx"));
        assert_eq!(cache.get("gemini"), Some("plan ctx"));
        assert_eq!(cache.get("codex"), Some("plan ctx"));
        assert_eq!(cache.get("ollama"), Some("plan ctx"));
        assert_eq!(cache.get("opencode"), Some("plan ctx"));
    }

    #[test]
    fn context_cache_none_for_missing() {
        let cache = RtContextCache {
            context: None,
        };
        assert_eq!(cache.get("claude"), None);
        assert_eq!(cache.get("ollama"), None);
    }
}
