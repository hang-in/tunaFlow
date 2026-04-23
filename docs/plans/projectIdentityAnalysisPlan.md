---
title: Project Identity Analysis — Artifacts 기반 "Karma" 파이프라인
status: planned
priority: P1
created_at: 2026-04-23
related:
  - src-tauri/src/commands/artifacts.rs                                       # CRUD 이미 완료
  - src-tauri/src/db/schema.rs                                                # artifacts 테이블 (L250)
  - src-tauri/src/commands/insight_extract.rs                                 # 기존 분석 파이프라인 참조
  - src/lib/insightOrchestration.ts                                           # 입력 discipline 참조 패턴
  - src-tauri/src/commands/agents_helpers/send_common/prompt_assembly.rs      # ContextPack selector 확장 대상
  - docs/plans/metaAgentPlan.md                                               # P0 — trigger + orchestration 주체
  - docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md                        # 기존 memory 계층 (중복 아님, source priority 관계)
  - docs/plans/userWorldviewInjectionPlan.md                                  # subtask-01 보유. 02~04 는 본 plan 으로 이관
  - docs/reference/tokenPolicyReference.md                                    # 품질 우선 철학
triggered_by:
  - 2026-04-23 사용자 설계 리마인드: "Artifacts 의 원 의도 = 사용자 결정 + agent finding 누적 → 비정기 분석 → 사용자 취향 + agent 성향 → 프로젝트 정체성"
  - Codex 자문 답변 (2026-04-23) — verdict: adjust, artifact type taxonomy + trigger guard 확정 후 진행 권고
supersedes_sections_of:
  - userWorldviewInjectionPlan.md (subtask-02: preference_events/snapshots, subtask-03: stance-conflict, subtask-04: background job)
---

# Project Identity Analysis — Artifacts → 정체성 파이프라인

> tunaFlow 의 **"Karma 시스템"** 엔지니어링 번역. 사용자 철학 ("죽음=Resume, 연기법, 업") 의 구체화 경로.
>
> 핵심 가설: **Artifacts 에 이미 누적되고 있는 "사용자 결정 + agent finding"** 을 **정기 분석** 하면, 사용자 취향 / agent 성향 / 프로젝트 정체성을 추출할 수 있다. 이 정체성 산출물이 ContextPack 에 주입되면 agent 의 매 응답이 프로젝트 누적 맥락에 더 부합한다.

---

## TL;DR for Developer

1. **Artifact 자동 생성 지점 보강** — 워크플로우의 6 타입 (`decision` / `review_outcome` / `rework_reason` / `finding_success` / `finding_failure` / `workflow_milestone`) 을 시점별로 자동 insert. 자유 감정 추론 / 대화 분석 금지 (surveillance 방지).
2. **metaAgent 가 trigger 감시** — `plans WHERE status='done' AND project_key=?` count 가 3의 배수 도달 **AND** 이전 `identity_summary` 이후 누적된 eligible artifacts ≥ 10 일 때만 analysis job enqueue.
3. **분석 에이전트가 `identity_summary` 생성** — 고정 섹션 + 토큰 예산 템플릿으로. 총 1,350~2,050 tokens. 프롬프트에 "Do not narrate. Prefer bullets over prose." 명시.
4. **`identity_summary` 는 artifacts 테이블 재사용** (신규 memory 계층 X). `type='identity_summary'`, `content` = 섹션형 markdown, `period='YYYY-MM'` 메타.
5. **ContextPack selector 확장** — `identity_summary` 의 최신 본문을 `worldview` 섹션 옆에 주입. 상세 input artifacts 는 agent tool-request on-demand.
6. **Insight 탭 "정체성 뷰"** — 최신 summary + 변곡점 timeline. 사용자 자기 관찰 도구.

구현 순서: 01 → 02 → 03 → 04. 01 없이는 02 가 돌아도 input 빈곤 (fail mode a). 03 이 안정적으로 섹션 준수한 후에 04 가 UX 가치.

**하지 말 것**:
- 자동 감정 추론 / 자유 대화 분석 (surveillance)
- `preference_events` / `preference_snapshots` 별도 테이블 (artifacts 재사용이 설계 의도)
- identity_summary 를 500~800 tokens 로 극단 압축 (Token Policy 위배 — 품질 우선)
- 새 memory 계층 신설 (compressed_memory / insight_extract 와 "source priority" 로 병존)
- time-based cron (plan 활동량과 무관한 trigger 는 idle 프로젝트에서 낭비)

---

## Specification

### 1. Artifact 자동 생성 — 6 타입

모든 자동 생성은 **워크플로우 이벤트 기반** (사용자 action 또는 확실한 agent 상태 전이). 대화 감정 추론 금지.

| type | 생성 시점 | 의미 | `content` 구조 |
|---|---|---|---|
| `decision` | 사용자가 Plan 승인 / 엔진 전환 / 경로 선택 | 의도 변곡점 | `{ what, rationale?, previous_stance? }` |
| `review_outcome` | Review verdict 완료 (pass/fail/escalate) | 품질 스냅샷 | `{ verdict, rubric{4D}, findings_count, failed_subtask_ids }` |
| `rework_reason` | Rework 진입 | 실패 카테고리 | `{ cycle, findings[], root_cause_hint }` |
| `finding_success` | Dev 가 subtask 를 scope 내로 완료 | agent 효율 신호 | `{ subtask_id, duration_ms, agent_engine, notes? }` |
| `finding_failure` | Dev 응답이 plan spec 벗어남 / blocker 발생 | agent 한계 신호 | `{ subtask_id, failure_kind, agent_engine, evidence_msg_id }` |
| `workflow_milestone` | Plan 완료 / PR 머지 / 릴리스 | 기준점 | `{ milestone_kind, plan_id, summary }` |

**필드 공통**:
- `artifacts.type` 위 6개 중 하나
- `artifacts.title` = 사람이 읽을 1-line 요약
- `artifacts.content` = JSON 문자열 (해석 유연성)
- `artifacts.conversation_id` / `branch_id` / `subtask_id` / `plan_id` = 연결 맥락
- `artifacts.status` = `'draft'` 기본 → 분석기가 `'analyzed'` 로 mark 가능

### 2. Trigger guard — 2 조건 AND

`src-tauri/src/commands/meta_agent.rs` (또는 metaAgent 가 구현될 모듈) 에서:

```rust
pub fn should_trigger_identity_analysis(
    conn: &Connection, project_key: &str,
) -> Result<bool, AppError> {
    // 조건 A: plan done count 가 3의 배수
    let done_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM plans WHERE status='done' AND project_key=?1",
        [project_key], |r| r.get(0),
    )?;
    if done_count == 0 || done_count % 3 != 0 { return Ok(false); }

    // 조건 B: 이전 identity_summary 이후 누적된 eligible artifacts >= 10
    let last_summary_at: i64 = conn.query_row(
        "SELECT COALESCE(MAX(created_at), 0) FROM artifacts
          WHERE type='identity_summary'
            AND (content_json_extract_project_key(content) = ?1
                 OR conversation_id IN (SELECT id FROM conversations WHERE project_key=?1))",
        [project_key], |r| r.get(0),
    )?;
    let eligible_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM artifacts
          WHERE type IN ('decision','review_outcome','rework_reason',
                         'finding_success','finding_failure','workflow_milestone')
            AND created_at > ?1
            AND conversation_id IN (SELECT id FROM conversations WHERE project_key=?2)",
        params![last_summary_at, project_key], |r| r.get(0),
    )?;
    if eligible_count < 10 { return Ok(false); }

    Ok(true)
}
```

**최초 기본값**: threshold = 10. Settings 에 `tunaflow.identity_analysis.min_artifacts` 로 노출 → 사용자 튜닝 가능. 초기 운영 3~6 개월 데이터 수집 후 default 재조정.

### 3. 분석 에이전트 prompt template — 고정 섹션 + 예산

```
# Project Identity Synthesis Prompt

You are synthesizing project identity from the last 3 completed plans' artifacts.

## Input
- Project: <project_key>
- Period: <since_ts> ~ <until_ts>
- Artifacts (filtered to 6 workflow-derived types, sorted by type then time):
  <ARTIFACTS_SERIALIZED>

## Output requirements
Produce a markdown document with EXACTLY these sections and token budgets.
Do not narrate. Write terse operational guidance. Prefer bullets over prose.
Every line must be reusable as future instruction to an agent.

### Project identity  (150-250 tokens)
- Nature / stage / primary users

### User working preference  (300-500 tokens)
- Decision patterns observed
- Preferred constraints / styles
- Explicit avoid-list

### Agent operating preference  (300-500 tokens)
- Which engine succeeds at which task category
- Common failure modes per engine
- Recommended delegation pattern

### Recent inflection points  (3 items, 100-150 tokens each)
For each: what changed, why (evidence artifact id), when.

### Do / Avoid  (200-300 tokens)
- Bullets. No prose.
```

**프롬프트 INV**:
- 단일 섹션 누락 시 재생성
- 섹션 예산 ±20% 초과 시 재생성 요청 (최대 2 라운드)
- 모든 bullet 은 향후 agent instruction 으로 재사용 가능한 형태 (운영 규약 톤)

### 4. `identity_summary` artifact 저장

기존 `create_artifact` 재사용:

```rust
create_artifact(CreateArtifactInput {
    conversation_id: None,              // 프로젝트 전역 (conversation 무관)
    branch_id: None,
    subtask_id: None,
    plan_id: None,
    artifact_type: "identity_summary".into(),
    title: format!("Identity — {} ({}~{})", project_key, since, until),
    content: <markdown_output>,         // 섹션형 markdown 본문
    status: "final".into(),
})
```

**메타 저장**: `content` 상단에 frontmatter 로 `project_key` / `period_start` / `period_end` / `artifact_refs: [<id>...]` 주입. 이전 summary 와의 diff 는 frontmatter 에 `supersedes: <prev_id>` 로 연결.

### 5. ContextPack selector 확장

`src-tauri/src/commands/agents_helpers/send_common/prompt_assembly.rs` 에:

```rust
// 기존 worldview 주입 경로 옆에 identity_summary 추가
let identity_fragment = fetch_latest_identity_summary(data.project_key.as_deref())
    .map(|a| a.content)                 // 본문 전체 (1,350~2,050 tokens 범위)
    .filter(|c| !c.trim().is_empty());

if let Some(s) = identity_fragment {
    sections.push(("project_identity", s));   // worldview 와 identity 사이 또는 identity 뒤
}
```

**위치 규칙** (Session Continuity INV 와 정합):
```
[project] [platform] [agent-role] ...
[worldview]           ← 사용자 수동 stance (subtask-01)
[project_identity]    ← 분석 생성 정체성 (본 plan)
[identity_fragment]   ← agent 역할 identity
[skills] [recent_context] ...
[user_prompt]
```

worldview 는 사용자가 직접 쓴 철학, project_identity 는 분석 추출 결과. 두 문서가 충돌하면 worldview 가 우선 — 프롬프트 프리픽스에 "User-authored worldview takes priority over analysis-derived identity on conflict." 한 줄 명시.

**상세 input artifacts 는 on-demand**: identity_summary 본문에 `artifact_refs` 나열 → agent 가 필요 시 tool-request `fetch_artifact` 로 조회.

### 6. Insight 탭 "정체성 뷰" UI

`src/components/tunaflow/context-panel/InsightPanel.tsx` (또는 동등) 에 새 서브뷰:

- **최신 identity_summary** — 6 섹션 접힌 상태로 각각 expand
- **변곡점 timeline** — `inflection_points` 섹션 파싱해 시간순 타임라인 카드
- **이전 summary 와 diff** — `supersedes` 체인 따라가기 버튼
- **수동 분석 트리거** — "지금 분석 시작" 버튼 (threshold 미달이어도 강제 실행)

UI 는 읽기 전용 + 1 action 버튼만. 편집 금지 (분석 결과 훼손 방지).

---

## Invariants

- **[INV-1]** 자동 artifact 생성은 명시된 **6 타입 (decision / review_outcome / rework_reason / finding_success / finding_failure / workflow_milestone) 한정**. 대화 감정 추론 / 자유 surveillance / user behavior mining 은 구현하지 않는다. **이유**: Codex Q1 지적 — taxonomy 가 넓어지면 personalization 이 아닌 surveillance 로 인식. 사용자 신뢰 훼손. **검증**: `grep "create_artifact" src-tauri/src -r` 결과의 `type` 인자가 위 7개 (identity_summary 포함) 외 값을 생성하지 않음. integration test — "random user message 가 자동 artifact 를 만드는지" negative case.

- **[INV-2]** identity analysis trigger 는 **2 조건 AND** 로만 발동: (a) `plans done_count % 3 == 0` **그리고** (b) 이전 summary 이후 eligible artifacts ≥ 10 (Settings 로 튜닝 가능). pure count 또는 time-based cron 은 금지. **이유**: Codex Q4 지적 — pure count 는 큰/작은 plan weight 불균형, time-based 는 idle 프로젝트에서 낭비. **검증**: `should_trigger_identity_analysis` unit test — 두 조건 각각의 단독 통과/실패 케이스 + AND 조합 케이스.

- **[INV-3]** `identity_summary` 는 **고정 섹션 + 토큰 예산 템플릿** 을 강제한다. 섹션 누락 시 재생성. 섹션별 예산 ±20% 초과 시 최대 2 라운드까지 재요청. 프롬프트에는 "Do not narrate. Prefer bullets over prose. Every line must be reusable as future instruction." 가 포함된다. **이유**: Codex Q2/Q5 지적 — generic summary 방지는 출력 길이 제약보다 **입력 discipline + 출력 구조 강제** 가 본질. **검증**: identity_summary 생성 테스트 — 섹션 parser 가 5개 섹션 모두 감지 + 각 섹션 토큰 수가 예산 ±20% 범위.

- **[INV-4]** **새 memory 계층을 신설하지 않는다**. `artifact type='identity_summary'` + ContextPack selector 확장으로만 구현. `compressed_memory` (대화 continuity) / `insight_extract` (코드/테스트 분석) / `identity_summary` (협업 취향/성향) 는 **source priority 가 다른 3 층 병존** 관계. **이유**: Codex Q3 지적 + `longTermMemoryRoadmapPlan` 의 Structured Task Memory 관점. **검증**: migration 변경 없음 (schema.rs diff 0). 새 테이블 생성 PR 금지.

- **[INV-5]** ContextPack 주입은 `identity_summary` **본문 (1,350~2,050 tokens)** 까지만. 상세 input artifacts (analysis 재료) 는 주입하지 않고 `artifact_refs` 만 포함해 agent 가 필요 시 tool-request 로 조회. **이유**: Token Policy (`tokenPolicyReference.md`) 의 "중복 회피" — 본문과 원본을 동시 주입하면 doubling. **검증**: ContextPack assemble 후 `fetch_latest_identity_summary` 가 본문 1건만 반환, `list_artifacts(type IN 6types)` 호출은 없음.

- **[INV-6]** **metaAgent** 는 trigger 감시 + prompt assembly + artifact creation 요청 범위. 실제 요약 생성 LLM 호출은 **dedicated persona 로 분리 가능** (옵션, 본 plan 범위 안에서는 metaAgent 단독 구현도 허용). **이유**: Codex Q7 + metaAgent plan 의 "프로세스 관리자, 제안만" 역할 정의. **검증**: metaAgent 코드가 요약 생성 LLM 호출 로직을 직접 포함하지 않음 (persona 분리 구현 시), 또는 metaAgent 내부 함수로 격리 (단독 구현 시).

---

## Rationale (reviewer-only)

### Codex 자문 반영 (2026-04-23)

Codex 가 adjust verdict 를 내리며 강조한 3 조건:

1. **artifact type taxonomy 먼저 확정** → 본 plan 의 6 타입으로 고정. surveillance 경계.
2. **trigger guard (volume threshold)** → threshold 10 초기값 (사용자 판단), Settings 로 튜닝.
3. **입력 discipline** → 프롬프트 템플릿에서 type별 정렬 + 좁은 범위 + 구조 강제.

추가로 Codex Q5 의 fail mode a+b+c 조합 (artifact 빈약 → generic summary → agent 응답 영향 미미) 에 대해:
- subtask-01 이 artifact 자동 생성 경로를 보강 (a 방어)
- INV-2 volume threshold 가 generic 직접 방지 (b 방어)
- INV-5 본문 주입 + on-demand 상세 조회가 agent 활용 경로 최적화 (c 방어)

### Karma 철학의 번역

사용자의 원 비유 ("죽음=Resume, 연기법, 업") 를 엔지니어링으로:

| 철학 개념 | 엔지니어링 매핑 |
|---|---|
| 업(業) 의 누적 | `artifacts` 6 타입 자동 생성 |
| 연기법 (인과) | `decision` → `rework_reason` → `finding_failure` 같은 사건 연쇄 |
| 세션 간 의도 전달 | `identity_summary` 를 ContextPack 주입 |
| Vipassana (자기 관찰) | Insight 탭 "정체성 뷰" |
| 구도 (성장) | 이전 summary 와 diff → 변곡점 조회 |

이 매핑은 tunaFlow 의 "2인 3각 협업 + 인간지능 주도" 원 취지와 정합. 자동화는 **기록 + 요약** 까지만, 판단은 사용자 몫.

### 대안 비교

| 대안 | 판정 | 사유 |
|---|---|---|
| 새 `preference_events` / `preference_snapshots` 테이블 (초기 userWorldview 설계) | 기각 | Artifacts 재사용으로 불필요. 중복 memory 계층. |
| stance-conflict marker + modal (초기 설계) | 기각 | LLM 자연 능력에 맡김. UX 침습 과다. |
| Time-based cron (월 1회) | 기각 | Idle 프로젝트에서 낭비, 활발한 프로젝트에서 부족 |
| Pure count trigger (plan 3개만) | 기각 | 큰/작은 plan weight 불균형 |
| compressed_memory 확장으로 대체 | 기각 | 대화 continuity 용도와 취향 분석 용도는 source priority 다름 |
| **채택** (6 타입 artifact + 2조건 trigger + 고정 섹션 template + ContextPack selector 확장) | ✅ | Codex adjust 방향 + 사용자 철학 + 기존 인프라 최대 재활용 |

### Open questions

1. **Q-1 threshold 튜닝**: 초기 10 이 적절한가? 3~6 개월 실운영 후 Settings 의 default 재조정 기준은 무엇인가? (예: 월 평균 분석 횟수 목표값, identity_summary 품질 rating 기반)

2. **Q-2 dedicated persona 분리 시점**: 초기 구현은 metaAgent 단독이 단순하나, 분석 품질이 낮으면 dedicated analyst persona (temperature 낮춤 + 섹션 template 엄격) 가 유리. 분리 판단 기준 (예: 섹션 예산 위반율 > X%)?

3. **Q-3 cold start**: 프로젝트 첫 analysis 시점 이전의 artifact 는 분석 대상이 없음 — 첫 summary 의 품질이 낮을 가능성. 최초 N개 artifact 까지 skip 전략 유효한가?

4. **Q-4 에이전트 응답 반영 측정**: identity_summary 가 ContextPack 에 들어간 이후 agent 응답 품질이 실제로 향상됐는지 어떻게 측정? (예: 같은 conv 에서 ON/OFF A/B, 사용자 재생성 요청율)

5. **Q-5 다국어**: identity_summary 의 언어는? 프로젝트 기본 언어 (한국어) vs 영어 고정 (i18nPlan 의 "프롬프트 영어 통일" 원칙). 본 plan 은 **영어 고정** 권장 (LLM 성능 + 토큰 효율) — 단 Insight 탭 UI 는 i18n 적용.

---

## Subtask 구조

| # | 파일 | 범위 | 의존 |
|---|---|---|---|
| 01 | [-task-01.md](./projectIdentityAnalysisPlan-task-01.md) | Artifact 자동 생성 지점 보강 (6 타입 각각의 trigger 경로) | — |
| 02 | [-task-02.md](./projectIdentityAnalysisPlan-task-02.md) | metaAgent trigger (2 조건) + analysis job orchestration | 01 |
| 03 | [-task-03.md](./projectIdentityAnalysisPlan-task-03.md) | 분석 prompt template + identity_summary artifact + ContextPack selector | 02 |
| 04 | [-task-04.md](./projectIdentityAnalysisPlan-task-04.md) | Insight 탭 "정체성 뷰" UI (읽기 전용 + 수동 트리거 버튼) | 03 |

4 subtask. 순차 의존. 01 이 가장 큰 (워크플로우 여러 지점 touch), 04 가 가장 작음.

---

## 관련 문서

- Karma 철학 근거: 본 세션 2026-04-22 사용자 철학적 프롬프트 (Identity/Interface/Continuity 3질문)
- Codex 자문 transcript: 본 세션 2026-04-23 Q1~Q7 답변
- Token Policy: `docs/reference/tokenPolicyReference.md` (품질 우선 + 중복 회피)
- userWorldview 축소: 본 plan 이 `userWorldviewInjectionPlan` 의 subtask-02~04 를 대체
- 기존 memory 계층: `longTermMemoryRoadmapPlan_2026-03-30.md` (Phase 1-3 완료)
- metaAgent 의존: `metaAgentPlan.md` (P0, 실제 구현 필요)
