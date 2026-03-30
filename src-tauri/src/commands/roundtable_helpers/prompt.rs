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
/// Prepends prior-round and current-round responses as reference context,
/// then appends the user's prompt. No directive text is injected —
/// the user controls what to ask, agents just see the discussion context.
pub fn build_round_prompt(
    topic: &str,
    transcript: &[(String, String)],
    current_round: &[(String, String)],
) -> String {
    let mut sections: Vec<String> = Vec::new();

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
}
