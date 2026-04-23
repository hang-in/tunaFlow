//! projectIdentityAnalysisPlan subtask-03 — identity 분석 실행 경로.
//!
//! 입력: 최근 period 의 6 타입 identity-input artifact.
//! 출력: `identity_summary` 타입 artifact 1건 (고정 5 섹션 markdown + frontmatter).
//!
//! LLM 호출은 CLI-first 원칙 (`agents::claude::run`) 을 재사용. 테스트는
//! `InvokeAnalyzer::Stub` 으로 LLM 호출을 격리한다.
//!
//! Invariants:
//! - 섹션 누락 / 예산 초과 → 1 회 재생성 시도. 재실패 시 job='failed'.
//! - 출력 content 앞에 `---\nproject_key: ...\n---` frontmatter 부착 (ContextPack
//!   주입 시 strip).
//! - 분석 output 은 `create_identity_summary` (analyzer 전용) 로만 저장되며,
//!   자동 생성 경로 (`create_identity_input_artifact`) 는 IdentitySummary kind 거부.

use std::collections::HashMap;

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::agents::claude::{run as run_claude, RunInput};
use crate::db::models::Artifact;
use crate::errors::AppError;

// ─── Prompt template ────────────────────────────────────────────────────────

pub const IDENTITY_PROMPT_TEMPLATE: &str = r#"You are synthesizing project identity from the last completed plans' artifacts.

## Input metadata
- Project: {project_key}
- Period: {since} ~ {until}
- Plan done count (cumulative): {done_plan_count}

## Input artifacts (workflow-derived, 6 types)
Sorted by type then chronological. DO NOT treat user messages as input.

{artifacts_serialized}

## Output requirements
Produce markdown with EXACTLY these sections and token budgets.

Rules:
- Do not narrate.
- Write terse operational guidance.
- Prefer bullets over prose.
- Every line must be reusable as future instruction to an agent.
- If evidence is insufficient for a section, write "(insufficient evidence)" rather than inventing.

### Project identity  (150-250 tokens)
Nature / stage / primary users. One-line summary first, then bullets.

### User working preference  (300-500 tokens)
- Decision patterns observed (cite artifact ids)
- Preferred constraints / styles
- Explicit avoid-list

### Agent operating preference  (300-500 tokens)
- Which engine succeeds at which task category
- Common failure modes per engine
- Recommended delegation pattern

### Recent inflection points  (exactly 3 items, 100-150 tokens each)
For each:
- What changed (1 line)
- Why (cite artifact id)
- When (date)

### Do / Avoid  (200-300 tokens)
- Bullets only. No prose.
- Split into "Do" and "Avoid" lists explicitly.

## Total budget: 1,350 ~ 2,050 tokens. Exceeding or missing sections trigger regeneration.
"#;

pub const ARTIFACT_TYPE_ORDER: &[&str] = &[
    "decision",
    "review_outcome",
    "rework_reason",
    "finding_success",
    "finding_failure",
    "workflow_milestone",
];

/// content (JSON string) 을 max_chars 만큼 축약. JSON 파싱 실패 시 원문 앞부분을 반환.
pub fn summarize_content(content: &str, max_chars: usize) -> String {
    let trimmed = content.trim();
    let compact = trimmed.replace('\n', " ");
    let mut end = 0usize;
    for (i, _) in compact.char_indices() {
        if i >= max_chars {
            break;
        }
        end = i + 1;
    }
    if compact.chars().count() > max_chars {
        format!("{}...", &compact[..end])
    } else {
        compact
    }
}

/// Input artifact 들을 type 별로 그룹핑 후 timestamp 순 정렬. 프롬프트 본문에
/// 들어갈 markdown 생성.
pub fn serialize_artifacts_for_prompt(artifacts: &[Artifact]) -> String {
    let mut grouped: HashMap<&str, Vec<&Artifact>> = HashMap::new();
    for a in artifacts {
        grouped.entry(a.artifact_type.as_str()).or_default().push(a);
    }
    let mut out = String::new();
    for t in ARTIFACT_TYPE_ORDER {
        if let Some(items) = grouped.get(t) {
            let mut sorted: Vec<&&Artifact> = items.iter().collect();
            sorted.sort_by_key(|a| a.created_at);
            out.push_str(&format!("### {} ({} items)\n", t, sorted.len()));
            for a in sorted {
                out.push_str(&format!(
                    "- [{}] {} @ {}: {}\n",
                    a.id,
                    a.title,
                    a.created_at,
                    summarize_content(&a.content, 100),
                ));
            }
            out.push('\n');
        }
    }
    out
}

/// 프롬프트 최종 조립. token budget / 재생성 힌트 (hint) 는 caller 가 posthoc 추가.
pub fn build_prompt(
    project_key: &str,
    since: i64,
    until: i64,
    done_plan_count: i64,
    artifacts: &[Artifact],
) -> String {
    let serialized = serialize_artifacts_for_prompt(artifacts);
    IDENTITY_PROMPT_TEMPLATE
        .replace("{project_key}", project_key)
        .replace("{since}", &since.to_string())
        .replace("{until}", &until.to_string())
        .replace("{done_plan_count}", &done_plan_count.to_string())
        .replace("{artifacts_serialized}", &serialized)
}

// ─── Section validation ─────────────────────────────────────────────────────

/// 필수 섹션 헤더 + token budget 범위 (하한, 상한).
/// Recent inflection points 는 3 × (100~150) 합산 범위.
pub const REQUIRED_SECTIONS: &[(&str, (usize, usize))] = &[
    ("### Project identity", (150, 250)),
    ("### User working preference", (300, 500)),
    ("### Agent operating preference", (300, 500)),
    ("### Recent inflection points", (300, 450)),
    ("### Do / Avoid", (200, 300)),
];

#[derive(Debug, Clone, Serialize)]
pub struct IdentitySectionValidation {
    pub has_all_sections: bool,
    pub missing: Vec<String>,
    pub budget_ok: bool,
    /// (섹션명, 실제 추정 tokens)
    pub offending_sections: Vec<(String, usize)>,
}

impl IdentitySectionValidation {
    pub fn is_ok(&self) -> bool {
        self.has_all_sections && self.budget_ok
    }
}

/// word count * 1.3 휴리스틱으로 token 추정. tiktoken 의존성 없이 근사.
/// CJK 의 경우 word_count 가 작아 과소 추정 가능 — 실측 후 튜닝 (subtask Risk).
pub fn estimate_tokens(s: &str) -> usize {
    let words = s.split_whitespace().count();
    ((words as f64) * 1.3).round() as usize
}

pub fn validate_identity_summary_sections(content: &str) -> IdentitySectionValidation {
    let mut missing = Vec::new();
    let mut offending = Vec::new();

    for (header, (lo, hi)) in REQUIRED_SECTIONS {
        let Some(start) = content.find(header) else {
            missing.push((*header).to_string());
            continue;
        };
        let body_start = start + header.len();
        // 다음 "### " 헤더 전까지 또는 끝
        let rel_end = content[body_start..].find("\n### ").unwrap_or(content.len() - body_start);
        let body = &content[body_start..body_start + rel_end];
        let tokens = estimate_tokens(body);
        // ±20% 여유 (LLM 출력 편차 수용)
        let tolerant_lo = (*lo as f64 * 0.8) as usize;
        let tolerant_hi = (*hi as f64 * 1.2) as usize;
        if tokens < tolerant_lo || tokens > tolerant_hi {
            offending.push(((*header).to_string(), tokens));
        }
    }
    IdentitySectionValidation {
        has_all_sections: missing.is_empty(),
        missing,
        budget_ok: offending.is_empty(),
        offending_sections: offending,
    }
}

// ─── Frontmatter ────────────────────────────────────────────────────────────

/// 분석 결과 앞에 붙일 YAML frontmatter. ContextPack 주입 시 `strip_frontmatter`
/// 로 제거해 토큰 낭비 방지.
pub fn build_frontmatter(
    project_key: &str,
    since: i64,
    until: i64,
    artifact_refs: &[&str],
    supersedes: Option<&str>,
) -> String {
    let refs_csv = artifact_refs.join(",");
    let sup = supersedes.unwrap_or("");
    format!(
        "---\nproject_key: {}\nperiod_start: {}\nperiod_end: {}\nartifact_refs: [{}]\nsupersedes: {}\n---\n\n",
        project_key, since, until, refs_csv, sup
    )
}

/// ContextPack 주입 전 frontmatter 제거. `---` 블록이 content 시작부에 있으면 2
/// 번째 `---` 직후까지 잘라낸다. 없으면 원문 그대로.
pub fn strip_frontmatter(content: &str) -> &str {
    if !content.starts_with("---") {
        return content;
    }
    // 첫 줄의 --- 이후 두 번째 --- 를 찾는다
    let after_first = &content[3..];
    if let Some(idx) = after_first.find("\n---") {
        let body_start = 3 + idx + 4; // "\n---" (4 chars)
        content[body_start..].trim_start_matches(|c: char| c == '\n' || c.is_whitespace())
    } else {
        content
    }
}

// ─── LLM 호출 (injection 가능) ─────────────────────────────────────────────

/// LLM 호출 경로 — 테스트는 Stub 으로 LLM 호출을 격리한다.
pub enum InvokeAnalyzer {
    /// 실제 claude CLI 호출. CLI-first 원칙.
    Real { model: Option<String> },
    /// 테스트용 고정 응답.
    #[cfg(test)]
    Stub(String),
    /// 테스트용 — 1회차 실패 2회차 성공 시뮬레이션.
    #[cfg(test)]
    StubSequence { responses: std::sync::Mutex<Vec<String>> },
}

impl InvokeAnalyzer {
    pub fn call(&self, prompt: &str) -> Result<String, AppError> {
        match self {
            Self::Real { model } => {
                let out = run_claude(RunInput {
                    prompt: prompt.to_string(),
                    model: model.clone(),
                    system_prompt: None,
                    resume_token: None,
                    project_path: None,
                    image_paths: vec![],
                })?;
                Ok(out.content)
            }
            #[cfg(test)]
            Self::Stub(s) => Ok(s.clone()),
            #[cfg(test)]
            Self::StubSequence { responses } => {
                let mut guard = responses.lock().map_err(|_| AppError::Lock)?;
                if guard.is_empty() {
                    return Err(AppError::Agent("StubSequence exhausted".into()));
                }
                Ok(guard.remove(0))
            }
        }
    }
}

// ─── Orchestration ──────────────────────────────────────────────────────────

/// 분석 실행 결과 — artifact id + 검증 요약.
pub struct IdentityAnalysisOutcome {
    pub artifact_id: String,
    pub regenerated: bool,
}

/// 분석 파이프라인 core. Sync — worker 의 spawn_blocking 블록 안에서 호출한다.
/// 재시도는 1회 (최초 1 + 재생성 1 = 총 2회 LLM 호출).
///
/// steps:
/// 1. eligible artifacts + period 수집
/// 2. prompt build + LLM 호출
/// 3. 섹션/예산 검증
/// 4. 실패 시 hint 추가해 1회 재생성
/// 5. frontmatter 부착 + `create_identity_summary` 저장
/// 6. 입력 artifact 를 status='analyzed' 로 mark (선택)
pub fn run_identity_analysis_inner(
    conn: &Connection,
    project_key: &str,
    invoker: &InvokeAnalyzer,
) -> Result<IdentityAnalysisOutcome, AppError> {
    // 1. period + inputs
    let since_ts = last_summary_ts(conn, project_key)?;
    let until_ts = crate::db::migrations::now_epoch_ms();
    let inputs = list_eligible_artifacts(conn, project_key, since_ts, until_ts)?;
    if inputs.is_empty() {
        return Err(AppError::BadRequest(
            "identity_analysis: no eligible artifacts in window".into(),
        ));
    }
    let done_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM plans p JOIN conversations c ON p.conversation_id = c.id \
             WHERE p.status='done' AND c.project_key = ?1",
            [project_key],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // 2. prompt + first LLM call
    let prompt = build_prompt(project_key, since_ts, until_ts, done_count, &inputs);
    let first = invoker.call(&prompt)?;

    // 3. validate
    let validation = validate_identity_summary_sections(&first);
    let (final_content, regenerated) = if validation.is_ok() {
        (first, false)
    } else {
        // 4. retry once with explicit hint
        let hint = format!(
            "\n\nPrevious output did not satisfy requirements: missing_sections={:?}, offending={:?}. Regenerate strictly.",
            validation.missing, validation.offending_sections
        );
        let retry_prompt = format!("{}{}", prompt, hint);
        let retry = invoker.call(&retry_prompt)?;
        let retry_validation = validate_identity_summary_sections(&retry);
        if !retry_validation.is_ok() {
            return Err(AppError::Agent(format!(
                "identity_analysis: validation failed after retry (missing={:?})",
                retry_validation.missing
            )));
        }
        (retry, true)
    };

    // 5. frontmatter + persist
    let refs: Vec<&str> = inputs.iter().map(|a| a.id.as_str()).collect();
    let previous = latest_summary_id(conn, project_key)?;
    let frontmatter = build_frontmatter(project_key, since_ts, until_ts, &refs, previous.as_deref());
    let full_content = format!("{}{}", frontmatter, final_content);

    let title = format!("Identity — {} ({} artifacts)", project_key, inputs.len());
    let artifact_id = crate::commands::artifacts::create_identity_summary(conn, project_key, &title, &full_content)?;

    // 6. mark inputs analyzed (best-effort)
    for input in &inputs {
        let _ = conn.execute(
            "UPDATE artifacts SET status = 'analyzed', updated_at = ?1 WHERE id = ?2 AND status = 'draft'",
            params![crate::db::migrations::now_epoch_ms(), input.id],
        );
    }

    Ok(IdentityAnalysisOutcome {
        artifact_id,
        regenerated,
    })
}

// ─── Helpers — period / inputs ─────────────────────────────────────────────

fn last_summary_ts(conn: &Connection, project_key: &str) -> Result<i64, AppError> {
    let ts: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(a.created_at), 0) FROM artifacts a \
             JOIN conversations c ON a.conversation_id = c.id \
             WHERE a.type='identity_summary' AND c.project_key = ?1",
            [project_key],
            |r| r.get(0),
        )
        .unwrap_or(0);
    Ok(ts)
}

fn latest_summary_id(conn: &Connection, project_key: &str) -> Result<Option<String>, AppError> {
    let id: Option<String> = conn
        .query_row(
            "SELECT a.id FROM artifacts a \
             JOIN conversations c ON a.conversation_id = c.id \
             WHERE a.type='identity_summary' AND c.project_key = ?1 \
             ORDER BY a.created_at DESC LIMIT 1",
            [project_key],
            |r| r.get(0),
        )
        .ok();
    Ok(id)
}

fn list_eligible_artifacts(
    conn: &Connection,
    project_key: &str,
    since_ts: i64,
    until_ts: i64,
) -> Result<Vec<Artifact>, AppError> {
    let placeholders = ARTIFACT_TYPE_ORDER
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 4))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT a.id, a.conversation_id, a.branch_id, a.subtask_id, a.plan_id, a.type, a.title, a.content, a.status, a.created_at, a.updated_at \
         FROM artifacts a JOIN conversations c ON a.conversation_id = c.id \
         WHERE c.project_key = ?1 AND a.created_at > ?2 AND a.created_at <= ?3 \
           AND a.type IN ({}) \
         ORDER BY a.created_at ASC",
        placeholders
    );
    let mut params_vec: Vec<&dyn rusqlite::ToSql> = vec![&project_key, &since_ts, &until_ts];
    for k in ARTIFACT_TYPE_ORDER {
        params_vec.push(k);
    }
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params_vec.iter()), |row| {
            Ok(Artifact {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                branch_id: row.get(2)?,
                subtask_id: row.get(3)?,
                plan_id: row.get(4)?,
                artifact_type: row.get(5)?,
                title: row.get(6)?,
                content: row.get(7)?,
                status: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str, typ: &str, title: &str, content: &str, ts: i64) -> Artifact {
        Artifact {
            id: id.into(),
            conversation_id: Some("c1".into()),
            branch_id: None,
            subtask_id: None,
            plan_id: None,
            artifact_type: typ.into(),
            title: title.into(),
            content: content.into(),
            status: "draft".into(),
            created_at: ts,
            updated_at: ts,
        }
    }

    #[test]
    fn prompt_template_contains_required_rules() {
        assert!(IDENTITY_PROMPT_TEMPLATE.contains("Do not narrate"));
        assert!(IDENTITY_PROMPT_TEMPLATE.contains("Total budget: 1,350 ~ 2,050 tokens"));
        assert!(IDENTITY_PROMPT_TEMPLATE.contains("### Project identity"));
        assert!(IDENTITY_PROMPT_TEMPLATE.contains("### Do / Avoid"));
    }

    #[test]
    fn serialize_groups_by_type_in_fixed_order() {
        let artifacts = vec![
            artifact("a1", "finding_failure", "f1", "{}", 30),
            artifact("a2", "decision", "d1", "{}", 10),
            artifact("a3", "decision", "d2", "{}", 20),
            artifact("a4", "workflow_milestone", "m1", "{}", 40),
        ];
        let out = serialize_artifacts_for_prompt(&artifacts);
        let decision_idx = out.find("### decision").unwrap();
        let failure_idx = out.find("### finding_failure").unwrap();
        let milestone_idx = out.find("### workflow_milestone").unwrap();
        // order: decision < finding_failure < workflow_milestone (per ARTIFACT_TYPE_ORDER)
        assert!(decision_idx < failure_idx);
        assert!(failure_idx < milestone_idx);
        // 정렬 확인 — d1 (ts=10) 이 d2 (ts=20) 앞
        let d1_idx = out.find("[a2]").unwrap();
        let d2_idx = out.find("[a3]").unwrap();
        assert!(d1_idx < d2_idx);
    }

    #[test]
    fn summarize_content_caps_length() {
        let long = "a".repeat(500);
        let summary = summarize_content(&long, 50);
        assert!(summary.len() <= 53); // 50 + "..."
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn summarize_content_noop_when_short() {
        let short = "hello";
        assert_eq!(summarize_content(short, 100), "hello");
    }

    fn valid_summary_body() -> String {
        let section_body = "word ".repeat(200);
        // section 당 token 추정 = 200 * 1.3 = 260. 각 section 의 예산 범위에 들어오도록
        // 섹션별로 단어 수 조정.
        format!(
            "### Project identity\n{}\n\n### User working preference\n{}\n\n### Agent operating preference\n{}\n\n### Recent inflection points\n{}\n\n### Do / Avoid\n{}\n",
            "word ".repeat(150),  // ~195 tokens (150-250 OK)
            "word ".repeat(300),  // ~390 tokens (300-500 OK)
            "word ".repeat(300),  // ~390 tokens (300-500 OK)
            section_body,         // ~260 tokens (300-450 OK with tolerance)
            "word ".repeat(200),  // ~260 tokens (200-300 OK)
        )
    }

    #[test]
    fn validate_accepts_well_formed_summary() {
        let body = valid_summary_body();
        let v = validate_identity_summary_sections(&body);
        assert!(v.has_all_sections, "missing={:?}", v.missing);
        assert!(v.budget_ok, "offending={:?}", v.offending_sections);
        assert!(v.is_ok());
    }

    #[test]
    fn validate_detects_missing_sections() {
        let partial = "### Project identity\n\
                       Some content.\n\n\
                       ### User working preference\n\
                       More content.\n";
        let v = validate_identity_summary_sections(partial);
        assert!(!v.has_all_sections);
        assert!(v.missing.contains(&"### Agent operating preference".to_string()));
        assert!(v.missing.contains(&"### Recent inflection points".to_string()));
        assert!(v.missing.contains(&"### Do / Avoid".to_string()));
    }

    #[test]
    fn validate_detects_budget_overflow() {
        let mut content = String::new();
        for (header, _) in REQUIRED_SECTIONS {
            content.push_str(header);
            content.push('\n');
            // 각 섹션 매우 짧게 (under budget)
            content.push_str("one\n\n");
        }
        let v = validate_identity_summary_sections(&content);
        assert!(v.has_all_sections);
        assert!(!v.budget_ok, "너무 짧으면 offending 이어야");
    }

    #[test]
    fn frontmatter_builds_expected_yaml() {
        let fm = build_frontmatter("proj-x", 100, 200, &["a1", "a2"], Some("prev-id"));
        assert!(fm.starts_with("---\n"));
        assert!(fm.contains("project_key: proj-x"));
        assert!(fm.contains("period_start: 100"));
        assert!(fm.contains("period_end: 200"));
        assert!(fm.contains("artifact_refs: [a1,a2]"));
        assert!(fm.contains("supersedes: prev-id"));
    }

    #[test]
    fn strip_frontmatter_removes_yaml_block() {
        let full = "---\nproject_key: p\nperiod_start: 0\n---\n\n### Project identity\nbody";
        let stripped = strip_frontmatter(full);
        assert!(stripped.starts_with("### Project identity"));
    }

    #[test]
    fn strip_frontmatter_noop_when_no_yaml() {
        let plain = "### Project identity\nbody";
        assert_eq!(strip_frontmatter(plain), plain);
    }

    // run_identity_analysis_inner 의 retry 로직은 DB 연동이 필요 — 별도 integration
    // 테스트로 상위 모듈 (commands) 쪽에서 테스트. 본 파일에서는 Stub invoker 의
    // 동작만 단위 테스트.

    #[test]
    fn stub_sequence_returns_responses_in_order() {
        let inv = InvokeAnalyzer::StubSequence {
            responses: std::sync::Mutex::new(vec!["first".into(), "second".into()]),
        };
        assert_eq!(inv.call("x").unwrap(), "first");
        assert_eq!(inv.call("x").unwrap(), "second");
        assert!(inv.call("x").is_err());
    }
}
