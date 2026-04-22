# Subtask 01 — `roleAssignments.ts` API 확장 + `assertRoleReady` 액션 개선

> 상위 plan: [roleAssignmentCoverageUxPlan.md](./roleAssignmentCoverageUxPlan.md)

## Changed files

- `src/lib/roleAssignments.ts` — `pruneStaleIds()`, `applyInferredAssignments()` 헬퍼 신규. `assertRoleReady()` 의 toast 액션 분기 추가.
- `src/lib/roleAssignments.test.ts` (신규 또는 기존 파일 확장) — unit tests.

## Change description

### 1. `pruneStaleIds()` 추가

```ts
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
```

### 2. `applyInferredAssignments()` 추가

```ts
export async function applyInferredAssignments(
  profiles: AgentProfile[],
): Promise<RoleAssignments> {
  const inferred = inferRoleAssignments(profiles);
  await saveRoleAssignments(inferred);
  return inferred;
}
```

### 3. `assertRoleReady()` toast 분기 (INV-3)

```ts
export async function assertRoleReady(
  role: RoleKey,
  profiles: AgentProfile[],
): Promise<{ ok: boolean; coverage: RoleCoverage }> {
  const assignments = await loadRoleAssignments();
  const cov = evaluateCoverage(assignments, profiles).find((c) => c.role === role)!;

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
        duration: 10000,
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

  if (cov.status === "model-unset") {
    const { toast } = await import("sonner");
    toast.warning(cov.hint, { duration: 4000 });
  }
  return { ok: true, coverage: cov };
}
```

## Dependencies

depends_on: 없음.

## Verification

- Unit tests (`src/lib/roleAssignments.test.ts`):
  ```ts
  describe("pruneStaleIds", () => {
    it("drops ids that are no longer in profiles", () => {
      const assignments = { architect: "a1", developer: "gone", reviewers: ["r1", "gone2"], synthesizer: "s1" };
      const profiles = [{ id: "a1" }, { id: "r1" }, { id: "s1" }] as any;
      const { pruned, droppedCount } = pruneStaleIds(assignments, profiles);
      expect(pruned).toEqual({ architect: "a1", developer: undefined, reviewers: ["r1"], synthesizer: "s1" });
      expect(droppedCount).toBe(2);
    });

    it("returns droppedCount=0 when all ids valid", () => {
      const assignments = { architect: "a1", developer: undefined, reviewers: ["r1", "r2"], synthesizer: undefined };
      const profiles = [{ id: "a1" }, { id: "r1" }, { id: "r2" }] as any;
      const { pruned, droppedCount } = pruneStaleIds(assignments, profiles);
      expect(pruned).toEqual(assignments);
      expect(droppedCount).toBe(0);
    });

    it("treats undefined fields as valid (no false positive)", () => {
      const { droppedCount } = pruneStaleIds({ reviewers: [] }, []);
      expect(droppedCount).toBe(0);
    });
  });

  describe("applyInferredAssignments", () => {
    it("persists inferred result via saveRoleAssignments", async () => {
      const profiles = [
        { id: "arch-1", personaId: "persona_architect", label: "arch" },
        { id: "rev-1", personaId: "persona_reviewer", label: "reviewer-1" },
        { id: "rev-2", personaId: "persona_reviewer", label: "reviewer-2" },
      ] as any;
      const applied = await applyInferredAssignments(profiles);
      expect(applied.architect).toBe("arch-1");
      expect(applied.reviewers).toEqual(["rev-1", "rev-2"]);
      // saved 검증 — loadRoleAssignments 결과도 동일해야
      expect(await loadRoleAssignments()).toEqual(applied);
    });
  });
  ```
- `npx vitest run src/lib/roleAssignments.test.ts` — 모두 pass.
- `npx tsc --noEmit` — exit 0.

## Risks

- **`getSetting` / `setSetting` mock**: 테스트가 `appStore` 의 `getSetting`/`setSetting` 을 mock 해야 실제 DB 건드리지 않음. 기존 vitest setup 에 mock 이 있는지 확인.
- **import cycle**: `roleAssignments.ts` 가 sonner 를 동적 import (`await import("sonner")`) 로 쓰는 건 tree-shake 목적 — 유지. 상단 static import 로 바꾸지 말 것.
- **toast 중복 표시**: stale pruning + missing 동시 발생 시 toast 2 개. UX 상 허용 (각 정보가 다름). 필요 시 Subtask 02 에서 UI 상태로 dedup 가능.
