# Subtask 01 — PlanProposalCard 2버튼 + 힌트 pill + RT 드로어 mode 구분

> 상위 plan: [designReviewGatePlan.md](./designReviewGatePlan.md)

## Changed files

- `src/components/tunaflow/chat/PlanProposalCard.tsx` — 승인 action 을 2개 버튼으로 분기 + 카드 상단 hint row.
- `src/components/tunaflow/RoundtableDrawer.tsx` (또는 동등) — `branch.mode === 'design_review'` 시 드로어 헤더에 구분 배지.
- `src/store/*.ts` — plan proposal 상태 슬라이스에 `approvalPath: 'direct' | 'rt' | null` 필드 추가 (이미 있으면 재사용).
- `src/types/plans.ts` (또는 동등) — `PlanProposal` 인터페이스에 `touchedPaths?: string[]`, `planDocumentPath?: string` 추가.

## Change description

### 1. Hint heuristic

`PlanProposalCard.tsx` 내부 또는 util 파일:

```ts
function shouldSuggestRT(plan: PlanProposal): boolean {
  if (plan.invariants && plan.invariants.length >= 3) return true;
  if (plan.subtasks && plan.subtasks.length >= 2) return true;
  const sensitive = ['src-tauri/src/db/migrations', 'src-tauri/src/agents/',
                     'src-tauri/src/commands/agents_helpers/send_common/'];
  return (plan.touchedPaths ?? []).some(p => sensitive.some(s => p.startsWith(s)));
}
```

이 함수는 **힌트 pill 표시 여부만** 결정. 버튼 활성화 / 비활성화에는 영향 없음 (INV-1).

### 2. 2-button 렌더

기존 단일 "승인" 버튼 자리에:

```tsx
<ActionRow>
  <Button
    variant="ghost"
    onClick={() => onApproveDirect(plan)}
    data-testid="plan-approve-direct"
  >
    바로 승인 → 구현
  </Button>
  <Button
    variant="primary"
    onClick={() => onApproveViaRT(plan)}
    data-testid="plan-approve-via-rt"
  >
    RT 검토 먼저 (Architect ↔ Codex)
  </Button>
</ActionRow>
```

`onApproveDirect` 는 기존 승인 경로 호출. `onApproveViaRT` 는 Subtask 02 의 신규 Tauri command `open_design_review_branch({ planId, planDocumentPath })` 호출.

### 3. Hint row

카드 헤더 하단:

```tsx
<div className="flex items-center gap-2 text-xs text-gray-500">
  <span>INV {plan.invariants?.length ?? 0}</span>
  <span>·</span>
  <span>Subtask {plan.subtasks?.length ?? 0}</span>
  <span>·</span>
  <span>{plan.touchedPaths?.length ?? 0} paths</span>
  {shouldSuggestRT(plan) && (
    <span className="ml-2 px-2 py-0.5 rounded-full bg-yellow-100 text-yellow-800 text-[10px] font-medium">
      RT 검토 권장
    </span>
  )}
</div>
```

pill 은 권장 문구일 뿐 — 버튼 disabled 로 연결하지 않음.

### 4. RT 드로어 mode 구분 배지

`RoundtableDrawer` 헤더에:

```tsx
{branch.mode === 'design_review' && (
  <Badge variant="accent">Design Review (Round {round}/3)</Badge>
)}
{branch.mode === 'roundtable' && (
  <Badge variant="default">Roundtable</Badge>
)}
```

`round` 는 branch state 또는 별도 Tauri command (`get_design_review_round(branchId)`) 로 읽음 — Subtask 02 범위.

### 5. Zustand 상태 확장

기존 `planProposalSlice` 에 `approvalPath` 필드:

```ts
interface PlanProposalState {
  // ... 기존 필드 ...
  approvalPath: 'direct' | 'rt' | null;
  rtRound: number;           // 0 = not started
  rtVerdict: 'pass' | 'fail' | 'escalate_to_human' | null;
  rtBlockerCount: number;    // INV-2 용 disabled 판정
}
```

`onApproveDirect` 는 `approvalPath='direct'`, `onApproveViaRT` 는 `'rt'` set.

### 6. INV-2 반영 — 강제 승인 전 BLOCKER 잔존 시 disabled

```tsx
const blocksDirect = approvalPath === 'rt' && rtVerdict === 'fail' && rtBlockerCount > 0;

<Button
  disabled={blocksDirect}
  onClick={blocksDirect ? openForceApprovalModal : onApproveDirect}
>
  {blocksDirect ? '강제 승인 (BLOCKER 있음)' : '바로 승인 → 구현'}
</Button>
```

강제 승인 모달은 Subtask 02 의 backend command (`force_approve_design_review`) 호출.

## Dependencies

depends_on: 없음 (Backend Subtask 02 의 신규 Tauri command 들은 mock 으로 대체 가능하나 실제 동작은 02 구현 후).

## Verification

- `npx vitest run src/components/tunaflow/chat/PlanProposalCard.test.tsx`:
  - 기본 rendering — 2 버튼 모두 표시
  - `plan.invariants.length === 4` → hint pill `RT 검토 권장` 표시
  - `plan.invariants.length === 1` + subtasks 0 + touchedPaths=['src/foo.ts'] → pill 미표시
  - "바로 승인" 클릭 → `onApproveDirect` 호출
  - "RT 검토 먼저" 클릭 → `onApproveViaRT` 호출
  - `rtVerdict='fail'` + `rtBlockerCount=1` → "바로 승인" disabled
- `npx tsc --noEmit` — exit 0.
- 수동: PlanProposalCard 가 표시되는 실제 플로우에서 두 버튼 클릭 시 Zustand state 변경 확인.

## Risks

- **PlanProposalCard prop 구조 변경 여지**: `touchedPaths`, `planDocumentPath`, `invariants`, `subtasks` 필드가 현재 prop 에 없을 수 있음. Architect 가 plan 산출 시 이 필드들을 응답에 포함하도록 Architect prompt 수정이 필요할 수 있음 — 이는 **Q-1 (plan 본문 §Open questions)** 로 flagged. Developer 는 구현 시 현재 PlanProposalCard prop 을 grep 하고 누락된 필드를 Architect 응답 파싱 쪽에 추가해야 할 수 있음.
- **Zustand slice 충돌**: `planProposalSlice` 가 이미 있는지 확인. 없으면 신설. 있으면 필드 추가만.
- **hint heuristic 의 sensitive paths 하드코딩**: 초기 릴리스는 하드코딩. 후속 Settings 에서 사용자 정의 경로 허용 가능 — 본 subtask 범위 밖.
- **버튼 배치 우선순위**: "RT 검토" 를 primary 로 둘지 "바로 승인" 을 primary 로 둘지 UX 판단. 본 subtask 는 RT 를 primary (눈에 더 잘 띔) 로 제안 — shouldSuggestRT=false 일 때는 "바로 승인" 을 primary 로 flip 하는 동적 변형도 고려 가능.
