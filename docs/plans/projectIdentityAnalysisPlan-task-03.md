# Subtask 03 — 분석 prompt template + `identity_summary` artifact + ContextPack selector

> 상위 plan: [projectIdentityAnalysisPlan.md](./projectIdentityAnalysisPlan.md)

## Changed files

- `src-tauri/src/agents/identity_analyzer.rs` (신규) — prompt assembly + LLM 호출 + 결과 파싱.
- `src-tauri/src/commands/artifacts.rs` — `create_identity_summary` 전용 helper (INV-1 bypass for analyzer).
- `src-tauri/src/commands/agents_helpers/send_common/prompt_assembly.rs` — ContextPack 에 `fetch_latest_identity_summary` 주입.
- `src-tauri/src/db/models.rs` — `IdentitySummary` section parser types.

## Change description

### 1. Prompt template (고정 섹션 + 예산)

```rust
// src-tauri/src/agents/identity_analyzer.rs

const IDENTITY_PROMPT_TEMPLATE: &str = r#"
You are synthesizing project identity from the last completed plans' artifacts.

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
```

### 2. Input artifact serialization

6 타입 각 block, type 별 정렬 (Codex Q2 의 "입력 discipline"):

```rust
fn serialize_artifacts_for_prompt(artifacts: &[Artifact]) -> String {
    let mut grouped: HashMap<&str, Vec<&Artifact>> = HashMap::new();
    for a in artifacts {
        grouped.entry(a.type_.as_str()).or_default().push(a);
    }
    let order = ["decision", "review_outcome", "rework_reason",
                 "finding_success", "finding_failure", "workflow_milestone"];
    let mut out = String::new();
    for t in &order {
        if let Some(items) = grouped.get(t) {
            out.push_str(&format!("### {} ({} items)\n", t, items.len()));
            let mut sorted: Vec<_> = items.iter().collect();
            sorted.sort_by_key(|a| a.created_at);
            for a in sorted {
                out.push_str(&format!(
                    "- [{}] {} @ {}: {}\n",
                    a.id, a.title,
                    fmt_ts(a.created_at),
                    summarize_content(&a.content, 100),  // content JSON 을 80-100 chars 로 축약
                ));
            }
            out.push('\n');
        }
    }
    out
}
```

### 3. LLM 호출 + 검증 루프

```rust
pub async fn run_identity_analysis(
    app: &AppHandle, state: &DbState,
    project_key: &str, job_id: &str,
) -> Result<String, AppError> {  // returns new identity_summary artifact id
    // 1. Collect input artifacts (since last summary)
    let (since_ts, until_ts) = resolve_period(state, project_key)?;
    let input_artifacts = list_eligible_artifacts(state, project_key, since_ts, until_ts)?;

    // 2. Build prompt
    let serialized = serialize_artifacts_for_prompt(&input_artifacts);
    let prompt = IDENTITY_PROMPT_TEMPLATE
        .replace("{project_key}", project_key)
        .replace("{since}", &fmt_ts(since_ts))
        .replace("{until}", &fmt_ts(until_ts))
        .replace("{done_plan_count}", &count_done_plans(state, project_key)?.to_string())
        .replace("{artifacts_serialized}", &serialized);

    // 3. Call LLM. Default = metaAgent's configured engine (Opus 권장 — 분석 품질 우선).
    //    temperature 낮게 (0.3), max_tokens = 2500 (여유).
    let response = call_llm_with_retry(&prompt, 2).await?;

    // 4. Validate sections & budgets
    let validation = validate_identity_summary_sections(&response);
    if !validation.is_ok() {
        // 1 회 재생성 시도 — "Regenerate. Previous output missed: {missing_sections}"
        let retry_prompt = format!("{}\n\nPrevious output missed: {:?}. Regenerate strictly.",
                                   prompt, validation.missing);
        let retry = call_llm_with_retry(&retry_prompt, 1).await?;
        if !validate_identity_summary_sections(&retry).is_ok() {
            return Err(AppError::Agent(
                "identity_analysis: section validation failed after retry".into(),
            ));
        }
        return finalize_artifact(state, project_key, &retry, input_artifacts).await;
    }

    finalize_artifact(state, project_key, &response, input_artifacts).await
}

async fn finalize_artifact(
    state: &DbState, project_key: &str, content: &str,
    inputs: Vec<Artifact>,
) -> Result<String, AppError> {
    // 5. Prefix with YAML frontmatter
    let refs: Vec<&str> = inputs.iter().map(|a| a.id.as_str()).collect();
    let frontmatter = format!(
        "---\nproject_key: {}\nperiod_start: {}\nperiod_end: {}\nartifact_refs: [{}]\nsupersedes: {}\n---\n\n",
        project_key, since, until, refs.join(","),
        latest_summary_id(state, project_key)?.unwrap_or_default(),
    );
    let full_content = format!("{}{}", frontmatter, content);

    // 6. create_identity_summary artifact
    let id = create_identity_summary(
        state, project_key,
        format!("Identity — {} ({})", project_key, fmt_period(since, until)),
        full_content,
    )?;

    // 7. Mark input artifacts as analyzed (optional)
    for inp in inputs {
        update_artifact_status(state, &inp.id, "analyzed").ok();
    }
    Ok(id)
}
```

### 4. `create_identity_summary` (analyzer 전용)

```rust
// src-tauri/src/commands/artifacts.rs
pub fn create_identity_summary(
    state: &DbState, project_key: &str,
    title: String, content: String,
) -> Result<String, AppError> {
    let id = format!("identity-{}-{}", project_key, now_epoch_ms());
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    conn.execute(
        "INSERT INTO artifacts (id, conversation_id, branch_id, subtask_id, plan_id, type, title, content, status, created_at, updated_at)
         VALUES (?1, NULL, NULL, NULL, NULL, 'identity_summary', ?2, ?3, 'final', ?4, ?4)",
        params![id, title, content, now_epoch_ms()],
    )?;
    Ok(id)
}
```

본 함수는 `create_identity_input_artifact` 의 INV-1 enforcement (IdentitySummary 거부) 를 우회하는 **분석 전용 경로**. `state.identity_analyzer_trusted=true` 같은 guard 를 주거나 단순히 호출 site 를 analyzer 모듈로 제한.

### 5. Section validation

```rust
// src-tauri/src/db/models.rs
pub struct IdentitySectionValidation {
    pub has_all_sections: bool,
    pub missing: Vec<&'static str>,
    pub budget_ok: bool,
    pub offending_sections: Vec<(&'static str, usize)>,   // (name, actual_tokens)
}

const REQUIRED_SECTIONS: &[(&str, (usize, usize))] = &[
    ("### Project identity", (150, 250)),
    ("### User working preference", (300, 500)),
    ("### Agent operating preference", (300, 500)),
    ("### Recent inflection points", (300, 450)),   // 3 × (100~150)
    ("### Do / Avoid", (200, 300)),
];

pub fn validate_identity_summary_sections(content: &str) -> IdentitySectionValidation {
    // 1. 각 section header 존재?
    // 2. 각 section 내용의 추정 token 수 (word count * 1.3 근사) 가 예산의 ±20% 범위?
    // 3. missing + offending 나열
}
```

### 6. ContextPack selector 확장

```rust
// src-tauri/src/commands/agents_helpers/send_common/prompt_assembly.rs
use crate::commands::artifacts::fetch_latest_identity_summary;

// assemble_prompt() 내부, worldview_fragment 삽입 직후:
let identity_fragment = data.project_key.as_deref()
    .and_then(|pk| fetch_latest_identity_summary(state, pk).ok())
    .map(|a| strip_frontmatter(&a.content).to_string());
if let Some(s) = identity_fragment {
    sections.push(("project_identity", s));
}
```

`fetch_latest_identity_summary`:
```rust
pub fn fetch_latest_identity_summary(
    state: &DbState, project_key: &str,
) -> Result<Artifact, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let row = conn.query_row(
        "SELECT * FROM artifacts
          WHERE type='identity_summary'
            AND content LIKE '%project_key: ' || ?1 || '%'
          ORDER BY created_at DESC LIMIT 1",
        [project_key], row_to_artifact,
    )?;
    Ok(row)
}
```

**LIKE 검색은 frontmatter 기반 임시 해결책**. 더 깔끔한 건 별도 메타 테이블 or artifacts 에 `project_key` 컬럼 추가 — 단 schema 변경 피하려 frontmatter + LIKE 로.

## Dependencies

depends_on: [02] — analysis job 이 enqueue 되어야 본 analyzer 가 실행됨.

## Verification

- `cargo test --lib agents::identity_analyzer`:
  - Prompt template 에 필수 키워드 포함 (Do not narrate / token budget)
  - `serialize_artifacts_for_prompt` — type 별 그룹 + 정렬 + summary 길이 제한
  - Section validation — 누락 섹션 / 예산 초과 / OK 케이스
  - Retry 로직 — 첫 응답 실패 시 1 회 재생성, 재실패 시 Err
- `cargo test --lib commands::artifacts::fetch_latest_identity_summary`:
  - project 별 최신 1건 반환
  - 없으면 Err (NotFound)
- `cargo test --lib commands::agents_helpers::send_common::prompt_assembly`:
  - identity_fragment 가 worldview 뒤, identity_fragment 앞 위치
  - 없으면 sections 에 미포함
- Manual E2E:
  1. 10+ eligible artifact 준비
  2. `trigger_identity_analysis_now(force=true)` 호출
  3. agent_jobs 상태 → running → done
  4. `list_artifacts(type='identity_summary')` 로 신규 artifact 확인
  5. 본문이 5 섹션 모두 + 예산 범위
  6. 다음 chat 요청에 ContextPack 으로 주입되는지 trace_log 확인

## Risks

- **LLM 섹션 미준수 빈도**: 실측해야 함. 1 회 재생성 후에도 fail 이 흔하면 prompt 강화 / 모델 상향 / temperature 하향 등 튜닝. 본 subtask 는 2 라운드까지만 retry, 이후 실패는 ERR 로 job 마감.
- **Token budget 추정 정확도**: `word_count * 1.3` 은 한글/영문 혼합에서 오차. tiktoken 직접 사용도 가능하나 의존성 추가. 초기는 휴리스틱, 후속 고도화.
- **frontmatter + LIKE 쿼리**: n 개 artifact 전체 scan. project 당 identity_summary 수는 월 수개 수준이라 성능 문제 없음. but scale 시 artifacts 테이블에 project_key 컬럼 추가 고려.
- **프롬프트 언어 선택**: 본 template 은 영어 (i18nPlan 방침 + LLM 성능). identity_summary content 도 영어 산출. Insight 탭 UI (subtask-04) 에서 한국어 사용자에게 표시할 때 auto-translate 추가 여부는 별도 논의.
- **Frontmatter 오염**: content 앞 frontmatter 가 ContextPack 에 주입되면 불필요 토큰. `strip_frontmatter` 함수로 주입 직전 제거.
- **Analyzer 의 권한 분리**: 현재는 metaAgent 단독 구현 허용 (INV-6). 향후 dedicated analyst persona 로 분리 시 본 `run_identity_analysis` 를 persona runtime 아래로 이동 + metaAgent 는 enqueue 까지만.
