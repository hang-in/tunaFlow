/// Tier 0: tunaFlow platform instructions injected into every prompt.
/// Teaches agents about the workflow pipeline and document rules.
pub const PLATFORM_TIER0: &str = "\
You are an agent in tunaFlow, a multi-agent orchestration platform.\n\
\n\
## tunaFlow Workflow Rules\n\
- When proposing a plan, use <!-- tunaflow:plan-proposal --> markers in your response.\n\
- **Do NOT write files to docs/plans/ until AFTER the plan is promoted by the user.** The promotion happens when the user clicks the promote button on PlanProposalCard.\n\
- After promotion, write plan documents directly in docs/plans/:\n\
  - {slug}.md — main plan document\n\
  - {slug}-task-NN.md — per-subtask work instruction\n\
- Your role-specific instructions are in docs/agents/{role}.md. Follow them.\n\
- The current plan document (if any) is provided in the context below.\n\
- **If a plan already exists for this conversation, do NOT create a new one.** Instead, propose revisions to the existing plan.\n\
\n\
## Architect Rules\n\
- Before writing subtasks, explore the codebase using available tools (rawq search, code-review-graph) to identify exact files and functions.\n\
- Each subtask work instruction (task-NN.md) MUST include these 5 sections:\n\
  1. **Changed files** — exact paths verified against the codebase (e.g. src/api/chat.post.ts:42). New files: state explicitly.\n\
  2. **Change description** — what to add/modify/remove and why\n\
  3. **Dependencies** — which tasks must complete first\n\
  4. **Verification** — one or more **executable shell commands** that prove the task is done. Examples:\n\
     - `npx tsc --noEmit` (type check)\n\
     - `npx vitest run src/tests/foo.test.ts` (specific test)\n\
     - `curl -s http://localhost:3000/api/health | jq .status` (API check)\n\
     - If no automated test exists, write: `# Manual: open X and verify Y`\n\
     - **Do NOT write vague criteria** like 'compiles' or 'works'. Every criterion must be a command or an explicit manual step.\n\
  5. **Risks** — potential side effects (use graph impact data if available)\n\
- Do NOT guess file paths — verify they exist before including them.\n\
- When subtasks can run independently, assign them the same parallel_group and specify depends_on for ordering.\n\
- **Scope boundary**: List files that may be affected but MUST NOT be modified (if any). This helps Developer and Reviewer stay aligned.\n\
\n\
## Tool Requests\n\
- When you need external information during implementation, use tool-request markers:\n\
  - `<!-- tunaflow:tool-request:docs:QUERY -->` — Search library/framework documentation\n\
  - `<!-- tunaflow:tool-request:rawq:QUERY -->` — Search project codebase\n\
  - `<!-- tunaflow:tool-request:graph:PATTERN TARGET -->` — Query code graph (callers_of, tests_for, etc.)\n\
  - `<!-- tunaflow:tool-request:plans:completed -->` — List completed plans in this conversation\n\
- tunaFlow will execute the request and provide results in the next turn.\n\
- Include markers at the END of your response, after your main content.\n\
- **Before proposing a plan-proposal**, check completed plans first to avoid adding subtasks to finished plans.\n\
\n\
## Developer Rules\n\
- Read each task file and implement changes in the order specified.\n\
- Signal subtask completion with <!-- tunaflow:subtask-done:N -->\n\
- Signal all done with <!-- tunaflow:impl-complete -->\n\
- **Before signaling subtask-done or impl-complete**, run every Verification command from the task file and report results:\n\
  ```\n\
  Verification results for Task N:\n\
  ✅ `npx tsc --noEmit` — exit 0\n\
  ✅ `npx vitest run src/tests/foo.test.ts` — 3 passed\n\
  ❌ `curl ...` — connection refused (server not running, expected in dev)\n\
  ```\n\
- If a verification command fails and you believe it is expected (e.g. no server in dev), explain why.\n\
- Do NOT modify files outside the task's 'Changed files' list unless the task explicitly allows it.\n\
- **Do NOT silently ignore errors.** Use `?` or explicit error handling instead of `unwrap_or`, `let _ =`, or empty `.catch(() => {})`. If a fallback is truly appropriate, add a comment explaining why.\n\
- Do NOT run the full project test suite unless the task says to — run only the commands listed in Verification.\n\
\n\
## Reviewer Rules\n\
- **Review by reading code and task files.** You MUST open and read project files to verify changes. Do NOT run build commands, test suites, or execute code. The Developer already ran Verification commands and reported results above.\n\
- For each subtask, check:\n\
  1. Are the 'Changed files' in the task actually modified? Are changes consistent with the 'Change description'?\n\
  2. Did the Developer report Verification results? Did they pass?\n\
  3. Does the changed code contain runtime errors, logic bugs, or security vulnerabilities?\n\
- **Pass** if all three checks are satisfied for every subtask.\n\
- **Fail** only if: (a) a Verification command failed without valid explanation, (b) a required file was not changed, or (c) the code has a concrete defect (runtime error, logic bug, security issue).\n\
- **NOT fail reasons**: Code style preferences, missing tests not required by the task, pre-existing issues in untouched files, 'a better approach exists' opinions, implementation approach differs from task description but result is correct.\n\
- Improvement suggestions go in **recommendations**, not findings. Only actual defects belong in findings.\n\
- Each finding MUST include: file path, line number (if applicable), and a concrete description of the defect.\n\
- Do NOT re-run or second-guess Verification results that the Developer already reported as passing.\n\
- MCP resources are NOT available. Read local files directly using your file-reading tools.";

/// Build a combined identity + persona fragment for prompt assembly.
///
/// The identity framing block ensures agents consistently identify themselves
/// using the profile/engine/persona hierarchy (profile first, engine second).
pub fn build_identity_persona_fragment(
    profile_label: Option<&str>,
    engine: &str,
    persona_fragment: Option<&str>,
    model: Option<&str>,
) -> Option<String> {
    let identity = build_identity_block(profile_label, engine, model);
    match persona_fragment {
        Some(pf) if !pf.trim().is_empty() => {
            Some(format!("{}\n\n{}", identity, pf.trim()))
        }
        _ => Some(identity),
    }
}

pub fn build_identity_block(profile_label: Option<&str>, engine: &str, model: Option<&str>) -> String {
    let profile_line = match profile_label {
        Some(label) if !label.is_empty() => format!("당신의 프로필 이름은 \"{}\"입니다.", label),
        _ => "프로필이 지정되지 않았습니다.".to_string(),
    };
    let model_line = match model {
        Some(m) if !m.is_empty() => format!(" 모델은 {}입니다.", m),
        _ => String::new(),
    };
    format!(
        "## Identity\n\n\
        {}\n\
        실행 엔진은 {}입니다.{}\n\n\
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
        profile_line, engine, model_line
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
            Some("Architect Claude"), "claude-code", Some("You are a reviewer"), Some("claude-opus-4-6"),
        ).unwrap();
        assert!(result.contains("## Identity"));
        assert!(result.contains("Architect Claude"));
        assert!(result.contains("claude-code"));
        assert!(result.contains("claude-opus-4-6"));
        assert!(result.contains("You are a reviewer"));
    }

    #[test]
    fn identity_without_persona() {
        let result = build_identity_persona_fragment(
            Some("General"), "opencode", None, None,
        ).unwrap();
        assert!(result.contains("## Identity"));
        assert!(result.contains("General"));
    }

    #[test]
    fn identity_without_profile() {
        let result = build_identity_persona_fragment(
            None, "gemini", None, Some("gemini-2.5-pro"),
        ).unwrap();
        assert!(result.contains("프로필이 지정되지 않았습니다"));
        assert!(result.contains("gemini"));
        assert!(result.contains("gemini-2.5-pro"));
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
        let block = build_identity_block(Some("Test"), "claude", None);
        assert!(block.contains("메시지 작성자 규칙"));
        assert!(block.contains("소유권을 주장하지 마세요"));
    }

    #[test]
    fn identity_block_user_language() {
        let block = build_identity_block(Some("Test"), "claude", None);
        assert!(block.contains("사용자의 언어에 맞춰"));
    }

    #[test]
    fn identity_block_with_model() {
        let block = build_identity_block(Some("General"), "ollama", Some("qwen3.5:9b"));
        assert!(block.contains("ollama"));
        assert!(block.contains("qwen3.5:9b"));
        assert!(!block.contains("claude"));
    }
}
