use rusqlite::Connection;

use super::context_loading::{ContextData, UserIntentMatch, load_context_data};
use super::super::trace_log::ContextPackMeta;
use super::super::identity::{parse_identity_and_persona, PLATFORM_TIER0};
use super::super::context_pack::ContextMode;

/// userIntentSsotSurfacingPlan §Layer 1: ContextPack 의 [USER_INTENT_LOOKUP]
/// 섹션을 직렬화한다. 매칭 0건이어도 빈 섹션을 출력 (INV-1 — architect 진입 시
/// 항상 surface). 각 항목은 `(YYYY-MM-DD) excerpt` 형태로 ~200 char cap.
pub(crate) fn build_user_intent_lookup_section(intents: &[UserIntentMatch]) -> String {
    let mut s = String::from(
        "[USER_INTENT_LOOKUP]\n\
         사용자가 과거 대화에서 명시한 의도 중 현재 작업과 관련된 메시지입니다.\n\
         코드/문서가 의도와 어긋난다고 판단되면 사용자에게 즉시 보고하세요.\n",
    );
    if intents.is_empty() {
        s.push_str("- (관련 사용자 의도 매칭 없음)\n");
    } else {
        for m in intents {
            // ts_ms → "YYYY-MM-DD" (UTC). chrono 의존을 피하기 위해 std 만 사용.
            let date = format_ymd_utc(m.timestamp_ms);
            // ~200 char cap (char 기준, byte 아님). 줄바꿈은 공백으로 정규화.
            let collapsed: String = m
                .content
                .chars()
                .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
                .collect::<String>();
            let mut excerpt: String = collapsed.split_whitespace().collect::<Vec<_>>().join(" ");
            let cap = 200;
            if excerpt.chars().count() > cap {
                let trimmed: String = excerpt.chars().take(cap).collect();
                excerpt = format!("{}…", trimmed);
            }
            // matched keywords 는 trace 에 보존되지만 섹션 본문은 짧게 유지하기
            // 위해 처음 3개만 inline. 빈 리스트면 표시 생략.
            let kw_str = if m.matched_keywords.is_empty() {
                String::new()
            } else {
                let preview: Vec<&String> = m.matched_keywords.iter().take(3).collect();
                let joined = preview
                    .iter()
                    .map(|k| k.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(" [keywords: {}]", joined)
            };
            s.push_str(&format!("- ({}) {}{}\n", date, excerpt, kw_str));
        }
    }
    s.push_str("[/USER_INTENT_LOOKUP]");
    s
}

/// timestamp(ms epoch) → "YYYY-MM-DD" (UTC). chrono 추가 없이 calendar 산출.
fn format_ymd_utc(ts_ms: i64) -> String {
    // 음수 타임스탬프는 unknown 으로 처리
    if ts_ms <= 0 {
        return "unknown".into();
    }
    let secs = ts_ms / 1_000;
    let days = secs / 86_400;

    // Unix epoch 1970-01-01 (Thu) 부터의 일수 → (Y, M, D)
    // Howard Hinnant's "days_from_civil" 의 역함수.
    let z = days + 719_468;
    let era = if z >= 0 { z / 146_097 } else { (z - 146_096) / 146_097 };
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    format!("{:04}-{:02}-{:02}", year, m, d)
}

/// Build a normalized enriched prompt for non-Claude engines.
///
/// Includes the same context sections as Claude's full ContextPack, but assembled
/// into a single prompt string (non-Claude engines don't support system_prompt separation).
///
/// Sections included (same as Claude path):
/// - Project path
/// - Recent conversation context + parent context (branch)
/// - Plan / Findings / Artifacts (Standard+)
/// - Skills / rawq / cross-session (Full)
/// - Thread inheritance (branch)
#[allow(dead_code)]
pub fn build_normalized_prompt(
    conn: &Connection,
    conversation_id: &str,
    prompt: &str,
    project_path: Option<&str>,
    active_skills: &[String],
    cross_session_ids: &[String],
    persona_fragment: Option<&str>,
) -> (String, ContextPackMeta) {
    let (assembled, _, meta) = build_normalized_prompt_with_budget(conn, conversation_id, prompt, project_path, active_skills, cross_session_ids, persona_fragment, None, None, None);
    (assembled, meta)
}

/// Phase B: Assemble a ContextPack prompt from pre-loaded data. Pure function — no DB dependency.
///
/// Takes a `ContextData` struct (from `load_context_data`) and produces the same
/// `(assembled, system_context, meta)` tuple as the original monolithic function.
pub fn assemble_prompt(
    data: &ContextData,
    identity_fragment: Option<&str>,
) -> (String, Option<String>, ContextPackMeta) {
    use super::super::context_pack::*;
    use crate::guardrail;
    use super::super::compression::maybe_compress_section_typed;

    let mut included_sections: Vec<String> = Vec::new();

    let total_budget = data.context_budget_cap.unwrap_or(guardrail::MAX_TOTAL_PROMPT);

    // Dynamic budget allocation — measure actual content, distribute proportionally.
    // Weight policy (Structured Memory Source Strengthening):
    //   Structured task memory (plan/findings/artifacts) > Conversational memory (compressed) > Cross-session
    //   "현재 작업과 직접 연결된 구조화 객체"가 "대화 요약"보다 우선한다.
    let budget_alloc = guardrail::allocate_budgets(total_budget, &[
        // Structured task memory — highest priority
        guardrail::SectionBudget { name: "plan",       content_len: data.plan_section.as_ref().map_or(0, |s| s.len()),     weight: 1.5, min_chars: 500,  max_chars: guardrail::MAX_PLAN_SECTION },
        guardrail::SectionBudget { name: "plan-doc",   content_len: data.plan_document.as_ref().map_or(0, |s| s.len()),    weight: 2.0, min_chars: 1000, max_chars: 6000 },
        guardrail::SectionBudget { name: "findings",   content_len: data.findings_section.as_ref().map_or(0, |s| s.len()), weight: 1.2, min_chars: 500,  max_chars: guardrail::MAX_FINDINGS_SECTION },
        guardrail::SectionBudget { name: "artifacts",  content_len: data.artifacts_section.as_ref().map_or(0, |s| s.len()),weight: 1.0, min_chars: 300,  max_chars: guardrail::MAX_ARTIFACTS_SECTION },
        // Supplementary sources
        guardrail::SectionBudget { name: "skills",     content_len: if data.active_skills.is_empty() { 0 } else { 2000 },  weight: 0.8, min_chars: 500,  max_chars: guardrail::MAX_SKILLS_SECTION },
        guardrail::SectionBudget { name: "rawq",       content_len: if data.retrieval_chunks.is_empty() { 0 } else { 1000 }, weight: 0.8, min_chars: 500, max_chars: guardrail::MAX_RAWQ_SECTION },
        guardrail::SectionBudget { name: "retrieval",  content_len: data.retrieval_chunks.len() * 300,                     weight: 1.0, min_chars: 500,  max_chars: guardrail::MAX_RETRIEVAL_SECTION },
        // Conversational memory — lower than structured task memory
        guardrail::SectionBudget { name: "compressed", content_len: data.compressed_memory.as_ref().map_or(0, |s| s.len()),weight: 0.7, min_chars: 500,  max_chars: guardrail::MAX_COMPRESSED_MEMORY_SECTION },
        guardrail::SectionBudget { name: "cross",      content_len: if data.cross_session_data.is_empty() { 0 } else { 1000 }, weight: 0.4, min_chars: 300, max_chars: guardrail::MAX_CROSS_SESSION_SECTION },
    ]);
    let dyn_cap = |name: &str| -> usize {
        budget_alloc.iter().find(|(n, _)| *n == name).map_or(2000, |(_, c)| *c)
    };

    let (ctx_mode, auto_reason) = determine_context_mode(data);

    // ─── Mode-specific context assembly profile ──────────────────────────
    #[allow(dead_code)]
    struct ModeProfile {
        context_cap: usize,
        retrieval_cap: usize,
        compressed_cap: usize,
        cross_session_cap: usize,
        retrieval_min_remaining: usize,
        compressed_min_remaining: usize,
        retrieval_content_max: usize,
        context_message_max: usize,
    }

    let profile = match ctx_mode {
        ContextMode::Lite => ModeProfile {
            context_cap: 4_000,
            retrieval_cap: 2_000,
            compressed_cap: 3_000,
            cross_session_cap: 2_000,
            retrieval_min_remaining: 3_000,
            compressed_min_remaining: 1_500,
            retrieval_content_max: 150,
            context_message_max: 300,
        },
        ContextMode::Standard => ModeProfile {
            context_cap: guardrail::MAX_CONTEXT_SECTION,
            retrieval_cap: guardrail::MAX_RETRIEVAL_SECTION,
            compressed_cap: guardrail::MAX_COMPRESSED_MEMORY_SECTION,
            cross_session_cap: guardrail::MAX_CROSS_SESSION_SECTION,
            retrieval_min_remaining: if total_budget >= 80_000 { 3_000 } else { 4_000 },
            compressed_min_remaining: 2_000,
            retrieval_content_max: 250,
            context_message_max: 400,
        },
        ContextMode::Full => ModeProfile {
            context_cap: 8_000,
            retrieval_cap: 6_000,
            compressed_cap: 6_000,
            cross_session_cap: 6_000,
            retrieval_min_remaining: 2_000,
            compressed_min_remaining: 1_500,
            retrieval_content_max: 400,
            context_message_max: 500,
        },
    };

    eprintln!("[context_pack] mode={:?}({}) budget={} ctx_cap={} ret_cap={} cmp_cap={}", ctx_mode, auto_reason, total_budget, profile.context_cap, profile.retrieval_cap, profile.compressed_cap);

    let mut sections: Vec<String> = Vec::new();
    let mut section_sizes: Vec<(String, usize)> = Vec::new();

    // Project
    if let Some(p) = &data.project_path {
        sections.push(format!("Project: {}", p));
        included_sections.push("project".into());
    }

    // Phase 2 — when conventions are synced into the project's CLAUDE.md /
    // AGENTS.md / GEMINI.md, the static layers are prepended automatically by
    // the CLI and live in the prompt-cache window. Skip them here to avoid
    // sending the same text twice (and invalidating the cache).
    // Identity stays in-line because it's RT-specific and not persisted.
    let skip_static = data.conventions_synced;

    // Tier 0: tunaFlow platform instructions (always injected, minimal footprint)
    if !skip_static {
        sections.push(PLATFORM_TIER0.to_string());
        included_sections.push("platform".into());
    } else {
        included_sections.push("platform:skipped".into());
    }

    // Agent role document (docs/agents/{role}.md) — injected right after platform
    if let Some(role_doc) = &data.agent_role_doc {
        if !skip_static {
            sections.push(format!("## Agent Role Instructions\n\n{}", role_doc));
            included_sections.push("agent-role".into());
        } else {
            included_sections.push("agent-role:skipped".into());
        }
    }

    // Previous implementation status — Architect revision 경로에서만 주입.
    // rev.N 설계 시 이전 impl branch 의 변경 파일/최근 findings 를 노출해
    // "어떤 파일을 keep/modify/revert 할지" 판단 근거를 제공. (PR-2)
    if let Some(section) = &data.previous_impl_status {
        sections.push(section.clone());
        included_sections.push("previous-impl".into());
    }

    // User Worldview — 사용자 stance 문서. identity 바로 앞에 삽입 (INV-1).
    // skip_static 에 영향받지 않음: 파일 기반이지만 CLAUDE.md/AGENTS.md 에 sync 되지 않는 별도 경로.
    if let Some(worldview_text) = crate::commands::worldview::load_for_injection(data.project_path.as_deref()) {
        sections.push(format!("## User Worldview\n\n{}", worldview_text));
        included_sections.push("worldview".into());
    }

    // Project identity summary — 최근 분석 결과 (projectIdentityAnalysisPlan subtask-03).
    // worldview 뒤, identity 앞에 삽입. context_loading 에서 pre-load 해 둔 fragment
    // 만 참조하므로 여기서 DB 접근 없음. frontmatter 는 이미 strip 된 상태.
    if let Some(identity_text) = &data.identity_summary_fragment {
        sections.push(format!("## Project Identity\n\n{}", identity_text));
        included_sections.push("project-identity".into());
    }

    // User intent lookup — userIntentSsotSurfacingPlan §Layer 1.
    // architect persona 진입 시에만 빌드되며 (developer/reviewer 는 None →
    // 본 블록 skip), 매칭이 0건이어도 빈 섹션을 inline 해 INV-1 을 만족.
    // 위치: project-identity 직후, identity/persona 직전 — agent 가 자기 정체성
    // /역할을 인지하기 직전에 사용자의 명시 의도를 먼저 본다.
    if let Some(intents) = &data.intent_lookup {
        sections.push(build_user_intent_lookup_section(intents));
        included_sections.push("intent-lookup".into());
    }

    // Identity + Persona section
    {
        let (identity_block, persona_block) = parse_identity_and_persona(identity_fragment);
        if let Some(id) = &identity_block {
            // Identity is RT-specific and always needed (not synced to file).
            sections.push(id.clone());
            included_sections.push("identity".into());
        }
        if let Some(p) = &persona_block {
            if !skip_static {
                sections.push(p.clone());
                included_sections.push("persona".into());
            } else {
                included_sections.push("persona:skipped".into());
            }
        }
    }

    // User profile section — injected right after identity/persona
    if let Some(profile_json) = &data.user_profile {
        if skip_static {
            included_sections.push("user-profile:skipped".into());
        } else if let Ok(p) = serde_json::from_str::<serde_json::Value>(profile_json) {
            let mut lines: Vec<String> = Vec::new();
            if let Some(v) = p.get("name").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                lines.push(format!("Name: {}", v));
            }
            if let Some(v) = p.get("title").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                lines.push(format!("Role: {}", v));
            }
            if let Some(v) = p.get("bio").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                lines.push(format!("Background: {}", v));
            }
            if let Some(v) = p.get("preferredLanguages").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                lines.push(format!("Preferred languages: {}", v));
            }
            if let Some(v) = p.get("gitName").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                lines.push(format!("Git name: {}", v));
            }
            if let Some(v) = p.get("gitEmail").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                lines.push(format!("Git email: {}", v));
            }
            if let Some(v) = p.get("githubOrg").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
                lines.push(format!("GitHub org: {}", v));
            }
            if !lines.is_empty() {
                sections.push(format!("## User\n\n{}", lines.join("\n")));
                included_sections.push("user-profile".into());
            }
        }
    }

    // ─── Conversation participants meta ────────────────────────────────
    // Always-present section listing which agents participated in this conversation.
    // Ensures agents never lose awareness of other agents' presence, even when
    // individual messages fall outside the recent context window.
    {
        let mut agent_map: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
        for (role, content, engine, persona) in &data.current_messages {
            if role == "assistant" {
                let label = match (persona, engine) {
                    (Some(p), Some(e)) if !p.is_empty() => format!("{} ({})", p, e),
                    (None, Some(e)) if !e.is_empty() => format!("({})", e),
                    _ => continue,
                };
                // Keep latest content preview per agent
                let preview = if content.len() > 80 {
                    format!("{}…", &content[..content.char_indices().take_while(|&(i,_)| i<=80).last().map_or(0,|(i,_)|i)])
                } else { content.clone() };
                agent_map.insert(label, preview);
            }
        }
        if !agent_map.is_empty() {
            let mut meta_section = String::from("## Conversation participants\n\nAgents active in this conversation:\n");
            for (agent, last_msg) in &agent_map {
                meta_section.push_str(&format!("- **{}**: {}\n", agent, last_msg));
            }
            sections.push(meta_section);
            included_sections.push("participants".into());
        }
    }

    // ─── Recent conversation context ───
    //
    // **Session continuation → DROP entirely** (Claude/Codex 자체 세션 history 가 있음).
    // tunaFlow 가 truncate 된 prepend 를 넣으면 모델이 그걸 권위 있는 최신 버전으로 오인해
    // 자기 직전 답변 끝부분을 무시하는 오염이 발생. 에이전트는 필요시 `recent_turns:N`
    // 도구로 명시 조회.
    //
    // **Fresh session → Full + anchor 2 turns**. tunaFlow 가 유일한 history 제공자이므로
    // 마지막 2 turn 은 `context_message_max` truncate 예외로 전문 포함.
    if data.is_session_continuation {
        included_sections.push("context:skipped(session-continuation)".into());
    } else {
        // Step 1: Identify each agent's last message index (must-include set)
        let mut agent_last_idx: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (i, (role, _, engine, persona)) in data.current_messages.iter().enumerate() {
            if role == "assistant" {
                let key = match (persona, engine) {
                    (Some(p), _) if !p.is_empty() => p.clone(),
                    (_, Some(e)) if !e.is_empty() => e.clone(),
                    _ => continue,
                };
                agent_last_idx.insert(key, i);
            }
        }
        let must_include: std::collections::HashSet<usize> = agent_last_idx.values().copied().collect();

        // Step 2: Budget-based trimming. 가장 마지막 2 turn 은 anchor 로 context_message_max 예외.
        // 직전 user + 직전 assistant 를 전문으로 포함 → 엔진 전환/새 세션 시 에이전트가
        // 자기 직전 답변 끝부분(선택지/결론)을 확실히 보게 함.
        let keep_recent = 5;
        let total_msgs = data.current_messages.len();
        let anchor_start = total_msgs.saturating_sub(2); // 최근 2 개 index 이상은 anchor
        let mut trimmed: Vec<(String, String, Option<String>, Option<String>)> = Vec::new();
        let mut char_budget = profile.context_cap;

        for (i, msg) in data.current_messages.iter().enumerate().rev() {
            let is_recent = i >= total_msgs.saturating_sub(keep_recent);
            let is_anchor = i >= anchor_start;
            let content = if is_recent {
                msg.1.clone()
            } else {
                crate::commands::conversation_memory::prune_tool_results(&msg.1)
            };
            // anchor 는 context_message_max 적용 안 함 (전문 길이 그대로)
            let effective_len = if is_anchor { content.len() } else { content.len().min(profile.context_message_max) };
            let msg_cost = msg.0.len() + effective_len + 40;
            if msg_cost <= char_budget || is_anchor {
                // anchor 는 budget 초과여도 무조건 포함 (품질 우선)
                trimmed.push((msg.0.clone(), content, msg.2.clone(), msg.3.clone()));
                char_budget = char_budget.saturating_sub(msg_cost.min(char_budget));
            } else if must_include.contains(&i) {
                trimmed.push((msg.0.clone(), content, msg.2.clone(), msg.3.clone()));
                char_budget = 0;
            }
        }
        trimmed.reverse();

        let trimmed_owned = trimmed;

        if let Some(ctx) = maybe_compress_section_typed(
            build_context_summary_with_authors(&trimmed_owned, &data.parent_messages, data.is_branch),
            profile.context_cap,
            Some("context"),
        ) {
            sections.push(ctx);
            included_sections.push("context".into());
        }
    }

    // ═══ UNIFIED MEMORY POLICY ═══
    // Priority: structured task memory > conversational memory > cross-session
    // Structured = plan/findings/artifacts (작업 continuity)
    // Conversational = compressed memory (대화 continuity)

    // Layer 2: Structured task memory — highest priority after context window
    if ctx_mode >= ContextMode::Standard {
        if let Some(s) = guardrail::truncate_section(
            data.plan_section.clone(),
            dyn_cap("plan"),
        ) {
            sections.push(s);
            included_sections.push("plan".into());
        }
        // Plan document (full markdown from project docs/plans/)
        if let Some(doc) = &data.plan_document {
            let doc_cap = dyn_cap("plan-doc");
            let truncated = if doc.len() > doc_cap {
                let mut end = doc_cap;
                while end > 0 && !doc.is_char_boundary(end) { end -= 1; }
                format!("{}\n\n[... truncated]", &doc[..end])
            } else { doc.clone() };
            sections.push(format!("## Plan Document\n\n{}", truncated));
            included_sections.push("plan-document".into());
        }
        if let Some(s) = guardrail::truncate_section(
            data.findings_section.clone(),
            dyn_cap("findings"),
        ) {
            sections.push(s);
            included_sections.push("findings".into());
        }
        if let Some(s) = guardrail::truncate_section(
            data.artifacts_section.clone(),
            dyn_cap("artifacts"),
        ) {
            sections.push(s);
            included_sections.push("artifacts".into());
        }
    }

    // Layer 3: Retrieval memory — past conversation chunks, ranked and deduped.
    // Session continuation 에서는 drop (Claude/Codex 자체 세션이 prior context 갖고 있음).
    // brand same-session 진입 시 main 의 retrieval 까지 brand 에 prepend 하면 토큰 낭비
    // + 모델이 "tunaFlow 가 권위 있는 latest" 로 오인 (Layer B, branchInheritsMainSessionPlan).
    if data.is_session_continuation {
        included_sections.push("retrieval:skipped(session-continuation)".into());
    } else if ctx_mode >= ContextMode::Standard {
        let current_size: usize = sections.iter().map(|s| s.len()).sum();
        let remaining = total_budget.saturating_sub(current_size);
        if remaining > profile.retrieval_min_remaining {
            // Apply mode-specific limit to pre-loaded chunks
            let retrieval_limit = match ctx_mode {
                ContextMode::Lite => 3,
                ContextMode::Standard => 6,
                ContextMode::Full => 10,
            };
            let chunks: Vec<&crate::commands::context_queries::RetrievedChunk> =
                data.retrieval_chunks.iter().take(retrieval_limit).collect();
            if !chunks.is_empty() {
                let mut section = String::from("## Relevant prior conversation\n\nPast conversation chunks relevant to the current question.\n");
                for chunk in &chunks {
                    let kind_label = match chunk.kind { "pair" => "Q&A", "anchor" => "Branch anchor", "brief" => "RT brief", _ => chunk.kind };
                    section.push_str(&format!("\n--- {} ---\n", kind_label));
                    for (role, content, engine, persona) in &chunk.messages {
                        let author = match (role.as_str(), persona, engine) {
                            ("assistant", Some(p), Some(e)) if !p.is_empty() => format!("assistant:{} ({})", p, e),
                            ("assistant", None, Some(e)) if !e.is_empty() => format!("assistant ({})", e),
                            _ => role.clone(),
                        };
                        let truncated = if content.len() > profile.retrieval_content_max {
                            let end = content.char_indices().take_while(|&(i, _)| i <= profile.retrieval_content_max).last().map_or(0, |(i, _)| i);
                            format!("{}…", &content[..end])
                        } else { content.clone() };
                        section.push_str(&format!("[{}] {}\n", author, truncated));
                    }
                }
                if let Some(s) = guardrail::truncate_section(Some(section), profile.retrieval_cap) {
                    sections.push(s);
                    included_sections.push("retrieval".into());
                }
            }
        } else {
            eprintln!("[memory_policy] retrieval skipped — remaining {} < threshold {}", remaining, profile.retrieval_min_remaining);
            included_sections.push("retrieval:skipped".into());
        }
    }

    // Layer 3b: Project document context — relevant docs/plans/ideas sections
    if ctx_mode >= ContextMode::Standard && !data.document_chunks.is_empty() {
        let current_size: usize = sections.iter().map(|s| s.len()).sum();
        let remaining = total_budget.saturating_sub(current_size);
        if remaining > 2_000 {
            let doc_cap = match ctx_mode {
                ContextMode::Lite => 1_500,
                ContextMode::Standard => 3_000,
                ContextMode::Full => 5_000,
            };
            let mut section = String::from("## Related project documentation\n\nRelevant sections from project documents (plans, ideas, references).\n");
            for (file_path, section_title, text_preview, score) in &data.document_chunks {
                let title = section_title.as_deref().unwrap_or("(intro)");
                section.push_str(&format!("\n--- {} > {} (relevance: {:.0}%) ---\n{}\n",
                    file_path, title, score * 100.0, text_preview));
            }
            if let Some(s) = guardrail::truncate_section(Some(section), doc_cap) {
                sections.push(s);
                included_sections.push("document-rag".into());
            }
        }
    }

    // Layer 4: Compressed conversation memory — continuity layer
    // Session continuation 에서는 drop (Claude 세션이 raw history 갖고 있음).
    if data.is_session_continuation {
        included_sections.push("compressed-memory:skipped(session-continuation)".into());
    } else {
        let current_size: usize = sections.iter().map(|s| s.len()).sum();
        let remaining = total_budget.saturating_sub(current_size);
        if remaining > profile.compressed_min_remaining {
            if let Some(memory) = &data.compressed_memory {
                // 출처 표기 — 엔진 전환 시 "Claude 가 요약한 내용" 을 Codex 가
                // 자기 작업으로 오인하지 않도록 생성 주체를 명시.
                let source_note = match &data.compressed_memory_source {
                    Some(src) if !src.is_empty() => format!(" (generated by {})", src),
                    _ => String::new(),
                };
                if let Some(s) = guardrail::truncate_section(Some(format!(
                    "## Compressed conversation memory{src}\n\n\
                    Structured summary of older messages. For current task details, see Plan/Findings/Artifacts above.\n\n\
                    {body}",
                    src = source_note,
                    body = memory,
                )), profile.compressed_cap) {
                    sections.push(s);
                    included_sections.push("compressed-memory".into());
                }
            }
        } else {
            eprintln!("[memory_policy] compressed-memory skipped — remaining {} < threshold {}", remaining, profile.compressed_min_remaining);
            included_sections.push("compressed-memory:skipped".into());
        }
    }

    // Layer 5+: Skills, rawq, cross-session (supplementary sources)
    // Tiering: skills/cross-session are Tier 2 (Pull via tool-request) unless explicitly active.
    // Agents can request them on-demand: <!-- tunaflow:tool-request:skills:KEYWORD -->
    if !data.active_skills.is_empty() {
        if let Some(s) = guardrail::truncate_section(
            build_skills_section(&data.active_skills, &data.prompt),
            dyn_cap("skills"),
        ) {
            sections.push(s);
            included_sections.push("skills".into());
        }
    } else if ctx_mode >= ContextMode::Full {
        // Full mode: still include keyword-matched skills as before
        if let Some(s) = guardrail::truncate_section(
            build_skills_section(&data.active_skills, &data.prompt),
            dyn_cap("skills"),
        ) {
            sections.push(s);
            included_sections.push("skills".into());
        }
    }
    // rawq: mode-independent — prompt_needs_rawq() internally decides
    if let Some(s) = guardrail::truncate_section(
        build_rawq_section(data.project_path.as_deref(), &data.prompt),
        dyn_cap("rawq"),
    ) {
        sections.push(s);
        included_sections.push("rawq".into());
    }
    // code-review-graph: structural change impact — best-effort, skip if unavailable
    if ctx_mode >= ContextMode::Standard {
        if let Some(s) = guardrail::truncate_section(
            build_crg_section(data.project_path.as_deref().unwrap_or("")),
            2000,
        ) {
            sections.push(s);
            included_sections.push("graph".into());
        }
    }
    // context-hub: handled via tool-request markers (<!-- tunaflow:tool-request:docs:QUERY -->)
    // cross-session: Tier 2 in Lite/Standard, Push only in Full mode.
    // Agents can request via <!-- tunaflow:tool-request:sessions:QUERY -->
    if ctx_mode >= ContextMode::Full && !data.cross_session_data.is_empty() {
        if let Some(s) = maybe_compress_section_typed(
            build_cross_session_section(&data.cross_session_data),
            profile.cross_session_cap,
            Some("cross-session"),
        ) {
            sections.push(s);
            included_sections.push("cross-session".into());
        }
    }

    // Thread inheritance (branch)
    if data.is_branch {
        if let Some(s) = &data.thread_inheritance {
            sections.push(s.clone());
            included_sections.push("thread-inheritance".into());
        }
    }

    // Build section sizes from sections + included_sections (1:1 correspondence for active ones)
    {
        let active_names: Vec<&str> = included_sections.iter()
            .filter(|s| !s.contains(":skipped"))
            .map(|s| s.as_str())
            .collect();
        for (i, name) in active_names.iter().enumerate() {
            if let Some(sec) = sections.get(i) {
                section_sizes.push((name.to_string(), sec.len()));
            }
        }
    }

    // Memory policy summary log
    {
        let total_chars: usize = sections.iter().map(|s| s.len()).sum();
        let active: Vec<&str> = included_sections.iter()
            .filter(|s| !s.contains(":skipped"))
            .map(|s| s.as_str())
            .collect();
        let skipped: Vec<&str> = included_sections.iter()
            .filter(|s| s.contains(":skipped"))
            .map(|s| s.as_str())
            .collect();
        let skipped_str = if skipped.is_empty() { "none".to_string() } else { skipped.join(",") };
        let mut sorted_sizes = section_sizes.clone();
        sorted_sizes.sort_by(|a, b| b.1.cmp(&a.1));
        let top3: Vec<String> = sorted_sizes.iter().take(3).map(|(n, s)| format!("{}={:.1}k", n, *s as f64 / 1000.0)).collect();
        let structured: Vec<&str> = active.iter().filter(|s| matches!(*s, &"plan" | &"plan-document" | &"findings" | &"artifacts")).copied().collect();
        let conversational: Vec<&str> = active.iter().filter(|s| matches!(*s, &"compressed-memory" | &"cross-session")).copied().collect();
        eprintln!(
            "[memory_policy] budget={}/{} structured=[{}] conversational=[{}] skipped=[{}] top=[{}]",
            total_chars, total_budget, structured.join(","), conversational.join(","), skipped_str, top3.join(","),
        );
    }

    // Assemble final prompt
    let system_context = if sections.is_empty() {
        None
    } else {
        guardrail::enforce_total_limit(Some(sections.join("\n\n")), total_budget)
    };
    let assembled = match &system_context {
        Some(ctx) => format!("{}\n\n---\n\n{}", ctx, &data.prompt),
        None => data.prompt.clone(),
    };

    let ctx_mode_str = if auto_reason.starts_with("auto:") {
        format!("{:?}({})", ctx_mode, auto_reason)
    } else {
        format!("{:?}", ctx_mode)
    };
    let total_len = assembled.len();
    let truncated = total_len >= total_budget;

    let sizes_json = serde_json::to_string(
        &section_sizes.iter().map(|(n, s)| serde_json::json!({ "name": n, "chars": s })).collect::<Vec<_>>()
    ).unwrap_or_default();

    let meta = ContextPackMeta {
        mode: ctx_mode_str,
        sections: included_sections,
        length: total_len,
        hash: sizes_json,
        truncated,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
    };

    (assembled, system_context, meta)
}

/// Legacy entry point — delegates to load_context_data + assemble_prompt.
/// Signature and return type are unchanged; no caller modifications needed.
/// Determine the context mode based on user override or auto heuristic.
///
/// Scoring: skills≥3 (+2), skills>0 (+1), cross_session (+1), explicit persona (+1),
/// branch (+1), active plan (+1), short prompt (-1/-2).
/// Full ≥ 3, Lite ≤ -1, Standard otherwise.
/// Words that signal the user is asking about past context (retrieval needed).
const HISTORY_SIGNAL_WORDS: &[&str] = &[
    "처음", "이전", "그때", "정리", "요약", "기억", "논의", "결정", "히스토리",
    "first", "earlier", "previous", "history", "summarize", "remember", "decided",
];

fn determine_context_mode(data: &ContextData) -> (ContextMode, &'static str) {
    match data.context_mode_override.as_deref() {
        Some("full") => (ContextMode::Full, "user-override"),
        Some("standard") => (ContextMode::Standard, "user-override"),
        Some("lite") => (ContextMode::Lite, "user-override"),
        _ => {
            let mut score: i32 = 0;
            if data.active_skills.len() >= 3 { score += 2; }
            else if !data.active_skills.is_empty() { score += 1; }
            if !data.cross_session_ids.is_empty() { score += 1; }
            let has_explicit_persona = data.persona_fragment
                .as_ref()
                .map(|f| f.contains("## Persona"))
                .unwrap_or(false);
            if has_explicit_persona { score += 1; }
            if data.is_branch { score += 1; }
            if data.has_active_plan { score += 1; }
            if data.prompt.len() < 50 { score -= 1; }
            if data.prompt.len() < 20 { score -= 1; }

            // Long conversations need retrieval — never drop to Lite
            let msg_count = data.current_messages.len();
            if msg_count >= 20 && score < 0 {
                score = 0; // Floor at Standard for long conversations
                eprintln!("[auto_mode] floor: {} msgs, preventing Lite", msg_count);
            }

            // History signal words → boost to ensure retrieval is included
            let prompt_lower = data.prompt.to_lowercase();
            if HISTORY_SIGNAL_WORDS.iter().any(|w| prompt_lower.contains(w)) {
                score = score.max(1); // At least Standard
                eprintln!("[auto_mode] history signal detected in prompt");
            }

            let (mode, reason) = if score >= 3 {
                (ContextMode::Full, "auto:full(skills/cross/plan)")
            } else if score <= -1 {
                (ContextMode::Lite, "auto:lite(short-prompt)")
            } else {
                (ContextMode::Standard, "auto:standard(baseline)")
            };
            eprintln!("[auto_mode] score={} msgs={} → {:?} reason={}", score, msg_count, mode, reason);
            (mode, reason)
        }
    }
}

pub fn build_normalized_prompt_with_budget(
    conn: &Connection,
    conversation_id: &str,
    prompt: &str,
    project_path: Option<&str>,
    active_skills: &[String],
    cross_session_ids: &[String],
    persona_fragment: Option<&str>,
    context_mode_override: Option<&str>,
    context_budget_cap: Option<usize>,
    user_profile_json: Option<&str>,
) -> (String, Option<String>, ContextPackMeta) {
    let data = load_context_data(
        conn, conversation_id, prompt, project_path,
        active_skills, cross_session_ids, persona_fragment,
        context_mode_override, context_budget_cap, user_profile_json,
    );
    assemble_prompt(&data, persona_fragment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn empty_context_data(project_path: Option<String>) -> ContextData {
        ContextData {
            conversation_id: "conv-test".into(),
            project_path,
            prompt: "hi".into(),
            is_branch: false,
            has_active_plan: false,
            current_messages: vec![],
            parent_messages: vec![],
            plan_section: None,
            plan_document: None,
            findings_section: None,
            artifacts_section: None,
            retrieval_chunks: vec![],
            document_chunks: vec![],
            compressed_memory: None,
            compressed_memory_source: None,
            cross_session_data: vec![],
            thread_inheritance: None,
            agent_role_doc: None,
            previous_impl_status: None,
            active_skills: vec![],
            cross_session_ids: vec![],
            persona_fragment: None,
            context_mode_override: None,
            context_budget_cap: None,
            user_profile: None,
            conventions_synced: false,
            is_session_continuation: false,
            identity_summary_fragment: None,
            intent_lookup: None,
        }
    }

    /// INV-1 (userWorldviewInjectionPlan-task-01): worldview 는 identity 바로 앞.
    /// project/platform/agent-role 은 worldview 앞에 유지.
    #[test]
    fn worldview_injected_immediately_before_identity() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().to_path_buf();
        let wv_path = project_dir.join(".tunaflow").join("user_worldview.md");
        fs::create_dir_all(wv_path.parent().unwrap()).unwrap();
        fs::write(&wv_path, "# stance\n\n본 사용자는 CLI-first 원칙을 따른다.").unwrap();

        let data = empty_context_data(Some(project_dir.to_string_lossy().to_string()));
        let identity_fragment = "## Identity\n\ntunaFlow test agent.";
        let (_assembled, _sys, meta) = assemble_prompt(&data, Some(identity_fragment));

        let idx_worldview = meta.sections.iter().position(|s| s == "worldview");
        let idx_identity = meta.sections.iter().position(|s| s == "identity");

        assert!(idx_worldview.is_some(), "worldview 섹션이 주입되어야 함");
        assert!(idx_identity.is_some(), "identity 섹션은 항상 존재");
        assert_eq!(
            idx_worldview.unwrap() + 1,
            idx_identity.unwrap(),
            "worldview 는 identity 바로 앞이어야 함 (INV-1)"
        );

        // project/platform/agent-role 은 worldview 앞에 있어야 함 (단, 이 테스트에선 agent-role 없음)
        let idx_platform = meta.sections.iter().position(|s| s == "platform");
        if let Some(p) = idx_platform {
            assert!(p < idx_worldview.unwrap(), "platform 은 worldview 앞");
        }
    }

    #[test]
    fn identity_summary_injected_between_worldview_and_identity() {
        // subtask-03 INV: project_identity 는 worldview 뒤, identity 앞.
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().to_path_buf();
        let wv_path = project_dir.join(".tunaflow").join("user_worldview.md");
        fs::create_dir_all(wv_path.parent().unwrap()).unwrap();
        fs::write(&wv_path, "# worldview body").unwrap();

        let mut data = empty_context_data(Some(project_dir.to_string_lossy().to_string()));
        data.identity_summary_fragment = Some("### Project identity\nsummary body".into());

        let (_a, _s, meta) = assemble_prompt(&data, Some("## Identity\n\ntest"));
        let idx_wv = meta.sections.iter().position(|s| s == "worldview").unwrap();
        let idx_id = meta.sections.iter().position(|s| s == "identity").unwrap();
        let idx_pi = meta
            .sections
            .iter()
            .position(|s| s == "project-identity")
            .expect("project-identity 섹션이 존재해야");
        assert!(idx_wv < idx_pi, "project-identity 는 worldview 뒤");
        assert!(idx_pi < idx_id, "project-identity 는 identity 앞");
    }

    #[test]
    fn identity_summary_absent_when_fragment_is_none() {
        let tmp = TempDir::new().unwrap();
        let data = empty_context_data(Some(tmp.path().to_string_lossy().to_string()));
        let (_a, _s, meta) = assemble_prompt(&data, Some("## Identity\n\ntest"));
        assert!(!meta.sections.iter().any(|s| s == "project-identity"));
    }

    #[test]
    fn worldview_absent_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().to_path_buf();
        // 파일 생성 안 함
        let data = empty_context_data(Some(project_dir.to_string_lossy().to_string()));
        let (_assembled, _sys, meta) = assemble_prompt(&data, Some("## Identity\n\ntest"));

        // 전역 ~/.tunaflow/user_worldview.md 가 실제 머신에 있으면 "worldview" 가 있을 수 있음.
        // 그 경우도 identity 바로 앞이라는 INV-1 은 유지되어야 함.
        if let Some(idx_wv) = meta.sections.iter().position(|s| s == "worldview") {
            let idx_id = meta.sections.iter().position(|s| s == "identity").unwrap();
            assert_eq!(idx_wv + 1, idx_id, "global worldview 도 identity 바로 앞이어야 함");
        } else {
            // worldview 없으면 identity 는 여전히 존재
            assert!(meta.sections.iter().any(|s| s == "identity"));
        }
    }

    // ─── userIntentSsotSurfacingPlan: [USER_INTENT_LOOKUP] 섹션 ─────────────

    #[test]
    fn intent_lookup_section_present_for_architect_even_with_zero_matches() {
        // INV-1: architect 진입 시 매칭 0건이어도 빈 섹션이 항상 inline.
        let tmp = TempDir::new().unwrap();
        let mut data = empty_context_data(Some(tmp.path().to_string_lossy().to_string()));
        data.intent_lookup = Some(Vec::new());

        let (assembled, _, meta) = assemble_prompt(&data, Some("## Identity\n\ntest"));
        assert!(meta.sections.iter().any(|s| s == "intent-lookup"),
            "architect 면 intent-lookup 섹션이 항상 출현: {:?}", meta.sections);
        assert!(assembled.contains("[USER_INTENT_LOOKUP]"));
        assert!(assembled.contains("[/USER_INTENT_LOOKUP]"));
        assert!(assembled.contains("관련 사용자 의도 매칭 없음"),
            "0건이면 '매칭 없음' 메시지");
    }

    #[test]
    fn intent_lookup_section_absent_for_developer_or_reviewer() {
        // INV-1 보완: architect 가 아닌 role (intent_lookup=None) 은 섹션 미출력.
        let tmp = TempDir::new().unwrap();
        let mut data = empty_context_data(Some(tmp.path().to_string_lossy().to_string()));
        data.intent_lookup = None;

        let (_, _, meta) = assemble_prompt(&data, Some("## Identity\n\ntest"));
        assert!(!meta.sections.iter().any(|s| s == "intent-lookup"),
            "developer/reviewer 면 섹션 없음: {:?}", meta.sections);
    }

    #[test]
    fn intent_lookup_section_renders_matches_and_caps_excerpt_at_200_chars() {
        use super::super::context_loading::UserIntentMatch;

        let tmp = TempDir::new().unwrap();
        let mut data = empty_context_data(Some(tmp.path().to_string_lossy().to_string()));
        // 길이 400 의 본문 — 200 char cap 검증
        let long_body: String = "가".repeat(400);
        data.intent_lookup = Some(vec![
            UserIntentMatch {
                timestamp_ms: 1_700_000_000_000, // 2023-11-14 UTC
                conversation_id: "c-old".into(),
                content: long_body.clone(),
                score: 0.9,
                matched_keywords: vec!["session".into(), "branch".into(), "context".into(), "extra".into()],
            },
        ]);

        let (assembled, _, meta) = assemble_prompt(&data, Some("## Identity\n\ntest"));
        assert!(meta.sections.iter().any(|s| s == "intent-lookup"));
        assert!(assembled.contains("(2023-11-14)"));
        // truncate marker (…) 가 본문에 보여야 함
        assert!(assembled.contains("…"));
        // keywords preview 는 최대 3개
        assert!(assembled.contains("session, branch, context"));
        assert!(!assembled.contains("extra"));
        // 한 줄 길이 검증: '- (date) ' 헤더 (+12) + 200 char + '… [keywords: …]' 정도
        // 총합 길이가 350 미만 (cap 이 효과적으로 작동하면)
        let line = assembled.lines()
            .find(|l| l.contains("(2023-11-14)"))
            .expect("intent line 존재");
        // 200 char excerpt 만 포함되어야 → 전체 400 char 가 전부 들어가지 않아야
        let body_chars = line.matches('가').count();
        assert_eq!(body_chars, 200, "본문 cap = 200 char (한글 포함)");
    }

    #[test]
    fn intent_lookup_section_position_between_project_identity_and_identity() {
        // 위치 INV: project-identity 직후, identity 직전 (페르소나 인지 전 의도 surface).
        use super::super::context_loading::UserIntentMatch;
        let tmp = TempDir::new().unwrap();
        let mut data = empty_context_data(Some(tmp.path().to_string_lossy().to_string()));
        data.identity_summary_fragment = Some("### Project identity\nbody".into());
        data.intent_lookup = Some(vec![UserIntentMatch {
            timestamp_ms: 1_700_000_000_000,
            conversation_id: "c".into(),
            content: "branch session 이슈 정리".into(),
            score: 0.8,
            matched_keywords: vec!["branch".into()],
        }]);

        let (_, _, meta) = assemble_prompt(&data, Some("## Identity\n\ntest"));
        let idx_pi = meta.sections.iter().position(|s| s == "project-identity")
            .expect("project-identity 존재");
        let idx_il = meta.sections.iter().position(|s| s == "intent-lookup")
            .expect("intent-lookup 존재");
        let idx_id = meta.sections.iter().position(|s| s == "identity")
            .expect("identity 존재");
        assert!(idx_pi < idx_il, "intent-lookup 은 project-identity 뒤");
        assert!(idx_il < idx_id, "intent-lookup 은 identity 앞");
    }

    #[test]
    fn intent_lookup_meta_sections_json_is_valid() {
        // INV-1 보완: trace_log.ctx_sections 에 intent-lookup 이 그대로 들어가야.
        let tmp = TempDir::new().unwrap();
        let mut data = empty_context_data(Some(tmp.path().to_string_lossy().to_string()));
        data.intent_lookup = Some(Vec::new());

        let (_, _, meta) = assemble_prompt(&data, None);
        let json = meta.sections_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let arr = parsed.as_array().unwrap();
        assert!(arr.iter().any(|v| v.as_str() == Some("intent-lookup")),
            "sections_json 에 intent-lookup 포함: {}", json);
    }

    // ─── format_ymd_utc helper ─────────────────────────────────────────────

    #[test]
    fn format_ymd_handles_known_dates() {
        // 1970-01-01 00:00 UTC = 0 → unknown 으로 취급 (음수/0 은 의미 없는 값)
        assert_eq!(format_ymd_utc(0), "unknown");
        assert_eq!(format_ymd_utc(1), "1970-01-01");
        // 2023-11-14 22:13:20 UTC = 1_700_000_000 sec
        assert_eq!(format_ymd_utc(1_700_000_000_000), "2023-11-14");
        // 2026-05-02 00:00:00 UTC = 1_777_680_000 sec
        assert_eq!(format_ymd_utc(1_777_680_000_000), "2026-05-02");
    }

    #[test]
    fn format_ymd_handles_negative_or_zero() {
        assert_eq!(format_ymd_utc(0), "unknown");
        assert_eq!(format_ymd_utc(-1), "unknown");
    }
}
