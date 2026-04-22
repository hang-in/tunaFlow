//! Core types and participant helpers for RT execution.

use serde::{Deserialize, Serialize};

/// Real-time participant execution status — emitted at actual subprocess lifecycle points.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RtParticipantStatus {
    pub conversation_id: String,
    pub name: String,
    pub engine: String,
    pub model: Option<String>,
    pub round: u32,
    pub status: String,
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
    #[serde(default)]
    pub role: Option<String>,
    /// Explicit output token cap. If not set, derived from role.
    #[serde(default)]
    pub max_tokens: Option<u32>,
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
    /// Session ID from the engine — used for resume_token in next round.
    pub session_id: Option<String>,
}

/// Controls how participants see context within and across rounds.
#[derive(Clone, Copy)]
pub enum RoundStrategy {
    Sequential,
    Deliberative,
}

pub type SessionMap = std::collections::HashMap<String, String>;

// ─── Participant helpers ───────────────────────────────────────────────────────

/// Build identity string for a RT participant.
pub fn participant_identity(p: &RoundtableParticipant) -> String {
    let engine = p.engine.as_deref().unwrap_or("claude");
    let mut lines = vec![format!("## Your Identity in this Roundtable\n\nYou are **{}** (engine: {}).", p.name, engine)];

    if let Some(role) = &p.role {
        let guidance = role_guidance(role);
        if guidance.is_empty() {
            lines.push(format!("Your role: {}.", role));
        } else {
            lines.push(format!("Your role: {}.\n{}", role, guidance));
        }
    }
    if p.blind {
        lines.push("You are a blind verifier — you have NOT seen other participants' responses. Judge independently.".into());
    }
    lines.push("Do NOT claim to be a different agent. Do NOT use other participants' names as your own.".into());
    lines.join("\n")
}

/// Role-specific behavioral guidance for RT participants.
fn role_guidance(role: &str) -> &'static str {
    match role {
        "proposer" => {
            "**Proposer guidelines:**\n\
             - Form your analysis independently — do not converge toward other participants' views.\n\
             - Lead with your conclusion, then provide supporting evidence.\n\
             - Flag assumptions explicitly; do not treat them as facts.\n\
             - **Emit an `## Invariants` section** listing constraints the implementation MUST NEVER violate.\n\
             - Format each invariant as: `- [INV-N] <short statement> — <why it matters>`\n\
             - Examples:\n\
               - [INV-1] Do not call db.write.lock() inside broadcast_event — same-thread re-entrant deadlock risk.\n\
               - [INV-2] Do not release streaming subscription during adopt — message loss risk.\n\
             - Prefer 0 to 7 invariants. If you cannot state any concrete invariant, write `None` and explain why.\n\
             - Invariants must be checkable by reading code or running a test — not subjective quality claims."
        }
        "reviewer" | "critic" => {
            "**Reviewer guidelines:**\n\
             - Evaluate across 4 dimensions: plan_coverage (completeness), code_quality (bugs/security), test_coverage, convention.\n\
             - Score each dimension 1–5. Include the scores in your response.\n\
             - For each finding, include: file path, line range (if applicable), defect type, severity.\n\
             - Put improvement suggestions in a separate `recommendations` section, not in `findings`.\n\
             - If verdict is `fail`, list failed subtask numbers as: `failed_subtask_ids: [N, M]`.\n\
             - **Invariants verification (required when proposer declared any):** for each INV-N in the proposer's\n\
               output, emit an entry in an `invariant_checks` array with shape:\n\
               `{ \"id\": \"INV-N\", \"status\": \"pass|fail|cannot_verify\", \"evidence\": \"<file:line or reasoning>\" }`\n\
             - If ANY invariant_check is `fail`, verdict MUST be `fail`.\n\
             - If multiple invariants are `cannot_verify`, mark verdict `fail` and request proposer to add tests or\n\
               narrow the invariant scope."
        }
        "verifier" | "judge" => {
            "**Verifier guidelines:**\n\
             - Focus on concrete evidence — do not rely on other participants' assessments.\n\
             - State your verdict first, then justify with specific references.\n\
             - Distinguish clearly between observed facts and inferences.\n\
             - If proposer declared invariants, cross-check each one independently and record the result in\n\
               `invariant_checks` — do not trust the reviewer's check."
        }
        "synthesizer" | "lead" => {
            "**Synthesizer guidelines:**\n\
             - Organize findings into three sections: `consensus`, `contested`, `dissent`.\n\
             - Preserve each reviewer's original verdict — do not overwrite it.\n\
             - Final verdict must be consistent with the vote tally across participants.\n\
             - If no clear consensus exists, state that explicitly rather than forcing agreement.\n\
             - If any participant's invariant_check status is `fail`, final verdict MUST be `fail` regardless of\n\
               dimension scores."
        }
        _ => "",
    }
}

/// Get the effective output token cap for a participant based on role.
pub fn effective_max_tokens(p: &RoundtableParticipant) -> Option<u32> {
    if let Some(cap) = p.max_tokens {
        return Some(cap);
    }
    match p.role.as_deref() {
        Some("proposer") => Some(1200),
        Some("reviewer" | "critic") => Some(900),
        Some("verifier" | "judge") => Some(800),
        Some("synthesizer" | "lead") => Some(2000),
        _ => None,
    }
}

/// Build output cap directive to prepend to prompt.
pub fn output_cap_directive(max_tokens: Option<u32>) -> String {
    match max_tokens {
        Some(cap) => format!(
            "[Output limit: Keep your response under approximately {} tokens. Be concise and focused.]\n\n",
            cap
        ),
        None => String::new(),
    }
}

// ─── Unit tests ────────────────────────────────────────────────────────────────

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
        assert!(id.contains("claude"));
    }

    #[test]
    fn identity_has_anti_impersonation_rule() {
        let p = make_participant("X", Some("gemini"), false, None);
        let id = participant_identity(&p);
        assert!(id.contains("Do NOT claim to be a different agent"));
    }

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
        assert_eq!(effective_max_tokens(&p), Some(2000));
    }

    #[test]
    fn max_tokens_lead_alias() {
        let p = make_participant("A", None, false, Some("lead"));
        assert_eq!(effective_max_tokens(&p), Some(2000));
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

    // ─── Invariants checklist (Phase 1 of harnessVerificationGapPlan) ──────────

    #[test]
    fn proposer_guidance_requires_invariants_section() {
        let p = make_participant("Proposer", Some("claude"), false, Some("proposer"));
        let id = participant_identity(&p);
        assert!(id.contains("## Invariants"), "proposer must be told to emit ## Invariants section");
        assert!(id.contains("[INV-"), "proposer must be shown the INV-N format");
    }

    #[test]
    fn proposer_guidance_mentions_bounded_invariant_count() {
        let p = make_participant("Proposer", Some("claude"), false, Some("proposer"));
        let id = participant_identity(&p);
        assert!(
            id.contains("0 to 7 invariants") || id.contains("0-7"),
            "proposer guidance must bound invariant count to avoid reviewer overload"
        );
    }

    #[test]
    fn reviewer_guidance_requires_invariant_checks() {
        let p = make_participant("Reviewer", Some("codex"), false, Some("reviewer"));
        let id = participant_identity(&p);
        assert!(id.contains("invariant_checks"), "reviewer must emit invariant_checks array");
        assert!(id.contains("pass|fail|cannot_verify"), "reviewer must know the 3 statuses");
    }

    #[test]
    fn reviewer_invariant_fail_forces_verdict_fail() {
        let p = make_participant("Reviewer", Some("codex"), false, Some("reviewer"));
        let id = participant_identity(&p);
        assert!(
            id.contains("any invariant_check") || id.contains("ANY invariant_check"),
            "reviewer guidance must state that failed invariant forces verdict=fail"
        );
    }

    #[test]
    fn critic_alias_also_has_invariant_checks() {
        let p = make_participant("Critic", Some("codex"), false, Some("critic"));
        let id = participant_identity(&p);
        assert!(id.contains("invariant_checks"), "critic (alias of reviewer) must also have invariant_checks");
    }

    #[test]
    fn verifier_guidance_crosschecks_invariants_independently() {
        let p = make_participant("Verifier", Some("gemini"), false, Some("verifier"));
        let id = participant_identity(&p);
        assert!(
            id.contains("cross-check") || id.contains("crosscheck") || id.contains("independently"),
            "verifier must independently cross-check invariants, not trust reviewer"
        );
    }

    #[test]
    fn synthesizer_guidance_honors_invariant_failures() {
        let p = make_participant("Synth", Some("claude"), false, Some("synthesizer"));
        let id = participant_identity(&p);
        assert!(
            id.contains("invariant_check"),
            "synthesizer must honor any participant's failed invariant_check"
        );
        assert!(
            id.contains("final verdict MUST be `fail`") || id.contains("final verdict MUST be fail"),
            "synthesizer must override final verdict to fail when any invariant fails"
        );
    }

    #[test]
    fn non_invariant_roles_do_not_get_invariant_noise() {
        // proposer and reviewer/critic/verifier/synthesizer all get invariant guidance.
        // Any unknown role should NOT get invariant guidance (keeps guidance narrow).
        let p = make_participant("Other", Some("claude"), false, Some("commentator"));
        let id = participant_identity(&p);
        assert!(!id.contains("invariant"), "unknown roles must not inherit invariant guidance");
    }
}
