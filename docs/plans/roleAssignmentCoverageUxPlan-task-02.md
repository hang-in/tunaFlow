# Subtask 02 — `AgentsSection.tsx` RoleCoveragePanel 재설계

> 상위 plan: [roleAssignmentCoverageUxPlan.md](./roleAssignmentCoverageUxPlan.md)

## Changed files

- `src/components/tunaflow/settings/AgentsSection.tsx` — RoleCoveragePanel useEffect 재작성 + tentative 배지 렌더 + "추천 구성 적용" 버튼.
- `src/components/tunaflow/settings/AgentsSection.test.tsx` (신규 또는 확장) — component tests.

## Change description

### 1. useEffect 재작성 (stale pruning + inferred 제안 상태)

`AgentsSection.tsx:192-205` 현재 로직 대체:

```tsx
import {
  loadRoleAssignments, saveRoleAssignments,
  pruneStaleIds, inferRoleAssignments, applyInferredAssignments,
  evaluateCoverage,
  type RoleAssignments, type RoleCoverage, type RoleKey,
} from "@/lib/roleAssignments";
import { toast } from "sonner";

function RoleCoveragePanel({ profiles }: { profiles: AgentProfile[] }) {
  const [assignments, setAssignments] = useState<RoleAssignments>({ reviewers: [] });
  const [isInferred, setIsInferred] = useState(false);   // ← 신규
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let alive = true;
    loadRoleAssignments().then((saved) => {
      if (!alive) return;

      // (a) stale ID 자동 정리
      const { pruned, droppedCount } = pruneStaleIds(saved, profiles);
      if (droppedCount > 0) {
        saveRoleAssignments(pruned);
        toast.info(`삭제된 프로필 ${droppedCount}개를 역할 배정에서 제거했습니다`, { duration: 4000 });
      }

      // (b) saved 가 완전히 비어있고 profiles 가 있으면 inferred 를 제안 상태로 표시
      const isEmpty = !pruned.architect && !pruned.developer && pruned.reviewers.length === 0 && !pruned.synthesizer;
      if (isEmpty && profiles.length > 0) {
        setAssignments(inferRoleAssignments(profiles));
        setIsInferred(true);
      } else {
        setAssignments(pruned);
        setIsInferred(false);
      }
      setLoaded(true);
    });
    return () => { alive = false; };
  }, [profiles]);   // profiles 변경 시 재평가 (기존 .length 만 watch 하던 것을 확장)
  ...
}
```

### 2. tentative 배지 + "추천 구성 적용" 버튼

RoleCoveragePanel 헤더 아래:

```tsx
{isInferred && (
  <div className="flex items-center gap-2 p-2 rounded-md bg-amber-500/10 border border-amber-500/30 mb-2">
    <AlertTriangle className="w-4 h-4 text-amber-500 shrink-0" />
    <span className="text-tf-sm text-foreground flex-1">
      추천 구성이 제안되어 있지만 아직 저장되지 않았습니다
    </span>
    <button
      onClick={async () => {
        const applied = await applyInferredAssignments(profiles);
        setAssignments(applied);
        setIsInferred(false);
        toast.success("역할 구성이 저장되었습니다");
      }}
      className="px-3 py-1 rounded bg-amber-500 text-white text-tf-sm font-medium hover:bg-amber-600"
    >
      추천 구성 적용
    </button>
  </div>
)}
```

### 3. RoleRow / ReviewersRow tentative prop

기존 `RoleRow`, `ReviewersRow` 에 `tentative?: boolean` prop 추가:

```tsx
function RoleRow({ label, coverage, profiles, selectedId, onChange, tentative }: {
  label: string;
  coverage: RoleCoverage;
  profiles: AgentProfile[];
  selectedId: string;
  onChange: (id: string) => void;
  tentative?: boolean;   // ← 신규
}) {
  return (
    <div className={cn("flex items-center gap-2 text-tf-sm", tentative && "opacity-60")}>
      {statusIcon(coverage.status)}
      <span className="w-[90px] text-muted-foreground">{label}</span>
      <select
        value={selectedId}
        onChange={(e) => onChange(e.target.value)}
        className="flex-1 bg-background rounded px-2 py-1 text-tf-sm outline-none border border-border/30"
      >
        <option value="">(선택 안됨)</option>
        {profiles.map((p) => (
          <option key={p.id} value={p.id}>{p.label}</option>
        ))}
      </select>
      {tentative && (
        <span className="px-2 py-0.5 rounded-full bg-amber-500/20 text-amber-700 text-[10px] font-medium">
          제안됨
        </span>
      )}
    </div>
  );
}
```

사용자가 select 변경 (또는 reviewer 체크박스 토글) → `isInferred=false` 로 전이 + `saveRoleAssignments` 호출 (기존 `updateAndSave` / `toggleReviewer` 로직 유지). tentative prop 은 자동 해제:

```tsx
const setSingle = (role: Exclude<RoleKey, "reviewers">, value: string) => {
  const next = { ...assignments, [role]: value || undefined };
  setAssignments(next);
  setIsInferred(false);                // ← 추가
  saveRoleAssignments(next);
};

const toggleReviewer = (profileId: string) => {
  const has = assignments.reviewers.includes(profileId);
  const reviewers = has
    ? assignments.reviewers.filter((id) => id !== profileId)
    : [...assignments.reviewers, profileId];
  const next = { ...assignments, reviewers };
  setAssignments(next);
  setIsInferred(false);                // ← 추가
  saveRoleAssignments(next);
};
```

### 4. RoleRow/ReviewersRow 호출부에 prop 전달

```tsx
<RoleRow label="Architect" coverage={coverage[0]!} profiles={profiles}
  selectedId={assignments.architect ?? ""} onChange={(v) => setSingle("architect", v)}
  tentative={isInferred} />
<RoleRow label="Developer" coverage={coverage[1]!} profiles={profiles}
  selectedId={assignments.developer ?? ""} onChange={(v) => setSingle("developer", v)}
  tentative={isInferred} />
<ReviewersRow coverage={coverage[2]!} profiles={profiles}
  selectedIds={assignments.reviewers} onToggle={toggleReviewer}
  tentative={isInferred} />
<RoleRow label="Synthesizer" coverage={coverage[3]!} profiles={profiles}
  selectedId={assignments.synthesizer ?? ""} onChange={(v) => setSingle("synthesizer", v)}
  tentative={isInferred} />
```

`ReviewersRow` 내부도 동일 패턴 — 체크박스 opacity-60 + "제안됨" 배지.

## Dependencies

depends_on: [01]

## Verification

- Component test (`AgentsSection.test.tsx`):
  ```tsx
  it("shows '추천 구성 적용' banner when saved roleAssignments is empty and profiles exist", async () => {
    mockGetSetting.mockResolvedValue({ reviewers: [] });   // saved = empty
    const profiles = [
      { id: "arch-1", personaId: "persona_architect", label: "arch" },
      { id: "rev-1", personaId: "persona_reviewer", label: "reviewer-1" },
      { id: "rev-2", personaId: "persona_reviewer", label: "reviewer-2" },
    ];
    render(<AgentsSection profiles={profiles} />);
    expect(await screen.findByText(/추천 구성이 제안되어/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /추천 구성 적용/ })).toBeInTheDocument();
  });

  it("hides banner and persists after clicking '추천 구성 적용'", async () => {
    // ... arrange ...
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /추천 구성 적용/ }));
    expect(mockSetSetting).toHaveBeenCalledWith("roleAssignments", expect.objectContaining({
      reviewers: expect.arrayContaining(["rev-1", "rev-2"]),
    }));
    expect(screen.queryByText(/추천 구성이 제안되어/)).not.toBeInTheDocument();
  });

  it("INV-1: isInferred=true 상태에서 saved 저장소는 empty 유지", async () => {
    render(<AgentsSection profiles={profiles} />);
    await screen.findByText(/추천 구성이 제안되어/);  // 제안 상태 확인
    // 버튼 클릭 안 한 상태에서 loadRoleAssignments 재호출 시 empty
    expect(await loadRoleAssignments()).toEqual({ reviewers: [] });
  });

  it("INV-2: stale ID 감지 시 toast + 자동 정리", async () => {
    mockGetSetting.mockResolvedValue({
      architect: "gone", developer: undefined, reviewers: ["gone2", "rev-1"], synthesizer: undefined,
    });
    const profiles = [{ id: "rev-1", personaId: "persona_reviewer", label: "r1" }];
    render(<AgentsSection profiles={profiles} />);
    await waitFor(() => {
      expect(mockSetSetting).toHaveBeenCalledWith("roleAssignments", expect.objectContaining({
        architect: undefined,
        reviewers: ["rev-1"],
      }));
    });
    // toast.info 호출 검증 (mock sonner)
  });

  it("manual toggle dismisses isInferred flag", async () => {
    render(<AgentsSection profiles={profilesWithEmptySaved} />);
    await screen.findByText(/추천 구성이 제안되어/);
    await user.selectOptions(screen.getByLabelText("Architect"), "arch-1");
    expect(screen.queryByText(/추천 구성이 제안되어/)).not.toBeInTheDocument();
  });
  ```
- `npx vitest run src/components/tunaflow/settings/AgentsSection.test.tsx` — 모두 pass.
- `npx tsc --noEmit` — exit 0.
- 수동 E2E:
  1. 신규 프로필 2개 생성 (persona_reviewer)
  2. Settings 닫고 다시 열기 → "추천 구성이 제안되어 있지만 아직 저장되지 않았습니다" 배너 노출
  3. "추천 구성 적용" 클릭 → 배너 사라지고 체크박스 opacity 정상
  4. RT 진입 → 에러 없음 + 정상 진입

## Risks

- **`profiles` 의존성 변경**: 기존 `[profiles.length]` → `[profiles]` 확장은 매 profile 편집 시 재평가. inferred 제안 상태가 매번 재계산되어 사용자 경험상 깜박임 가능. 완화: dep 를 `[profiles.length, profiles.map(p=>p.id).join(',')]` 로 stable hash 사용 또는 별도 비교.
- **Toast 중복**: stale pruning 이 발동하는 동시에 RT 진입이 일어나면 같은 toast 2회 가능. sonner 의 id 기반 dedup (`toast.info(..., { id: 'role-prune' })`) 로 방어.
- **Option C 로 변경 요청**: 본 구현은 Option B (tentative 표시 + 원클릭) 기반. 사용자 시연 후 Option C 로 변경 요청 시 tentative 블록 전체를 "제안 표시 없음 + 빈 select" 로 simplification — 15 LOC 이하 변경으로 가능.
- **Persona 명이 바뀐 경우**: `inferRoleAssignments` 는 `persona_reviewer` / `persona_architect` / `persona_implementer` 하드코딩. 기존 Persona 의 ID 가 변경되면 infer 결과 비어짐 → "추천 구성 적용" 버튼 노출 안 됨. 이는 기존 동작과 동일 (현재도 label 기반 fallback 있음).
