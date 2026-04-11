/// Maximum characters per prior response included in prompt context.
const MAX_ANSWER_LENGTH: usize = 4000;

/// Truncate string to `max` characters (char-boundary safe).
pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= max)
        .last()
        .unwrap_or(0);
    format!("{}...", &s[..end])
}

/// Build context-enriched prompt for a single participant.
///
/// Prepends participant identity + prior/current-round responses as reference context,
/// then appends the user's prompt.
#[allow(dead_code)]
pub fn build_round_prompt(
    topic: &str,
    transcript: &[(String, String)],
    current_round: &[(String, String)],
) -> String {
    build_round_prompt_with_identity(topic, transcript, current_round, None)
}

/// Build prompt with explicit participant identity.
pub fn build_round_prompt_with_identity(
    topic: &str,
    transcript: &[(String, String)],
    current_round: &[(String, String)],
    identity: Option<&str>,
) -> String {
    let mut sections: Vec<String> = Vec::new();

    // Participant identity — tells the agent who it is in this roundtable
    if let Some(id) = identity {
        sections.push(id.to_string());
    }

    if !transcript.is_empty() {
        let lines: Vec<String> = transcript
            .iter()
            .map(|(name, content)| {
                format!("[{}]:\n{}", name, truncate(content, MAX_ANSWER_LENGTH))
            })
            .collect();
        sections.push(format!("## Prior round responses\n\n{}", lines.join("\n\n")));
    }

    if !current_round.is_empty() {
        let lines: Vec<String> = current_round
            .iter()
            .map(|(name, content)| {
                format!("[{}]:\n{}", name, truncate(content, MAX_ANSWER_LENGTH))
            })
            .collect();
        sections.push(format!("## This round (other agents)\n\n{}", lines.join("\n\n")));
    }

    if sections.is_empty() {
        return topic.to_string();
    }

    let context_block = sections.join("\n\n---\n\n");
    format!(
        "{}\n\n---\n\n{}",
        context_block, topic
    )
}

/// Build prompt using vector-retrieved context instead of full transcript.
/// Each prior response is represented by its most relevant chunk (~800 chars)
/// rather than the full response (~4000 chars), saving ~80% tokens.
pub fn build_round_prompt_with_vector_context(
    topic: &str,
    vector_context: &[(String, String)], // (name, relevant_chunk)
    current_round: &[(String, String)],
    identity: Option<&str>,
) -> String {
    let mut sections: Vec<String> = Vec::new();

    if let Some(id) = identity {
        sections.push(id.to_string());
    }

    if !vector_context.is_empty() {
        let lines: Vec<String> = vector_context
            .iter()
            .map(|(name, chunk)| format!("[{}]:\n{}", name, chunk))
            .collect();
        sections.push(format!("## Prior discussion (relevant excerpts)\n\n{}", lines.join("\n\n")));
    }

    if !current_round.is_empty() {
        let lines: Vec<String> = current_round
            .iter()
            .map(|(name, content)| format!("[{}]:\n{}", name, truncate(content, MAX_ANSWER_LENGTH)))
            .collect();
        sections.push(format!("## This round (other agents)\n\n{}", lines.join("\n\n")));
    }

    if sections.is_empty() {
        return topic.to_string();
    }

    let context_block = sections.join("\n\n---\n\n");
    format!("{}\n\n---\n\n{}", context_block, topic)
}

/// Describes what context was included in a participant's prompt.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptSources {
    pub round: u32,
    pub total_rounds: u32,
    pub mode: String,
    pub prior_round_refs: Vec<String>,
    pub current_round_refs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── truncate ────────────────────────────────────────────────────────
    #[test]
    fn truncate_within_limit() {
        assert_eq!(truncate("short", 100), "short");
    }

    #[test]
    fn truncate_over_limit() {
        let result = truncate("hello world", 5);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn truncate_empty() {
        assert_eq!(truncate("", 10), "");
    }

    // ─── build_round_prompt ──────────────────────────────────────────────
    #[test]
    fn prompt_topic_only() {
        let result = build_round_prompt("What is Rust?", &[], &[]);
        assert_eq!(result, "What is Rust?");
    }

    #[test]
    fn prompt_with_transcript() {
        let transcript = vec![("Agent1".into(), "Answer 1".into())];
        let result = build_round_prompt("topic", &transcript, &[]);
        assert!(result.contains("Prior round responses"));
        assert!(result.contains("[Agent1]"));
        assert!(result.contains("topic"));
    }

    #[test]
    fn prompt_with_current_round() {
        let current = vec![("Agent2".into(), "Reply".into())];
        let result = build_round_prompt("topic", &[], &current);
        assert!(result.contains("This round (other agents)"));
        assert!(result.contains("[Agent2]"));
    }

    #[test]
    fn prompt_with_both() {
        let transcript = vec![("A".into(), "prev".into())];
        let current = vec![("B".into(), "cur".into())];
        let result = build_round_prompt("topic", &transcript, &current);
        assert!(result.contains("Prior round"));
        assert!(result.contains("This round"));
    }

    // ─── PromptSources serialization ─────────────────────────────────────
    #[test]
    fn prompt_sources_json() {
        let sources = PromptSources {
            round: 2,
            total_rounds: 3,
            mode: "sequential".into(),
            prior_round_refs: vec!["Agent1".into()],
            current_round_refs: vec![],
        };
        let json = serde_json::to_string(&sources).unwrap();
        assert!(json.contains("\"round\":2"));
        assert!(json.contains("\"totalRounds\":3"));
        assert!(json.contains("\"mode\":\"sequential\""));
        assert!(json.contains("\"priorRoundRefs\":[\"Agent1\"]"));
        assert!(json.contains("\"currentRoundRefs\":[]"));
    }

    // ─── Identity injection ─────────────────────────────────────────────

    #[test]
    fn identity_prepended_before_topic() {
        let identity = "## Your Identity\n\nYou are Agent-X.";
        let result = build_round_prompt_with_identity("discuss Y", &[], &[], Some(identity));
        assert!(result.starts_with("## Your Identity"));
        assert!(result.contains("discuss Y"));
        // Identity should come before topic
        let id_pos = result.find("Your Identity").unwrap();
        let topic_pos = result.find("discuss Y").unwrap();
        assert!(id_pos < topic_pos);
    }

    #[test]
    fn identity_plus_transcript_plus_current() {
        let identity = "I am the Reviewer.";
        let transcript = vec![("Architect".into(), "proposal".into())];
        let current = vec![("Developer".into(), "implementation".into())];
        let result = build_round_prompt_with_identity("review this", &transcript, &current, Some(identity));
        // All three sections present in order: identity → prior → current → topic
        let id_pos = result.find("Reviewer").unwrap();
        let prior_pos = result.find("Prior round").unwrap();
        let current_pos = result.find("This round").unwrap();
        let topic_pos = result.find("review this").unwrap();
        assert!(id_pos < prior_pos);
        assert!(prior_pos < current_pos);
        assert!(current_pos < topic_pos);
    }

    // ─── Blind participant ──────────────────────────────────────────────

    #[test]
    fn blind_participant_sees_no_transcript() {
        // Blind verifier receives topic only (no transcript, no current_round)
        let identity = "You are a blind verifier.";
        let _transcript: Vec<(String, String)> = vec![("A".into(), "response A".into())];
        let _current: Vec<(String, String)> = vec![("B".into(), "response B".into())];
        // For blind: pass empty transcript and current
        let result = build_round_prompt_with_identity("evaluate X", &[], &[], Some(identity));
        assert!(!result.contains("Prior round"));
        assert!(!result.contains("This round"));
        assert!(!result.contains("response A"));
        assert!(!result.contains("response B"));
        assert!(result.contains("evaluate X"));
        assert!(result.contains("blind verifier"));
    }

    // ─── Sequential: prior + current round semantics ────────────────────

    #[test]
    fn sequential_first_participant_no_current_round() {
        let transcript = vec![("P1".into(), "round 1".into())]; // prior round
        // First participant in round 2 → no current_round yet
        let result = build_round_prompt("topic", &transcript, &[]);
        assert!(result.contains("Prior round"));
        assert!(!result.contains("This round"));
    }

    #[test]
    fn sequential_second_participant_sees_current_round() {
        let transcript = vec![("P1".into(), "round 1".into())];
        let current = vec![("P2".into(), "round 2 first".into())];
        let result = build_round_prompt("topic", &transcript, &current);
        assert!(result.contains("Prior round"));
        assert!(result.contains("This round"));
        assert!(result.contains("round 2 first"));
    }

    // ─── Deliberative: no current-round refs ────────────────────────────

    #[test]
    fn deliberative_no_current_round() {
        let transcript = vec![("A".into(), "prev".into())];
        // In deliberative mode, current_round is always empty
        let result = build_round_prompt("topic", &transcript, &[]);
        assert!(result.contains("Prior round"));
        assert!(!result.contains("This round"));
    }

    // ─── Truncation in transcript ───────────────────────────────────────

    #[test]
    fn long_response_truncated_in_prompt() {
        let long_content = "x".repeat(5000);
        let transcript = vec![("Verbose".into(), long_content.clone())];
        let result = build_round_prompt("topic", &transcript, &[]);
        // Should not contain full 5000-char response
        assert!(result.len() < long_content.len() + 500);
        assert!(result.contains("..."));
    }

    // ─── PromptSources with current_round_refs ──────────────────────────

    #[test]
    fn prompt_sources_sequential_with_current_refs() {
        let sources = PromptSources {
            round: 1,
            total_rounds: 2,
            mode: "sequential".into(),
            prior_round_refs: vec![],
            current_round_refs: vec!["Agent1".into(), "Agent2".into()],
        };
        let json = serde_json::to_string(&sources).unwrap();
        assert!(json.contains("\"currentRoundRefs\":[\"Agent1\",\"Agent2\"]"));
    }

    #[test]
    fn prompt_sources_deliberative_empty_current() {
        let sources = PromptSources {
            round: 1,
            total_rounds: 1,
            mode: "deliberative".into(),
            prior_round_refs: vec!["A".into()],
            current_round_refs: vec![],
        };
        let json = serde_json::to_string(&sources).unwrap();
        assert!(json.contains("\"mode\":\"deliberative\""));
        assert!(json.contains("\"currentRoundRefs\":[]"));
    }
}
