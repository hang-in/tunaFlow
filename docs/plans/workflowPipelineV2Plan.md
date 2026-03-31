# Workflow Pipeline V2 — 6-Stage Redesign

> Status: draft
> Created: 2026-03-31
> Supersedes: orchestratedWorkflowPipelinePlan.md (Phase A-E 구현 완료 → 역할 재정의)

---

## 1. 목표

기존 5-stage 파이프라인을 6-stage로 재설계하여 **Architect의 what+how 설계**와 **Developer의 순차 구현**을 명확히 분리한다.

핵심 변경:
- Subtask 검토 스테이지 신설 (Plan과 Approved 사이)
- Approved = dev 대기열 (승인 게이트가 아님)
- Dev = subtask별 결과 보고 + 재수행/sub-plan
- impl-plan 마커 폐기 (Developer는 보고 없이 코딩만)

---

## 2. 6-Stage 정의

```
Plan → Subtask → Approved → Dev → Review → Decision
```

### 2.1 Plan (phase: "drafting")

**목적**: 플랜 리스트. Architect가 작성한 plan-proposal이 승격되면 여기에 나타남.

**UI**: 플랜 카드 리스트 (기존과 동일)
**액션**:
- 플랜 카드 클릭 → 상세 보기 (expand)
- subtask에 details(how)가 없으면 [상세 설계 요청] 버튼 → Architect에게 how 작성 요청
- how가 다 채워지면 [검토 시작] → phase="subtask_review" → Subtask 스테이지로 이동

### 2.2 Subtask (phase: "subtask_review") — 신규

**목적**: 선택한 플랜의 **전체 내용 + 각 subtask의 how를 검토**하는 영역.

**UI**:
```
┌─────────────────────────────────────────────┐
│ Plan: API 리팩토링  rev.2                    │
│ Description: ...                            │
│ Expected Outcome: ...                       │
│                                             │
│ ─── Subtasks ──────────────────────────────  │
│                                             │
│ 1. DB 스키마 변경                            │
│    How: migrations.rs v20, plans 테이블...   │
│    [의견 추가]  상태: ✅ 검토 완료            │
│                                             │
│ 2. API 엔드포인트 추가                       │
│    How: commands/plans.rs에 update_X...      │
│    [의견 추가] [수정 요청]  상태: 🔄 검토 중   │
│                                             │
│ 3. 프론트엔드 연동                           │
│    How: PlansPanel.tsx에 X 컴포넌트...       │
│    [의견 추가]  상태: ⏳ 대기                 │
│                                             │
│ ─── Actions ───────────────────────────────  │
│ [승인 → Approved]  [전체 수정 요청]           │
└─────────────────────────────────────────────┘
```

**검토 방식**:
- 사용자가 직접 subtask별 의견 작성 (메모 형태)
- [수정 요청] → Architect에게 해당 subtask의 how 수정 요청 (requestPlanRevision 패턴 재사용)
- [전체 수정 요청] → 플랜 전체를 Architect에게 재검토 요청
- 다른 에이전트에게 검토 의뢰 가능 (RT 분기 또는 forward)

**전환 조건**: 사용자가 [승인 → Approved] 클릭 → phase="approval"

### 2.3 Approved (phase: "approval")

**목적**: dev 대기열. 승인 완료된 플랜들이 대기.

**UI**: 플랜 카드 리스트 (간결)
**액션**:
- [Dev 시작] → Developer 에이전트 선택 → Implementation Branch 생성 → phase="implementation"
- 기존 ApprovalGate의 승인/보류/검토 제거 → [Dev 시작]만
- [되돌리기] → phase="subtask_review" (Subtask 스테이지로 복귀)

### 2.4 Dev (phase: "implementation", "rework")

**목적**: Developer가 subtask를 순차 구현한 결과를 확인.

**UI**:
```
┌─────────────────────────────────────────────┐
│ Plan: API 리팩토링  [Implementation Branch]   │
│                                             │
│ ─── Subtask 진행 현황 ──────────────────────  │
│                                             │
│ 1. DB 스키마 변경        ✅ 완료             │
│    결과: v20 migration 추가, 테스트 통과      │
│                                             │
│ 2. API 엔드포인트 추가    🔄 진행 중          │
│    (Developer가 작업 중...)                   │
│                                             │
│ 3. 프론트엔드 연동        ⏳ 대기             │
│                                             │
│ ─── Actions ───────────────────────────────  │
│ [브랜치 열기] [계획 수정 요청]                 │
└─────────────────────────────────────────────┘
```

**완료 조건**: Developer가 `<!-- tunaflow:impl-complete -->` 포함 → Review로 전환
**실패 시**:
- 개별 subtask 재수행 → Developer에게 해당 subtask만 재지시
- 새 sub-plan 생성 → 해당 subtask에 대한 새 plan-proposal → 워크플로 재진입 (프로젝트 전체가 아닌 부분 plan)

### 2.5 Review (phase: "review")

기존과 동일. RT 기반 2-agent 리뷰.

### 2.6 Decision (phase: "done" + status: "abandoned")

기존과 동일. 최종 판정.

---

## 3. 데이터 모델 변경

### 3.1 PlanPhase 확장

```sql
-- v20 migration: no schema change needed
-- "subtask_review" is just a new value for the existing phase TEXT column
```

```typescript
type PlanPhase = "drafting" | "subtask_review" | "approval" | "implementation" | "review" | "done" | "rework";
```

### 3.2 Subtask 검토 상태 (선택적)

subtask별 검토 상태를 추적하려면 `plan_subtasks`에 `review_status` 컬럼 추가:

```sql
ALTER TABLE plan_subtasks ADD COLUMN review_status TEXT DEFAULT 'pending';
-- "pending" | "reviewing" | "approved" | "revision_requested"
```

또는 plan_events로 추적 (스키마 변경 없이):
- event_type: "subtask_review_started", "subtask_review_approved", "subtask_revision_requested"
- detail: JSON with subtask_id

→ **Phase 1에서는 plan_events로 추적, 필요 시 컬럼 추가**

---

## 4. 구현 순서

### Phase 1: Subtask 스테이지 + HarnessSummary 업데이트

1. PlanPhase에 "subtask_review" 추가 (TS 타입 + Rust 모델 — 스키마 변경 없음)
2. HarnessSummary 6-stage 칩 (Plan → Subtask → Approved → Dev → Review → Decision)
3. STAGE_PHASE_MAP 업데이트 (CenterPanel + PlansPanel)
4. SubtaskReviewView 컴포넌트 신규 — 선택한 플랜의 전체 + subtask별 how 표시
5. Plan 스테이지에서 [검토 시작] 버튼 → phase="subtask_review"

### Phase 2: Subtask 검토 UX

1. SubtaskReviewView에 subtask별 의견 추가 (메모 연동 or 인라인)
2. [수정 요청] → Architect에게 특정 subtask how 수정 요청
3. [전체 수정 요청] → 기존 requestPlanRevision 재사용
4. [승인 → Approved] → phase="approval"

### Phase 3: Approved → Dev 시작 간소화

1. ApprovalGate 제거 → [Dev 시작] 버튼만
2. [되돌리기] → phase="subtask_review"

### Phase 4: Dev subtask별 결과 뷰

1. DevProgressView — subtask별 진행 상태 표시
2. 개별 subtask 재수행 액션
3. Sub-plan 생성 → 워크플로 재진입

---

## 5. 변경 범위

| 파일 | 변경 |
|------|------|
| `src/types/index.ts` | PlanPhase에 "subtask_review" 추가 |
| `src/components/tunaflow/context-panel/HarnessSummary.tsx` | 6-stage 칩 |
| `src/components/tunaflow/CenterPanel.tsx` | PHASE_TO_STAGE + TAB_PHASE_MAP 업데이트 |
| `src/components/tunaflow/context-panel/PlansPanel.tsx` | STAGE_PHASE_MAP, ApprovalGate 간소화 |
| (신규) `src/components/tunaflow/context-panel/SubtaskReviewView.tsx` | Subtask 검토 뷰 |
| (신규) `src/components/tunaflow/context-panel/DevProgressView.tsx` | Dev subtask별 진행 뷰 |
| `src/lib/workflowOrchestration.ts` | Subtask 검토 관련 함수 |
| `docs/plans/orchestratedWorkflowPipelinePlan.md` | status → archived, superseded_by 추가 |

---

## 6. 마이그레이션 참고

- 기존 "approval" phase 플랜 → 그대로 유지 (Approved 스테이지에 표시)
- 기존 "implementation" phase 플랜 → 그대로 유지 (Dev 스테이지에 표시)
- "subtask_review"는 새 값이므로 기존 데이터 영향 없음

---

## 7. 절대 하지 말 것

1. plan_subtasks 테이블 스키마 변경 (Phase 1 범위) — events로 추적
2. 기존 Plan CRUD API 시그니처 변경
3. impl-plan 마커 복원 (폐기 확정)
4. Developer에게 보고 요청 (Developer는 코딩만)
5. 전체 프로젝트 재설계가 아닌 부분 수정으로 점진적 진행

---

## 8. 역할 요약 (확정)

| 역할 | 책임 | 산출물 |
|------|------|--------|
| 사용자 | 방향 결정, 검토, 승인 | 승인/반려/의견 |
| Architect | what + how 설계 | Plan (subtask title + details) |
| Developer | 코딩만 (순차 구현) | 코드 + impl-complete |
| Reviewer | 결과 검증 | verdict (pass/fail/conditional) |
