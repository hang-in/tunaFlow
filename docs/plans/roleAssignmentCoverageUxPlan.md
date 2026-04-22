---
title: Role Assignment Coverage UX — inferred 저장 명시화 + stale ID 자동 정리
status: planned
priority: P2
created_at: 2026-04-22
related:
  - src/components/tunaflow/settings/AgentsSection.tsx           # RoleCoveragePanel 신설 지점
  - src/lib/roleAssignments.ts                                    # load/save/infer/evaluateCoverage
  - docs/plans/harnessVerificationGapPlan.md                      # §5 proposer 규약
triggered_by:
  - 2026-04-22 사용자 리포트 — reviewer 프로필 2개를 만든 상태에서 "Reviewer (≥2) 프로필이 설정되지 않았습니다" 토스트.
  - 원인: `AgentsSection.tsx:192-205` 의 auto-infer 가 state 에만 반영되고 `saveRoleAssignments` 호출 안 함.
    사용자가 수동 토글 후 RT 정상 진입 → 시나리오 확정.
---

# Role Assignment Coverage UX — inferred 저장 명시화

> 현재 Settings 의 "역할 커버리지" 패널은 profile 자동 추론 결과를 체크된 것처럼 보여주지만 DB/store 에는 저장하지 않는다. 사용자는 "설정 완료" 라 판단하나 `assertRoleReady` 는 saved empty 만 읽어 에러를 낸다. 본 plan 은 UX 를 명시적으로 만들어 상태 오인을 제거한다.

---

## TL;DR for Developer

1. **`AgentsSection.tsx:197-201` 의 auto-infer 블록을 "Persist 후 표시" 로 고친다** — inferred 를 즉시 `saveRoleAssignments` 호출. state 와 store 일치.
2. **"추천 구성 적용" 원클릭 버튼을 RoleCoveragePanel 상단에 추가** (Option B 선호) — 초기에는 inferred 값이 **플레이스홀더** 로만 표시되고 (회색 체크 + "제안됨" 배지), 사용자가 버튼 클릭 시 실제 저장. Option C (매 역할마다 수동 체크) 도 고려했으나 UX 마찰 큼. Open question Q-1 로 최종 선택 flag.
3. **Stale profile ID 자동 정리** — `evaluateCoverage` 가 `byId.has(id)` 로 거르는 stale ID 를 감지하면 자동으로 pruned assignments 를 save + toast 안내 ("삭제된 프로필 N개 제거됨").
4. **RT 진입 전 `assertRoleReady` 의 toast 를 액션화** — 이미 "Settings 열기" 액션이 있음. inferred 가 유효하면 "추천 구성 적용" 바로가기도 제공.

구현 순서: 1 → 2 → 3 → 4. 1~2 는 같은 파일에 집중 (10~20 LOC). 3~4 는 `roleAssignments.ts` 에 API 추가 + 호출부.

**하지 말 것**: 이 plan 을 Q-5 (auto-magic persist) 방향으로 구현 — 사용자 명시성 원칙과 충돌. 본 plan 은 "inferred 는 제안, 적용은 1-click" 을 기본 스탠스로 함.

---

## Specification

### 1. `roleAssignments.ts` API 확장

현재 `inferRoleAssignments(profiles)` 는 순수 추론 함수. 신규 헬퍼:

```ts
// src/lib/roleAssignments.ts

/** Stale ID 제거 — profiles 에 없는 ID 를 assignments 에서 drop. */
export function pruneStaleIds(
  assignments: RoleAssignments,
  profiles: AgentProfile[],
): { pruned: RoleAssignments; droppedCount: number } {
  const valid = new Set(profiles.map((p) => p.id));
  let dropped = 0;
  const keepOne = (id?: string) => {
    if (!id) return undefined;
    if (valid.has(id)) return id;
    dropped += 1;
    return undefined;
  };
  const reviewers = assignments.reviewers.filter((id) => {
    if (valid.has(id)) return true;
    dropped += 1;
    return false;
  });
  return {
    pruned: {
      architect: keepOne(assignments.architect),
      developer: keepOne(assignments.developer),
      reviewers,
      synthesizer: keepOne(assignments.synthesizer),
    },
    droppedCount: dropped,
  };
}

/** "추천 구성 적용" 버튼이 호출. inferred 를 persist. */
export async function applyInferredAssignments(
  profiles: AgentProfile[],
): Promise<RoleAssignments> {
  const inferred = inferRoleAssignments(profiles);
  await saveRoleAssignments(inferred);
  return inferred;
}
```

### 2. `AgentsSection.tsx` — RoleCoveragePanel 재설계

기존 `useEffect` (L192-205):

```tsx
// before
if (!a.architect && !a.developer && a.reviewers.length === 0 && profiles.length > 0) {
  setAssignments(inferRoleAssignments(profiles));
} else {
  setAssignments(a);
}
```

→ 새 동작:

```tsx
useEffect(() => {
  let alive = true;
  loadRoleAssignments().then((saved) => {
    if (!alive) return;
    // (a) stale ID 자동 정리 (§3)
    const { pruned, droppedCount } = pruneStaleIds(saved, profiles);
    if (droppedCount > 0) {
      saveRoleAssignments(pruned);
      toast.info(`삭제된 프로필 ${droppedCount}개를 역할 배정에서 제거했습니다`, { duration: 4000 });
    }
    // (b) saved 가 전부 비어있으면 inferred 를 "제안 상태" 로 표시 (저장 X)
    const isEmpty = !pruned.architect && !pruned.developer && pruned.reviewers.length === 0;
    if (isEmpty && profiles.length > 0) {
      const inferred = inferRoleAssignments(profiles);
      setAssignments(inferred);
      setIsInferred(true);       // ★ 제안 상태 flag
    } else {
      setAssignments(pruned);
      setIsInferred(false);
    }
    setLoaded(true);
  });
  return () => { alive = false; };
}, [profiles]);
```

`isInferred: boolean` state 추가. 제안 상태일 때 UI 렌더 변경:

```tsx
{isInferred && (
  <div className="flex items-center gap-2 p-2 rounded-md bg-amber-500/10 border border-amber-500/30">
    <AlertTriangle className="w-4 h-4 text-amber-500" />
    <span className="text-tf-sm text-foreground flex-1">
      추천 구성이 제안되어 있지만 아직 저장되지 않았습니다
    </span>
    <Button
      size="sm"
      onClick={async () => {
        const applied = await applyInferredAssignments(profiles);
        setAssignments(applied);
        setIsInferred(false);
      }}
    >
      추천 구성 적용
    </Button>
  </div>
)}
```

제안 상태에서 RoleRow/ReviewersRow 는 **회색 / 반투명** + "제안됨" 배지:

```tsx
<RoleRow
  label="Reviewer"
  coverage={coverage[2]!}
  profiles={profiles}
  selectedIds={assignments.reviewers}
  onToggle={toggleReviewer}
  tentative={isInferred}   // ← 신규 prop
/>
```

`tentative=true` 일 때 체크박스는 render 되지만 opacity-50 + "제안됨" 배지 inline. 사용자가 직접 토글하면 isInferred→false + 즉시 save.

### 3. Stale ID 자동 정리 & toast

§1 의 `pruneStaleIds` 가 load 시점에 자동 호출 (§2 의 useEffect 내부). dropped > 0 이면 사용자에게 toast 1회. 후속으로는 pruned 값이 이미 저장됨.

### 4. `assertRoleReady` toast 에 "추천 구성 적용" 바로가기

기존 toast 액션 (`roleAssignments.ts:127-133`) 은 "Settings 열기" 만 있음. 추가:

```ts
// src/lib/roleAssignments.ts::assertRoleReady

if (cov.status === "missing") {
  const inferred = inferRoleAssignments(profiles);
  const wouldSatisfy =
    role === "reviewers" ? inferred.reviewers.length >= 2
    : role === "architect" ? !!inferred.architect
    : role === "developer" ? !!inferred.developer
    : role === "synthesizer" ? !!inferred.synthesizer
    : false;

  const { toast } = await import("sonner");
  if (wouldSatisfy) {
    toast.error(cov.hint, {
      action: {
        label: "추천 구성 적용",
        onClick: async () => {
          await applyInferredAssignments(profiles);
          toast.success("역할 구성 적용됨 — 다시 시도하세요");
        },
      },
      duration: 8000,
    });
  } else {
    toast.error(cov.hint, {
      action: {
        label: "Settings 열기",
        onClick: () => window.dispatchEvent(new CustomEvent("tunaflow:open-settings", { detail: { section: "agents" } })),
      },
      duration: 8000,
    });
  }
  return { ok: false, coverage: cov };
}
```

즉 inferred 가 충족시킬 수 있다면 Settings 안 열고 원클릭 해결. 아니면 Settings 유도.

---

## Invariants

- **[INV-1]** Settings UI 에 **체크박스로 표시되는 값** 과 `getSetting("roleAssignments")` 가 항상 일치한다. "제안 상태 (inferred 미저장)" 은 tentative 배지 + opacity 시각 차이로 명시적으로 구분된다. **이유**: 현 버그의 근본 원인 — 표시와 저장 불일치. **검증**: Vitest — `isInferred=true` 상태에서 `getSetting` 호출 시 empty 반환 확인. 사용자가 체크박스 직접 토글 → `isInferred=false` 로 전이 + `getSetting` 가 새 값 반환.

- **[INV-2]** `loadRoleAssignments` 는 호출 시마다 stale ID (현 `agentProfiles` 에 없는 ID) 를 자동 정리하고, 정리가 발생한 경우 1회 toast 로 사용자에게 알린다. 정리 후 값은 **즉시 persist**. **이유**: profile 삭제/재생성 시 stale 로 인해 "프로필 2개 있는데 에러" 재현. **검증**: Unit test — assignments.reviewers=['stale-1', 'stale-2'], profiles=[{id:'new-1'}] 입력 시 pruned.reviewers=[] + droppedCount=2.

- **[INV-3]** `assertRoleReady` 가 missing 반환할 때, inferred 가 해당 role 을 충족시킬 수 있으면 toast 액션은 "추천 구성 적용" (원클릭) 이다. 충족 불가 시만 "Settings 열기". **이유**: 불필요한 Settings 진입을 줄이고 UX friction 감소. **검증**: Integration test — 2개 reviewer persona 를 가진 profiles 상태에서 assertRoleReady("reviewers") 호출 후 toast 액션 label 검증.

---

## Rationale (reviewer-only)

### Option B (원클릭 승인) vs Option C (매 역할 수동 체크) — 선정 근거

| 옵션 | 동작 | 장점 | 단점 |
|---|---|---|---|
| **B (채택)** | inferred 는 제안 상태로 표시 + "추천 구성 적용" 버튼 1개로 일괄 persist | 1-click 으로 즉시 사용 가능, 사용자 명시성 유지 | 사용자가 개별 역할에 custom 배정을 원하면 버튼 클릭 후 개별 수정 필요 |
| C | inferred 를 체크박스로 **표시하지 않고** placeholder 상태로만 (비활성). 사용자가 매 역할마다 직접 체크 | 가장 명시적 — 모든 값이 user action 결과 | UX friction 큼 — 신규 사용자 4 번 토글 필요 |
| ~A (auto-magic persist)~ | 기각 — 사용자 방침 "명시성 > 자동화" |
| ~Hot fix only~ | 기각 — 사용자가 C 경로 (UX polish) 선택 |

Option B 는 **명시성과 편의성의 균형** 이 가장 좋고, Q-5 (메타에이전트 자동화) 와도 호환 — 메타에이전트가 자동 생성하는 경우에도 "추천 구성 적용 필요" 라는 동일 UX 를 재활용 가능.

### stale ID 자동 정리의 정당성

`byId.has(id)` 로 거르는 현재 로직은 conservative — valid 가 줄어들어도 assignments 는 유지. 이유는 "프로필이 잠시 reload 되지 않았을 가능성" 인데, 실제로는 `useChatStore.agentProfiles` 가 이미 최신 상태이므로 실수가 아님. 자동 정리가 안전.

### 왜 hot fix 를 건너뛰는가

사용자가 "긴급하지 않다" 판단. 본 plan 으로 직행 시 이점:
- 1 라인 hot fix 후 UX plan 으로 또 변경 → 불필요한 중간 상태
- UX plan 자체가 10~20 LOC 수준이라 hot fix 대비 비용 크게 높지 않음
- regression 보호 (stale ID 케이스 + toast UX) 를 한 PR 에 묶음

### Open questions

1. **Q-1 (Option B vs C 최종 확정)**: 본 plan 은 Option B 를 기본 스펙으로 작성. 사용자가 Option C (완전 수동) 를 원하면 §2 의 tentative UI 를 "제안 값 표시 없음 + 빈 체크박스" 로 변경. 최종 결정은 Developer 가 UI 구현 초안 후 사용자 시연 + 확정.

2. **Q-2 (toast duration)**: 현재 missing 토스트는 8초 (`roleAssignments.ts:132`). 추천 구성 적용 토스트는 사용자가 클릭할 시간이 필요하므로 유지 혹은 10초 확대.

3. **Q-3 (metaAgent 자동 생성 plan 과의 상호작용)**: 향후 메타에이전트가 plan 을 자동 생성 → RoleCoveragePanel 의 inferred 도 메타에이전트 출력으로 대체될 가능성. 본 plan 의 "추천 구성 적용" 버튼은 동일 UX 로 재활용 가능하므로 충돌 없음.

---

## Subtask 구조

| # | 파일 | 범위 | 의존 |
|---|---|---|---|
| 01 | [-task-01.md](./roleAssignmentCoverageUxPlan-task-01.md) | `roleAssignments.ts` API 확장 (`pruneStaleIds`, `applyInferredAssignments`) + `assertRoleReady` 액션 개선 + unit tests | — |
| 02 | [-task-02.md](./roleAssignmentCoverageUxPlan-task-02.md) | `AgentsSection.tsx` RoleCoveragePanel 재설계 (tentative 표시 + "추천 구성 적용" 버튼 + stale toast) + Vitest | 01 |

2 subtask. UX 플로우가 간단해서 3개로 쪼갤 필요 없음.

---

## 관련 문서

- 버그 리포트 맥락: 본 세션 2026-04-22 "Reviewer (≥2) 프로필이 설정되지 않았습니다" 토론
- RT 진입 경로: `src/lib/workflow/reviewWorkflow.ts` L66 (`reviewers.length === 0` 체크)
- 유사 패턴 참조: `src/components/tunaflow/chat/PlanProposalCard.tsx` (tentative 배지 UI)
