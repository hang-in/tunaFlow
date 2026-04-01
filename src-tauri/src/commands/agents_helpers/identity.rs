/// Tier 0: tunaFlow platform instructions injected into every prompt.
/// Teaches agents about the workflow pipeline and document generation rules.
pub const PLATFORM_TIER0: &str = "\
You are an agent in tunaFlow, a multi-agent orchestration platform.\n\
\n\
## tunaFlow Workflow Rules\n\
- Do NOT create files directly in docs/plans/. tunaFlow generates plan documents automatically.\n\
- When a plan is needed, propose it using <!-- tunaflow:plan-proposal --> markers in your response.\n\
- tunaFlow will parse your markers, create the plan in the database, and generate the document file.\n\
- Your role-specific instructions are in docs/agents/{role}.md. Follow them.\n\
- The current plan document (if any) is provided in the context below.\n\
- Work based on the plan document content, not by creating your own files.";

/// Build a combined identity + persona fragment for prompt assembly.
///
/// The identity framing block ensures agents consistently identify themselves
/// using the profile/engine/persona hierarchy (profile first, engine second).
pub fn build_identity_persona_fragment(
    profile_label: Option<&str>,
    engine: &str,
    persona_fragment: Option<&str>,
) -> Option<String> {
    let identity = build_identity_block(profile_label, engine);
    match persona_fragment {
        Some(pf) if !pf.trim().is_empty() => {
            Some(format!("{}\n\n{}", identity, pf.trim()))
        }
        _ => Some(identity),
    }
}

pub fn build_identity_block(profile_label: Option<&str>, engine: &str) -> String {
    let profile_line = match profile_label {
        Some(label) if !label.is_empty() => format!("당신의 프로필 이름은 \"{}\"입니다.", label),
        _ => "프로필이 지정되지 않았습니다.".to_string(),
    };
    format!(
        "## Identity\n\n\
        {}\n\
        실행 엔진은 {}입니다.\n\n\
        자기소개 규칙:\n\
        - 사용자에게 보이는 1급 이름은 프로필 이름입니다. 자기소개는 프로필 기준으로 시작하세요.\n\
        - 엔진은 필요할 때만 2순위 정보로 설명하세요.\n\
        - persona는 역할/정책 정보이며, 자기 이름처럼 답하지 마세요.\n\
        - 사용자가 다른 이름으로 부르면 짧게 정정하세요.\n\
        - 혼합 표현(예: \"Claude Code(opencode)\")을 사용하지 마세요.\n\
        - 사용자의 언어에 맞춰 응답하세요.\n\n\
        메시지 작성자 규칙:\n\
        - 대화 기록에서 각 assistant 메시지는 작성자가 표시되어 있습니다(예: [assistant:ProfileName (engine)]).\n\
        - 당신이 작성하지 않은 메시지의 소유권을 주장하지 마세요.\n\
        - 사용자가 과거 답변의 작성자를 물으면, 표시된 작성자 정보를 기준으로 답하세요.\n\
        - 작성자가 불분명한 메시지는 추측하지 말고 \"작성자 정보가 없습니다\"라고 답하세요.\n\n\
        작업 규칙:\n\
        - 구현 요청을 받으면 먼저 계획을 제시하고 사용자 승인을 기다리세요.\n\
        - 승인 없이 코드를 작성하거나 파일을 생성하지 마세요.\n\
        - 프로젝트 디렉토리 외부에 파일을 생성하지 마세요.",
        profile_line, engine
    )
}

/// Parse identity metadata from the combined persona_fragment.
/// Returns (identity_section, persona_section).
pub fn parse_identity_and_persona(fragment: Option<&str>) -> (Option<String>, Option<String>) {
    match fragment {
        Some(f) if !f.trim().is_empty() => {
            // Check if fragment starts with "## Identity" (injected by build_identity_persona_fragment)
            if f.contains("## Identity") {
                // Split at the persona boundary if exists
                if let Some(pos) = f.find("\n\n## Persona") {
                    let identity = f[..pos].trim().to_string();
                    let persona = f[pos..].trim().to_string();
                    (Some(identity), if persona.is_empty() { None } else { Some(persona) })
                } else {
                    // Identity block only, no persona
                    (Some(f.trim().to_string()), None)
                }
            } else {
                // Legacy: plain persona fragment without identity
                (None, Some(format!("## Persona\n\n{}", f.trim())))
            }
        }
        _ => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_with_profile_and_persona() {
        let result = build_identity_persona_fragment(
            Some("Architect Claude"), "claude-code", Some("You are a reviewer"),
        ).unwrap();
        assert!(result.contains("## Identity"));
        assert!(result.contains("Architect Claude"));
        assert!(result.contains("claude-code"));
        assert!(result.contains("You are a reviewer"));
    }

    #[test]
    fn identity_without_persona() {
        let result = build_identity_persona_fragment(
            Some("General"), "opencode", None,
        ).unwrap();
        assert!(result.contains("## Identity"));
        assert!(result.contains("General"));
    }

    #[test]
    fn identity_without_profile() {
        let result = build_identity_persona_fragment(
            None, "gemini", None,
        ).unwrap();
        assert!(result.contains("프로필이 지정되지 않았습니다"));
        assert!(result.contains("gemini"));
    }

    #[test]
    fn parse_identity_only() {
        let fragment = "## Identity\n\nYour profile is Test.\nEngine: claude.";
        let (id, persona) = parse_identity_and_persona(Some(fragment));
        assert!(id.is_some());
        assert!(persona.is_none());
    }

    #[test]
    fn parse_identity_and_persona_split() {
        let fragment = "## Identity\n\nProfile: Test\n\n## Persona\n\nYou are a reviewer.";
        let (id, persona) = parse_identity_and_persona(Some(fragment));
        assert!(id.unwrap().contains("Identity"));
        assert!(persona.unwrap().contains("reviewer"));
    }

    #[test]
    fn parse_legacy_persona_only() {
        let fragment = "You are a code reviewer.";
        let (id, persona) = parse_identity_and_persona(Some(fragment));
        assert!(id.is_none());
        assert!(persona.unwrap().contains("## Persona"));
    }

    #[test]
    fn parse_none_fragment() {
        let (id, persona) = parse_identity_and_persona(None);
        assert!(id.is_none());
        assert!(persona.is_none());
    }

    #[test]
    fn identity_block_has_attribution_rules() {
        let block = build_identity_block(Some("Test"), "claude");
        assert!(block.contains("메시지 작성자 규칙"));
        assert!(block.contains("소유권을 주장하지 마세요"));
    }

    #[test]
    fn identity_block_user_language() {
        let block = build_identity_block(Some("Test"), "claude");
        assert!(block.contains("사용자의 언어에 맞춰"));
    }
}
