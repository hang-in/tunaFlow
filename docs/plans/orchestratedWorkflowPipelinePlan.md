# Orchestrated Workflow Pipeline — Chat → Plan → Implement → Review

> Status: draft
> Created: 2026-03-31
> Inspired by: [Stavros — How I Write Software with LLMs](https://www.stavros.io/posts/how-i-write-software-with-llms/)

---

## 1. 목표

채팅에서 시작해 Plan 승격 → 승인/검토 루프 → Developer 구현 → Reviewer 검증까지 이어지는
**end-to-end 오케스트레이션 파이프라인**을 tunaFlow에 구현한다.

```
Chat (Architect)
  ↓  "Plan으로 승격하겠습니다"
Plan Tab (Approval Gate)
  ↓  승인 / 보류+검토(Branch) / 반려
Implementation (Developer, Branch)
  ↓  실행계획 보고 → 승인 → 코드 생성
Review (2 Reviewers, RT)
  ↓  코드 + 테스트 실행 → Plan 대조 → 판정
Done / Rework
```

---

## 2. 현재 인프라 vs 필요한 것

| 구성요소 | 현재 상태 | 필요한 작업 |
|----------|----------|------------|
| Chat → Plan 승격 | Plan 탭에서 수동 생성만 가능 | 에이전트 제안 → 자동 파싱 → Plan 등록 |
| Plan 승인 워크플로우 | status cycle (draft→active→done) | 승인/보류/검토 3-way gate + 검토 Branch |
| Plan → Developer 전달 | sendFollowup으로 수동 forward | 자동 Branch 생성 + 실행계획 보고 단계 |
| Developer 실행계획 보고 | 없음 | 구현 전 보고 → 승인 게이트 |
| Test 실행 | EvaluationPanel (에이전트 비교만) | 실제 `cargo test`/`vitest` 실행 |
| Review (코드+Plan 대조) | ReviewPanel (아티팩트 표시만) | 2-agent RT 리뷰 + 테스트 결과 포함 |
| Plan 상태 추적 | 기본 status만 | phase 필드 + 이력 로그 |

---

## 3. 데이터 모델 변경

### 3.1 plans 테이블 확장

```sql
-- v18 migration
ALTER TABLE plans ADD COLUMN phase TEXT NOT NULL DEFAULT 'drafting';
-- phase: "drafting" | "approval" | "implementation" | "review" | "done" | "rework"

ALTER TABLE plans ADD COLUMN architect_engine TEXT;
ALTER TABLE plans ADD COLUMN developer_engine TEXT;
ALTER TABLE plans ADD COLUMN reviewer_engines TEXT;  -- JSON: ["claude", "gemini"]

ALTER TABLE plans ADD COLUMN implementation_branch_id TEXT REFERENCES branches(id);
ALTER TABLE plans ADD COLUMN review_branch_id TEXT REFERENCES branches(id);
```

### 3.2 plan_events 테이블 (이력 로그)

```sql
CREATE TABLE plan_events (
    id            TEXT PRIMARY KEY,
    plan_id       TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    event_type    TEXT NOT NULL,
    -- "promoted" | "approved" | "held" | "review_requested" | "review_merged"
    -- | "impl_plan_submitted" | "impl_approved" | "impl_completed"
    -- | "review_passed" | "review_failed" | "rework_requested"
    actor         TEXT,           -- "user" | engine name
    detail        TEXT,           -- JSON: memo content, rejection reason, etc.
    created_at    INTEGER NOT NULL
);
CREATE INDEX idx_plan_events_plan_id ON plan_events(plan_id);
```

### 3.3 TypeScript 타입 추가

```typescript
type PlanPhase = "drafting" | "approval" | "implementation" | "review" | "done" | "rework";

interface Plan {
  // ... existing fields ...
  phase: PlanPhase;
  architectEngine?: string;
  developerEngine?: string;
  reviewerEngines?: string[];   // parsed from JSON
  implementationBranchId?: string;
  reviewBranchId?: string;
}

interface PlanEvent {
  id: string;
  planId: string;
  eventType: string;
  actor?: string;
  detail?: string;
  createdAt: number;
}
```

---

## 4. Phase 1: Chat → Plan 승격

### 4.1 에이전트 제안 메커니즘 (UX 추천)

**접근: Structured marker + UI surface**

에이전트 응답에 특정 마커 블록이 포함되면 프론트엔드가 감지하여 "Plan으로 승격" 버튼을 인라인 표시한다.

```markdown
<!-- tunaflow:plan-proposal -->
## Plan Proposal: {title}

### Description
{description}

### Expected Outcome
{expected_outcome}

### Subtasks
1. {subtask_1_title} — {subtask_1_details}
2. {subtask_2_title} — {subtask_2_details}
...

### Constraints
- {constraint_1}
- {constraint_2}

### Non-goals
- {non_goal_1}
<!-- /tunaflow:plan-proposal -->
```

**왜 이 방식인가:**

| 방식 | 장점 | 단점 |
|------|------|------|
| JSON 응답 파싱 | 정확한 구조 | 에이전트가 대화 흐름 중 JSON만 출력하기 어려움, UX 부자연스러움 |
| 자연어 후 수동 변환 | 유연 | 사용자 수작업 필요, 현재와 다를 바 없음 |
| **HTML 마커 + Markdown** | 대화 흐름 자연스러움 + 자동 파싱 가능 + 에이전트가 생성하기 쉬움 | 마커 규약 필요 |

### 4.2 프론트엔드 파싱 + UI

**MessageItem 내 PlanProposalCard:**

```
┌─────────────────────────────────────────┐
│ 📋 Plan Proposal: API 리팩토링          │
│                                         │
│ Description: REST API를 GraphQL로 전환   │
│ Expected Outcome: ...                   │
│                                         │
│ Subtasks:                               │
│ □ 스키마 정의                            │
│ □ resolver 구현                          │
│ □ 기존 엔드포인트 마이그레이션            │
│                                         │
│ Constraints: ...                        │
│ Non-goals: ...                          │
│                                         │
│ [✅ Plan으로 승격]  [🔄 수정 요청]  [❌ 무시] │
└─────────────────────────────────────────┘
```

- **Plan으로 승격**: plan-proposal 파싱 → `planApi.createPlan()` → Plan 탭에 등록 (phase="approval") → plan_events에 "promoted" 기록
- **수정 요청**: 사용자가 피드백 입력 → 에이전트에게 전달 → 재제안
- **무시**: 카드 닫기

### 4.3 RT를 통한 고도화

승격 전에 더 논의가 필요하면:
1. 사용자가 메시지에서 "RT 분기" 액션 → RT Branch 생성
2. 여러 에이전트가 plan proposal을 토론
3. RT 결과에서 최종 plan-proposal 마커 포함 → 승격

### 4.4 ContextPack 연동

Plan이 생성되면 `persona_fragment`에 Architect 역할 힌트를 포함:

```
당신은 이 프로젝트의 Architect입니다.
사용자의 요구사항을 분석하고, 구현 계획을 <!-- tunaflow:plan-proposal --> 형식으로 제안하세요.
제안 전에 충분히 질문하고 트레이드오프를 논의하세요.
```

이 힌트는 Architect persona에 내장하거나, 사용자가 "계획 세워줘" 같은 신호를 보낼 때 자동 주입.

---

## 5. Phase 2: Plan Approval Gate

### 5.1 Plan 탭 워크플로우 UI

Plan이 `phase="approval"`이 되면 Plan 탭에 3-way 게이트 표시:

```
┌─────────────────────────────────────────────┐
│ 📋 API 리팩토링          phase: Approval     │
│                                             │
│ [Subtasks list...]                          │
│                                             │
│ ┌─────────────────────────────────────────┐ │
│ │ [✅ 승인]  [⏸ 보류]  [🔍 검토 요청]     │ │
│ └─────────────────────────────────────────┘ │
│                                             │
│ Timeline:                                   │
│ • 03-31 14:20 — promoted from chat (claude) │
│ • 03-31 14:25 — review requested (user)     │
│ • 03-31 14:40 — review merged (branch b3)   │
│ • 03-31 14:45 — approved (user)             │
└─────────────────────────────────────────────┘
```

### 5.2 검토 요청 → Branch → 병합

**"검토 요청" 클릭 시:**

1. 사용자가 의견(메모) 작성 (텍스트 입력 or 기존 메모 첨부)
2. 자동으로 Review Branch 생성 (`plan.review_branch_id`)
3. Branch에 의견 + plan 내용이 initial context로 전달
4. 에이전트가 Branch에서 plan 수정안 토론
5. 사용자가 Branch 결과를 확인 → "Plan에 병합" 버튼
6. 병합: Branch 내 최종 plan-proposal을 파싱 → `replace_plan_subtasks()` 호출
7. plan_events에 "review_merged" 기록
8. phase 유지 (approval) → 사용자가 다시 승인/검토/보류 선택

### 5.3 승인 → Implementation 전환

**"승인" 클릭 시:**

1. `plan.phase = "implementation"`, `plan.status = "active"`
2. plan_events에 "approved" 기록
3. Developer engine 선택 다이얼로그 표시 (기본값: plan.developerEngine or Sonnet)
4. Implementation Branch 자동 생성 (`plan.implementation_branch_id`)
5. Phase 3으로 전환

---

## 6. Phase 3: Implementation (Developer)

### 6.1 실행계획 보고 (Pre-implementation Report)

Implementation Branch에 Developer 에이전트를 자동 호출:

**Prompt:**
```
당신은 Developer입니다. 아래 Plan을 구현해야 합니다.

{plan_content with subtasks}

코드를 작성하기 전에 먼저 실행 계획을 보고하세요:
1. 수정/생성할 파일 목록
2. 각 파일의 변경 내용 요약
3. 의존성 변경 여부
4. 예상 위험/주의사항

<!-- tunaflow:impl-plan --> 형식으로 보고하세요.
아직 코드를 작성하지 마세요.
```

### 6.2 실행계획 승인 게이트

Developer의 보고를 PlanCard 내에 인라인 표시:

```
┌─────────────────────────────────────────┐
│ Implementation Plan (from Developer)     │
│                                         │
│ Files to modify:                        │
│ • src/api/rest.ts → remove              │
│ • src/api/graphql.ts → create           │
│ • src/schema/ → create directory        │
│                                         │
│ [✅ 구현 시작]  [🔄 수정 요청]           │
└─────────────────────────────────────────┘
```

- **구현 시작**: Developer에게 "승인됨, 구현하세요" 후속 메시지 전송
- **수정 요청**: 피드백 → Developer Branch에서 재논의

### 6.3 구현 완료

Developer가 코드 작성 완료 → 응답에 `<!-- tunaflow:impl-complete -->` 마커 포함 시:

1. `plan.phase = "review"`
2. plan_events에 "impl_completed" 기록
3. Phase 4로 자동 전환

---

## 7. Phase 4: Review (2 Reviewers)

### 7.1 RT 기반 2-agent 리뷰

Review Branch 자동 생성 → RT 모드로 2명의 Reviewer 호출:

**Participants:**
```typescript
[
  { name: "Reviewer-A", engine: plan.reviewerEngines[0] || "claude", role: "reviewer" },
  { name: "Reviewer-B", engine: plan.reviewerEngines[1] || "gemini", role: "reviewer" },
]
```

**Prompt:**
```
당신은 코드 리뷰어입니다.

## Plan (원래 요구사항)
{plan_content}

## Implementation (Developer 구현 결과)
{implementation_branch_messages or diff}

## 리뷰 기준
1. Plan의 모든 subtask가 구현되었는가?
2. 코드 품질 (버그, 보안, 성능)
3. 테스트 커버리지

아래 테스트 결과를 참고하세요:
{test_results}

<!-- tunaflow:review-verdict -->
verdict: pass | fail | conditional
findings:
- {finding_1}
- {finding_2}
recommendations:
- {recommendation_1}
<!-- /tunaflow:review-verdict -->
```

### 7.2 테스트 실행

리뷰 시작 전 자동으로 테스트 실행:

1. 프로젝트 타입 감지 (`Cargo.toml` → Rust, `package.json` → Node)
2. 해당 테스트 러너 실행:
   - `cargo test --lib 2>&1`
   - `npx vitest run 2>&1`
3. 결과를 파싱하여 pass/fail/skip 카운트 추출
4. 테스트 결과를 Reviewer RT prompt에 포함
5. 결과를 `test-report` artifact로 저장

**새 Tauri command:**
```rust
#[tauri::command]
pub fn run_project_tests(
    project_path: String,
    test_type: Option<String>,  // auto-detect if None
) -> Result<TestRunResult, AppError>

pub struct TestRunResult {
    pub test_type: String,      // "cargo" | "vitest" | "jest" | ...
    pub passed: i32,
    pub failed: i32,
    pub skipped: i32,
    pub duration_ms: i64,
    pub output: String,         // raw stdout+stderr
    pub success: bool,
}
```

### 7.3 리뷰 판정

Reviewer RT 완료 후 verdict 파싱:

- **pass (둘 다)**: `plan.phase = "done"`, plan_events에 "review_passed"
- **fail (하나라도)**: `plan.phase = "rework"`, plan_events에 "review_failed" + findings
- **conditional**: 사용자에게 판단 위임 (PlanCard에 findings 표시)

### 7.4 Rework 루프

`phase = "rework"` 시:
1. Review findings를 Implementation Branch에 전달
2. Developer가 수정
3. 다시 Phase 4 (Review) 진입
4. 반복

---

## 8. 구현 페이즈

### Phase A: 기반 인프라 (DB + 타입 + API)
1. v18 migration: plans 테이블 확장 + plan_events 테이블
2. TypeScript 타입 확장 (PlanPhase, PlanEvent)
3. Tauri commands: plan phase 업데이트, plan_events CRUD
4. Frontend API wrapper 추가

### Phase B: Chat → Plan 승격
1. plan-proposal 마커 파서 (MessageItem 내)
2. PlanProposalCard 컴포넌트
3. 승격 액션 (파싱 → createPlan → phase="approval")
4. Architect persona 힌트 (선택적)

### Phase C: Plan Approval Gate
1. PlanCard에 3-way gate UI (승인/보류/검토)
2. 검토 Branch 자동 생성 + 의견 전달
3. Branch → Plan 병합 (replace_plan_subtasks)
4. Timeline (plan_events) 표시

### Phase D: Implementation (Developer)
1. Implementation Branch 자동 생성
2. Pre-implementation report 자동 요청
3. impl-plan 마커 파싱 + 승인 게이트 UI
4. impl-complete 감지 → phase 전환

### Phase E: Review + Test Execution
1. `run_project_tests` Tauri command (Rust)
2. 테스트 러너 감지 + 실행 + 결과 파싱
3. Review RT 자동 실행 (2-agent)
4. review-verdict 파싱 → pass/fail/rework 분기
5. Rework 루프

---

## 9. 마커 규약 요약

| 마커 | 생성자 | 소비자 | 용도 |
|------|--------|--------|------|
| `<!-- tunaflow:plan-proposal -->` | Architect | MessageItem | Plan 제안 감지 + 승격 |
| `<!-- tunaflow:impl-plan -->` | Developer | PlanCard | 실행계획 보고 감지 |
| `<!-- tunaflow:impl-complete -->` | Developer | Pipeline | 구현 완료 감지 |
| `<!-- tunaflow:review-verdict -->` | Reviewer | Pipeline | 리뷰 판정 감지 |

---

## 10. 수정 범위 예측

| 계층 | 파일 | 변경 |
|------|------|------|
| DB | `migrations.rs` | v18: plans 확장, plan_events 테이블 |
| Backend | `plans.rs` | phase update, event 생성 commands |
| Backend | (신규) `test_runner.rs` | 프로젝트 테스트 실행 |
| Frontend Types | `types/index.ts` | PlanPhase, PlanEvent 추가 |
| Frontend API | `api/plans.ts` | 새 commands wrapper |
| Frontend Store | `assetSlice.ts` | plan events 상태 |
| UI | `PlansPanel.tsx` | 3-way gate, timeline, impl-plan 표시 |
| UI | (신규) `PlanProposalCard.tsx` | 채팅 내 plan proposal 카드 |
| UI | (신규) `planProposalParser.ts` | 마커 파싱 유틸 |
| UI | `MessageItem.tsx` or `MarkdownComponents.tsx` | plan-proposal 감지 + 카드 삽입 |
| ContextPack | `context_pack.rs` | plan phase 정보 포함 (선택적) |

---

## 11. 검증 계획

각 Phase 완료 후:
- `cargo check && cargo test --lib` (Rust 컴파일 + 테스트)
- `npx tsc --noEmit && npx vitest run` (Frontend)
- `tauri dev`에서 수동 시나리오 테스트:
  - Phase B: 채팅에서 "계획 세워줘" → plan-proposal 카드 표시 → 승격
  - Phase C: Plan 탭에서 승인/검토 루프
  - Phase D: Implementation Branch에서 Developer 보고 → 승인 → 구현
  - Phase E: 테스트 실행 → RT 리뷰 → 판정

---

## 12. 절대 하지 말 것

1. 기존 Plan CRUD API 시그니처 변경 금지 — 확장만
2. 기존 MessageActions 동작 변경 금지 — 새 카드 추가만
3. 마커 파싱 실패 시 silent fallback — 일반 메시지로 표시
4. Branch 자동 생성 실패 시 전체 워크플로우 차단 금지 — 수동 fallback 제공
5. 테스트 실행을 필수로 강제하지 않음 — 테스트 러너 미감지 시 skip
