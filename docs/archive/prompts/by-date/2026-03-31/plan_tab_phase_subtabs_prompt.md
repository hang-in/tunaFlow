# Plan 탭 Phase 서브탭 리팩토링

프로젝트: `/Users/d9ng/privateProject/tunaFlow`
모든 응답과 보고는 한국어로 작성하라.

---

## 사전 읽기 (필수)

아래 파일을 **반드시** 먼저 읽어라. 읽지 않고 작업하면 기존 동작이 깨진다.

1. `CLAUDE.md` — 프로젝트 전체 구조, 코딩 컨벤션, 안전 규칙
2. `src/components/tunaflow/context-panel/PlansPanel.tsx` — 리팩토링 대상 (전체 읽기, ~1004줄)
3. `src/components/tunaflow/CenterPanel.tsx` — PlansPanel 호출부 (line 242-250)
4. `src/types/index.ts` — `PlanPhase`, `Plan`, `PlanSubtask` 타입 (line 277-305)
5. `src/lib/api/plans.ts` — plan API 함수들
6. `src/lib/workflowOrchestration.ts` — phase 전환 함수들
7. `docs/plans/orchestratedWorkflowPipelinePlan.md` — 전체 파이프라인 설계 (참고용)

---

## 배경

현재 PlansPanel은 **모든 plan을 단일 리스트**로 표시한다. Plan의 `phase` 필드(drafting/approval/implementation/review/done/rework)에 따라 카드 내부에 다른 UI가 인라인으로 보이지만, 전체 리스트에서 phase별로 구분되어 있지 않다.

워크플로우 파이프라인이 도입되면서 plan이 phase를 따라 이동하는 흐름이 생겼으므로, Plan 탭 내부에 **phase별 서브탭**을 추가하여 칸반보드처럼 plan이 단계를 따라 이동하는 UX를 만들어야 한다.

---

## 목표

PlansPanel에 **서브탭 네비게이션**을 추가하여 plan을 phase별로 필터링 표시한다.

```
[Plan] 탭 (CenterPanel)
  ↓
[PLAN | APPROVED | DEV | REVIEW | DONE]  ← 서브탭
  ↓
해당 phase의 plan 카드들만 표시
```

### 서브탭 ↔ PlanPhase 매핑

| 서브탭 | 표시할 phase | 설명 |
|--------|-------------|------|
| PLAN | `drafting` | 아직 승격 전이거나 작성 중인 plan |
| APPROVED | `approval` | 승격 후 승인 대기 중 |
| DEV | `implementation`, `rework` | 구현 중 또는 rework 중 |
| REVIEW | `review` | 리뷰 진행 중 |
| DONE | `done`, `abandoned` | 완료/중단된 아카이브 |

### 핵심 동작

1. **서브탭 전환** → 해당 phase의 plan만 필터링하여 표시
2. **phase 전환 시 자동 서브탭 이동** → plan의 phase가 바뀌면 현재 활성 서브탭도 새 phase에 해당하는 탭으로 자동 전환
3. **카드 내부 UI는 그대로 유지** → 각 phase에 맞는 기존 인라인 UI가 해당 서브탭 아래에서 보임:
   - APPROVED 탭: `ApprovalGate` (승인/보류/검토 버튼)
   - DEV 탭: `ImplPlanCard` + "계획 수정 요청" + "Review RT 시작" 등
   - REVIEW 탭: `ReviewVerdictCard`
4. **"New plan" 버튼 제거** — Plan은 Chat 탭에서 Architect의 plan-proposal 승격으로만 생성
5. **드로어 연동** — DEV 탭에서 plan 카드 클릭 시 Implementation Branch 드로어가 열림 (기존 `openThread` 활용)

---

## 현재 구조 분석

### PlansPanel (line 921-1004)

```typescript
export function PlansPanel() {
  // plans: Plan[] — 전체 plan 리스트
  // 단순 map으로 PlanCard 렌더링
  // 하단에 "New plan" 버튼 + CreatePlanForm
}
```

### PlanCard (line 619-919)

- plan의 phase에 따라 내부 UI가 조건부 렌더링:
  - `plan.phase === "approval"` → `ApprovalGate` (line 785-803)
  - `plan.phase === "implementation"` → `ImplPlanCard` + `PlanRevisionButton` (line 806-835)
  - `plan.phase === "review"` → `ReviewVerdictCard` (line 838-851)
  - `plan.phase === "rework"` → rework UI (line 854-889)
- 이 구조는 **변경하지 않는다**. PlanCard 내부 로직은 그대로 두고, PlansPanel이 어떤 카드를 보여줄지만 필터링한다.

### 변경이 필요한 컴포넌트

| 컴포넌트 | 변경 내용 |
|----------|----------|
| `PlansPanel` | 서브탭 상태 + 필터링 + phase 전환 시 자동 탭 이동 |
| `CenterPanel` | 없음 (PlansPanel 호출 방식 불변) |
| `PlanCard` | `onPhaseChange` 콜백 추가 (phase 전환 시 부모에게 알림) |
| `CreatePlanForm` | 제거 (+ "New plan" 버튼도 제거) |

---

## 작업 단계

### Step 1: 서브탭 상수 + 상태 추가

`PlansPanel` 내부에 서브탭 정의:

```typescript
const PHASE_TABS = [
  { key: "plan",     label: "PLAN",     phases: ["drafting"] as PlanPhase[] },
  { key: "approved", label: "APPROVED", phases: ["approval"] as PlanPhase[] },
  { key: "dev",      label: "DEV",      phases: ["implementation", "rework"] as PlanPhase[] },
  { key: "review",   label: "REVIEW",   phases: ["review"] as PlanPhase[] },
  { key: "done",     label: "DONE",     phases: ["done", "abandoned"] as PlanPhase[] },
] as const;

type PhaseTabKey = typeof PHASE_TABS[number]["key"];
```

`PlansPanel`에 `activePhaseTab` 상태 추가:

```typescript
const [activePhaseTab, setActivePhaseTab] = useState<PhaseTabKey>("plan");
```

### Step 2: 서브탭 렌더링

PlansPanel의 return 부분에서 "Plans" 제목 아래, plan 리스트 위에 서브탭 바 추가:

```typescript
<div className="flex items-center gap-1 mb-3 border-b border-border/30 pb-1.5">
  {PHASE_TABS.map((tab) => {
    const count = plans.filter((p) => tab.phases.includes(p.phase)).length;
    const isActive = activePhaseTab === tab.key;
    return (
      <button
        key={tab.key}
        onClick={() => setActivePhaseTab(tab.key)}
        className={cn(
          "px-2.5 py-1 rounded-t text-[10px] font-medium transition-colors",
          isActive
            ? "text-foreground bg-accent border-b-2 border-primary"
            : "text-muted-foreground/60 hover:text-muted-foreground"
        )}
      >
        {tab.label}
        {count > 0 && <span className="ml-1 text-[9px] opacity-60">({count})</span>}
      </button>
    );
  })}
</div>
```

### Step 3: plan 필터링

기존 `plans.map(...)` 부분을 활성 탭의 phase로 필터링:

```typescript
const activeTab = PHASE_TABS.find((t) => t.key === activePhaseTab)!;
const filteredPlans = plans.filter((p) => activeTab.phases.includes(p.phase));
```

빈 상태 메시지도 탭에 맞게 변경:

```typescript
{filteredPlans.length === 0 && (
  <div className="text-center py-4">
    <ClipboardList className="w-5 h-5 text-muted-foreground/40 mx-auto mb-2" />
    <p className="text-xs text-muted-foreground">
      {activePhaseTab === "plan"
        ? "Chat 탭에서 Architect와 대화하여 Plan을 생성하세요."
        : `${activeTab.label} 단계의 Plan이 없습니다.`}
    </p>
  </div>
)}
```

### Step 4: phase 전환 시 자동 서브탭 이동

`handlePlanUpdated`에서 phase가 변경되면 해당 phase의 서브탭으로 자동 전환:

```typescript
const handlePlanUpdated = (planId: string, update: Partial<Plan>) => {
  setPlans((prev) => prev.map((p) => (p.id === planId ? { ...p, ...update } : p)));

  // Phase 변경 시 자동으로 해당 서브탭으로 이동
  if (update.phase) {
    const targetTab = PHASE_TABS.find((t) => t.phases.includes(update.phase!));
    if (targetTab) {
      setActivePhaseTab(targetTab.key);
    }
  }
};
```

### Step 5: "New plan" 버튼 + CreatePlanForm 제거

PlansPanel에서 아래 코드를 **삭제**한다:

- `showForm` 상태 (`useState(false)`)
- `expandedNewId` 상태
- `handleCreated` 함수
- `{showForm && <CreatePlanForm .../>}` 블록
- `{!showForm && <button>New plan</button>}` 블록

**주의**: `CreatePlanForm` 컴포넌트 정의 자체는 삭제하지 않는다. PlanProposalCard에서 직접 `planApi.createPlan()`을 호출하므로 CreatePlanForm은 사용되지 않지만, dead code 정리는 별도 작업이다.

단, `showForm`, `expandedNewId`, `handleCreated`를 참조하는 import나 코드가 있다면 함께 정리한다.

### Step 6: plan 추가 시 자동 서브탭 이동

Chat 탭에서 plan-proposal 승격 시 `PlansPanel`에 새 plan이 추가되면 해당 phase 탭으로 이동해야 한다. 현재 `handleCreated`로 하던 것을 `plans` 상태 변경 감지로 대체:

`useEffect`에서 plans가 변경될 때 새로 추가된 plan이 현재 탭에 없으면 해당 탭으로 이동:

```typescript
// 새 plan이 추가되면 해당 phase 탭으로 자동 이동
useEffect(() => {
  if (plans.length === 0) return;
  const newest = plans[0]; // plans는 created_at DESC 정렬
  const targetTab = PHASE_TABS.find((t) => t.phases.includes(newest.phase));
  if (targetTab && targetTab.key !== activePhaseTab) {
    // 최신 plan이 현재 탭에 없으면 해당 탭으로 이동
    const currentPhases = PHASE_TABS.find((t) => t.key === activePhaseTab)?.phases ?? [];
    if (!currentPhases.includes(newest.phase)) {
      setActivePhaseTab(targetTab.key);
    }
  }
}, [plans]);
```

---

## 검증

각 단계 완료 후:
```bash
cd /Users/d9ng/privateProject/tunaFlow
npx tsc --noEmit              # TypeScript
npx vitest run                # Frontend tests (66+)
cd src-tauri && cargo check   # Rust (변경 없지만 확인)
```

최종 시각적 확인 (`tauri dev`):
1. Plan 탭 진입 시 서브탭 바 (PLAN | APPROVED | DEV | REVIEW | DONE) 표시
2. 각 서브탭 클릭 시 해당 phase의 plan만 표시
3. plan이 없는 탭: PLAN 탭은 "Chat 탭에서 Architect와 대화하여 Plan을 생성하세요" 메시지
4. Chat에서 plan-proposal 승격 → APPROVED 탭으로 자동 이동
5. APPROVED 탭에서 승인 → DEV 탭으로 자동 이동 + Implementation Branch 드로어 열림
6. DEV 탭에서 "계획 수정 요청" 버튼 정상 동작
7. Review RT 시작 → REVIEW 탭으로 자동 이동
8. verdict pass → DONE 탭으로 자동 이동
9. "New plan" 버튼이 사라졌는지 확인

---

## 절대 하지 말 것

1. **PlanCard 내부 로직 변경 금지** — 카드 안의 ApprovalGate, ImplPlanCard, ReviewVerdictCard, PlanRevisionButton, MergeBranchButton 등의 동작은 현재 그대로 유지. PlansPanel이 어떤 카드를 보여줄지만 필터링
2. **CenterPanel 변경 금지** — PlansPanel 호출 방식 불변
3. **PlanCard의 phase 조건부 렌더링 제거 금지** — `plan.phase === "implementation" && ...` 같은 조건문들은 그대로 둔다. 서브탭이 필터링하므로 중복 방어이지만 제거하면 서브탭 버그 시 전체 UI가 깨짐
4. **API 시그니처 변경 금지** — planApi 함수, Tauri command 시그니처 불변
5. **workflowOrchestration.ts 변경 금지** — phase 전환 로직 불변
6. **CreatePlanForm 컴포넌트 정의 삭제 금지** — 호출부만 제거, 정의는 dead code로 남김 (별도 정리)
7. **기존 테스트를 깨뜨리는 변경 금지**

---

## 사이드 이펙트 체크리스트

이 변경으로 영향받을 수 있는 경로들:

| 경로 | 확인 사항 |
|------|----------|
| Chat 탭 → PlanProposalCard → 승격 | 승격 후 plans 리로드 → APPROVED 탭 이동 확인 |
| Plan 탭 → ApprovalGate → 승인 | `handlePlanUpdated({ phase: "implementation" })` → DEV 탭 이동 확인 |
| DEV 탭 → 계획 수정 요청 | requestPlanRevision은 메인 채팅에서 동작 → Plan 탭에 영향 없음 확인 |
| DEV 탭 → Review RT 시작 | `handlePlanUpdate({ phase: "review" })` → REVIEW 탭 이동 확인 |
| REVIEW 탭 → verdict pass | `handlePlanUpdate({ phase: "done" })` → DONE 탭 이동 확인 |
| Rework | `handlePlanUpdate({ phase: "implementation" })` → DEV 탭 이동 확인 |
| Branch에서 Plan 탭 열기 | `canonicalConvId`가 parentConversationId로 resolve → 서브탭 동작 무관 |

---

## 참고: 미결정 사항 (이번 작업 범위 아님)

아래 내용은 이전 논의에서 언급되었으나 아직 확정/구현되지 않은 사항이다. 이번 작업에서는 건드리지 않는다.

1. **헤드 에이전트 기본값**: 채팅/Plan의 기본 에이전트를 Architect로 설정하는 UX. 현재는 사용자가 수동으로 에이전트를 선택한다.
2. **Workflow Skill Tier 1/2**: plan 활성 시 상세 마커 규약을 ContextPack에 추가 주입하는 기능.
3. **Agent Template 자동 로딩**: `docs/agents/*.md` 파일을 role에 맞게 ContextPack에서 자동 로딩하는 기능.
4. **CreatePlanForm dead code 정리**: 사용처가 제거된 후 컴포넌트 정의도 별도 작업으로 삭제.
