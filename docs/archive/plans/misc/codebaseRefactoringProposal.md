# 코드베이스 리팩토링 제안서

> Status: draft
> Created: 2026-03-31
> 목적: 코드 품질 향상, 모듈 분리, 사이드 이펙트 방지를 위한 구조 개선 제안

---

## 1. 현재 상태 요약

### 1.1 코드베이스 규모

| 계층 | 파일 수 | 총 줄 수 | 비고 |
|------|---------|---------|------|
| Frontend Components | 52 | ~18,000 | 1개 파일 1,026줄 |
| Store Slices | 7 | ~1,490 | 1개 파일 469줄 |
| Lib (utils/api) | ~10 | ~925 | API 레이어 양호 |
| Backend Commands | ~20 | ~6,000 | 2개 파일 1,100줄+ |
| Backend Agents | 6 | ~1,850 | |
| DB Layer | 4 | ~955 | 잘 분리됨 |
| Tests | Frontend 11 files / 69 tests, Rust 60 tests | | |

### 1.2 양호한 부분

이 부분들은 변경하지 않는다:

- **API 레이어** (`src/lib/api/`) — 도메인별 분리, 순수 invoke wrapper, 150줄 4파일
- **planProposalParser.ts** — 외부 의존 없는 순수 파서, 테스트 완비 (168줄 테스트)
- **순환 의존 없음** — store↛component, lib↛component 경계 유지
- **DB 레이어** — schema/migrations/models 3파일 깔끔 분리
- **에러 처리** — 프로덕션 코드에 unwrap/panic 없음, AppError enum 커버리지 양호
- **타입 정의** (`types/index.ts` 334줄) — 모놀리식이나 순수 타입만 있어서 허용 범위

---

## 2. 문제 영역

### 2.1 [P0] PlansPanel.tsx — 1,026줄, 9개 컴포넌트

**현황:**

```
PlansPanel.tsx (1,026줄)
├── CreatePlanForm()      line 56-203   (148줄)
├── SubtaskRow()          line 204-329  (126줄)
├── EventTimeline()       line 330-360  (31줄)
├── DraftingActions()     line 361-434  (74줄)
├── ApprovalGate()        line 435-507  (73줄)
├── ReviewVerdictCard()   line 508-566  (59줄)
├── MergeBranchButton()   line 567-624  (58줄)
├── PlanCard()            line 625-943  (319줄)
└── PlansPanel()          line 944-1026 (83줄, export)
```

**문제:**
- 한 파일에 9개 컴포넌트 → 수정 시 어떤 컴포넌트에 영향이 가는지 파악 어려움
- PlanCard가 319줄로 내부에 phase별 UI를 모두 조건부 렌더링
- 서브탭 작업 시 이 파일을 동시에 여러 곳에서 수정해야 함 → 충돌 위험

**제안 구조:**
```
src/components/tunaflow/context-panel/plans/
├── index.ts                   (re-export)
├── PlansPanel.tsx             (컨테이너 + 스테이지 필터링, ~100줄)
├── PlanCard.tsx               (카드 본체 + subtask/event 로딩, ~320줄)
├── CreatePlanForm.tsx         (plan 수동 생성 — dead code 후보이나 보존)
├── SubtaskRow.tsx             (~130줄)
├── DraftingActions.tsx        (~75줄)
├── ApprovalGate.tsx           (~60줄)
├── ReviewVerdictCard.tsx      (~60줄)
├── MergeBranchButton.tsx      (~60줄)
├── EventTimeline.tsx          (~30줄)
└── constants.ts               (PLAN_STATUS_CFG, PLAN_PHASE_CFG, OWNER_OPTIONS 등)

# 이미 별도 파일로 존재 (이동 불필요):
src/components/tunaflow/context-panel/SubtaskReviewView.tsx  (Subtask 스테이지)
src/components/tunaflow/context-panel/DevProgressView.tsx    (Dev 스테이지)
```

**작업 방법:**
- 컴포넌트별로 잘라내어 새 파일로 이동
- 공유 상수/타입을 `constants.ts`에 모음
- PlanCard의 props interface를 명확히 정의 (현재 inline)
- CenterPanel.tsx의 import 경로만 변경 (`"./context-panel/PlansPanel"` → `"./context-panel/plans"`)

**주의:**
- 컴포넌트 간 공유 상태: `OWNER_OPTIONS`, `PLAN_STATUS_CFG`, `PLAN_PHASE_CFG`, `INPUT_CLS`
- PlanCard 내부에서 ApprovalGate, ReviewVerdictCard 등을 직접 사용 → import로 전환
- 기존 동작은 100% 보존 — 분리만, 로직 변경 없음

---

### 2.2 [P0] workflowOrchestration.ts — Store 직접 조작

**현황 (line 267-279):**
```typescript
const { useChatStore } = await import("@/stores/chatStore");
const store = useChatStore.getState();
const savedFragment = store.personaFragment;
const savedLabel = store.personaLabel;
useChatStore.setState({ personaFragment: null, personaLabel: null });
try {
  await store.sendWithEngine(architectEngine, prompt, undefined, systemPrompt);
} finally {
  useChatStore.setState({ personaFragment: savedFragment, personaLabel: savedLabel });
}
```

**문제:**
- 비즈니스 로직이 UI 상태(persona)를 직접 조작
- 동적 import이지만 런타임 의존 발생
- persona save/restore가 race condition 위험 (concurrent 호출 시)
- 테스트 시 store mock 필요 → 테스트 어려움

**제안: 콜백 주입 패턴**

```typescript
// workflowOrchestration.ts — store 의존 제거
export async function requestPlanRevision(
  plan: Plan,
  branchMessages: Message[],
  architectEngine: string,
  sendToArchitect: (engine: string, prompt: string, systemPrompt?: string) => Promise<void>,
): Promise<void> {
  // ... 압축/context 구성 ...
  await sendToArchitect(architectEngine, prompt, systemPrompt);
  await planApi.createPlanEvent(plan.id, "revision_requested", "user", ...);
}
```

```typescript
// PlansPanel.tsx (호출부) — store 접근은 UI 레이어에서
const store = useChatStore();
const handleRevision = async () => {
  await requestPlanRevision(plan, msgs, engine, async (eng, prompt, sys) => {
    // persona 관리는 UI 레이어 책임
    await store.sendWithEngine(eng, prompt, undefined, sys);
  });
};
```

**효과:**
- workflowOrchestration이 순수 비즈니스 로직 모듈로 격리
- 테스트 시 sendToArchitect만 mock하면 됨
- persona 관리 로직이 UI 레이어에 명시적으로 위치

---

### 2.3 [P1] workflowOrchestration.ts — 테스트 부재

**현황:** 8개 export 함수, 0개 테스트

| 함수 | 줄 수 | 테스트 | 위험도 |
|------|------|--------|--------|
| `startReviewBranch` | 27 | ✗ | 중 |
| `approveAndStartImplementation` | 35 | ✗ | **높음** |
| `approveImplPlan` | 6 | ✗ | 낮음 |
| `startReviewRT` | 61 | ✗ | **높음** |
| `processReviewVerdict` | 23 | ✗ | 중 |
| `requestPlanRevision` | 67 | ✗ | **높음** |
| `scanMessagesForMarkers` | 27 | ✗ | 중 |
| `buildPlanContext` (private) | 13 | ✗ | 낮음 |

**제안:**
- `workflowOrchestration.test.ts` 신규 생성
- invoke/planApi를 mock하여 phase 전환, event 생성, prompt 구성을 검증
- P0 리팩토링(store 결합 제거) 후에 작성하면 mock이 단순해짐

---

### 2.4 [P1] ENGINE_CONFIGS — slice 간 불필요한 의존

**현황:**
```
runtimeSlice.ts  ← ENGINE_CONFIGS 정의 (line 26-40)
    ↑
branchSlice.ts   ← import { ENGINE_CONFIGS } from "./runtimeSlice"  (line 2)
```

`branchSlice`가 `runtimeSlice`에 의존하는 유일한 이유가 `ENGINE_CONFIGS` 상수입니다.

**제안:** `src/lib/engineConfig.ts`로 추출

```typescript
// src/lib/engineConfig.ts
export interface EngineConfig {
  command: string;
  engineKey: string;
  label: string;
  hasChunkEvent: boolean;
}

export const ENGINE_CONFIGS: Record<string, EngineConfig> = {
  claude:   { command: "start_claude_stream", engineKey: "claude-code", ... },
  codex:    { command: "start_codex_run",     engineKey: "codex", ... },
  gemini:   { command: "start_gemini_stream", engineKey: "gemini", ... },
  opencode: { command: "start_opencode_run",  engineKey: "opencode", ... },
};
```

**영향 범위:** runtimeSlice, branchSlice, store-runtime.test.ts의 import 경로 변경만.

---

### 2.5 [P2] branchSlice.ts (469줄) — 이중 책임

**현황:** 13개 메서드가 두 가지 도메인을 담당

| 도메인 | 메서드 |
|--------|--------|
| Branch CRUD | loadBranches, createBranch, deleteBranch, renameBranch, adoptBranch, linkGitBranch |
| Thread 메시징 | openThread, closeThread, sendThreadMessage, sendThreadRoundtable, sendThreadRoundtableFollowup, openBranchStream, closeBranchStream |

**제안:** `branchSlice.ts` (CRUD, ~200줄) + `threadSlice.ts` (메시징, ~270줄)

**주의:** chatStore.ts에서 slice 합성 순서에 영향. threadSlice가 branchSlice의 상태(activeBranchId 등)를 읽어야 하므로 `get()` 패턴 유지.

---

### 2.6 [P2] send_common.rs (1,211줄) — identity 로직 분리

**현황:** Identity/Persona 관련 함수 3개 (~120줄) + PLATFORM_TIER0 상수가 prompt 조립/메시지 영속화와 섞여있음.

```
line 12:  PLATFORM_TIER0 (상수)
line 22:  build_identity_persona_fragment()
line 36:  build_identity_block()
line 67:  parse_identity_and_persona()
```

**제안:** `agents_helpers/identity.rs`로 분리 (~150줄)
- `build_identity_persona_fragment`, `build_identity_block`, `parse_identity_and_persona`, `PLATFORM_TIER0`
- `send_common.rs`에서 `use super::identity::*;` 로 참조
- 기존 테스트 9개 (line 966-1037)도 함께 이동

---

### 2.7 [P2] projects.rs (755줄) — 혼합 책임

**현황:** 11개 Tauri command가 3가지 도메인을 담당

| 도메인 | Commands |
|--------|----------|
| Project CRUD | list_projects, create_project, get_project, hide_project, validate_project_path |
| Rawq | get_rawq_status, ensure_rawq_index, start_rawq_index |
| Git/Scaffold | get_git_status, ensure_project_workflow_templates, scaffold_project_dir |

**제안:** `projects.rs` (CRUD, ~300줄) + `project_tools.rs` (rawq/git/scaffold, ~450줄)

---

### 2.8 [P3] plans.rs (420줄, 15 commands) — 모니터링

현재는 허용 범위이나 워크플로우 파이프라인 기능이 추가되면서 command가 더 늘어날 가능성이 있음. 20개 이상이 되면 `plans.rs` (CRUD) + `plan_workflow.rs` (phase/event/engine) 분리 고려.

---

## 3. 우선순위 및 의존 관계

```
P0-a: PlansPanel 분리 ──────────→ 서브탭 작업의 전제 조건
P0-b: workflowOrchestration      독립 (P0-a와 병렬 가능)
      store 결합 제거
                    ↓
P1-a: workflowOrchestration ───→ P0-b 완료 후 (mock이 단순해짐)
      테스트 추가
P1-b: ENGINE_CONFIGS 추출 ─────→ 독립 (언제든 가능)

P2: branchSlice 분리, ─────────→ 독립 (다른 작업과 무관)
    identity.rs 분리,
    projects.rs 분리
```

---

## 4. 리팩토링 원칙

1. **동작 변경 없음** — 모든 리팩토링은 구조 변경만. 로직 수정, 기능 추가, 알고리즘 변경 금지
2. **단계별 검증** — 각 파일 이동 후 `tsc --noEmit` + `vitest run` + `cargo check` 통과 확인
3. **import 경로만 변경** — re-export(`index.ts`)로 외부 import 경로 유지 가능하면 유지
4. **기존 테스트 전부 통과** — Frontend 69, Rust 60 테스트 기준
5. **한 번에 하나의 파일만 분리** — 여러 파일을 동시에 분리하지 않음
6. **새 뷰 컴포넌트 반영** — SubtaskReviewView, DevProgressView는 이미 별도 파일로 분리됨. PlansPanel 분리 시 이들과의 import 관계 유지

---

## 5. 효과 예측

| 지표 | 현재 | 리팩토링 후 |
|------|------|------------|
| 최대 파일 크기 (Frontend) | 1,026줄 | ~320줄 (PlanCard) |
| 최대 파일 크기 (Backend) | 1,211줄 | ~1,060줄 (send_common) |
| PlansPanel 컴포넌트 수 | 9개/1파일 | 1개/1파일 |
| slice 간 불필요 의존 | 1건 | 0건 |
| workflowOrchestration store 결합 | 직접 조작 | 콜백 주입 |
| 비즈니스 로직 테스트 커버리지 | 0% | 주요 함수 커버 |

---

## 6. 질문 사항

이 제안서를 검토한 후 아래 사항에 대해 의견을 주세요:

1. **P0-a PlansPanel 분리**: 서브탭은 이미 HarnessSummary 6-stage로 완료됨. 분리만 남음 → 즉시 진행 가능.
2. **P0-b store 결합 제거**: 콜백 주입 패턴이 적절한가, 아니면 다른 패턴(이벤트 버스, 미들웨어 등)이 더 나은가? → 콜백 주입이 가장 단순하고 테스트 가능. 추천.
3. **P2 우선순위**: branchSlice 분리 → identity.rs 분리 → projects.rs 분리 순서 추천. branchSlice가 469줄로 가장 크고 thread 메시징 작업 시 빈번히 수정.
4. **추가 발견**: HarnessSummary.tsx(170줄)와 CenterPanel.tsx(368줄)는 현재 허용 범위. PlansPanel 분리 후 재평가.
