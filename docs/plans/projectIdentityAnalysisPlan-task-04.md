# Subtask 04 — Insight 탭 "정체성 뷰" UI

> 상위 plan: [projectIdentityAnalysisPlan.md](./projectIdentityAnalysisPlan.md)

## Changed files

- `src/components/tunaflow/context-panel/IdentityView.tsx` (신규) — 최신 identity_summary 렌더 + 변곡점 timeline + diff 뷰.
- `src/components/tunaflow/context-panel/InsightPanel.tsx` (또는 상위 라우터) — Identity 탭 엔트리 추가.
- `src/lib/api/identityAnalysis.ts` (신규) — FE wrapper (list/fetch/trigger).
- `src/lib/parseIdentitySummary.ts` (신규) — markdown 섹션 파서.
- `src/types/identity.ts` (신규) — type 정의.

## Change description

### 1. FE API wrapper

```ts
// src/lib/api/identityAnalysis.ts
import { invoke } from '@tauri-apps/api/core';

export type IdentitySummary = {
    id: string;
    title: string;
    content: string;             // frontmatter + sections markdown
    created_at: number;
    // frontmatter parsed
    projectKey: string;
    periodStart: number;
    periodEnd: number;
    artifactRefs: string[];
    supersedes?: string;
};

export function listIdentitySummaries(projectKey: string): Promise<IdentitySummary[]> {
    return invoke('list_identity_summaries', { projectKey });
}

export function fetchIdentitySummary(id: string): Promise<IdentitySummary> {
    return invoke('get_artifact', { id }).then(parseIdentitySummary);
}

export function triggerIdentityAnalysis(projectKey: string, force = false) {
    return invoke('trigger_identity_analysis_now', { projectKey, force });
}

export function fetchArtifactRefs(ids: string[]): Promise<Artifact[]> {
    return Promise.all(ids.map(id => invoke<Artifact>('get_artifact', { id })));
}
```

### 2. Section parser

```ts
// src/lib/parseIdentitySummary.ts
export type ParsedIdentity = {
    frontmatter: {
        projectKey: string;
        periodStart: number;
        periodEnd: number;
        artifactRefs: string[];
        supersedes?: string;
    };
    sections: {
        projectIdentity: string;
        userWorkingPreference: string;
        agentOperatingPreference: string;
        inflectionPoints: Array<{ what: string; why: string; when: string; artifactId?: string }>;
        doAvoid: { do: string[]; avoid: string[] };
    };
};

export function parseIdentitySummary(content: string): ParsedIdentity {
    const fm = extractFrontmatter(content);
    const body = stripFrontmatter(content);

    // section header 기준 split
    const sections = splitBySections(body, [
        "### Project identity",
        "### User working preference",
        "### Agent operating preference",
        "### Recent inflection points",
        "### Do / Avoid",
    ]);

    return {
        frontmatter: fm,
        sections: {
            projectIdentity: sections[0]?.trim() ?? "",
            userWorkingPreference: sections[1]?.trim() ?? "",
            agentOperatingPreference: sections[2]?.trim() ?? "",
            inflectionPoints: parseInflectionPoints(sections[3] ?? ""),
            doAvoid: parseDoAvoid(sections[4] ?? ""),
        },
    };
}
```

Parser 는 best-effort — 섹션 누락 시 빈 문자열 / 빈 배열. UI 측에서 "(데이터 없음)" 렌더.

### 3. IdentityView 컴포넌트

```tsx
// src/components/tunaflow/context-panel/IdentityView.tsx
import { useState, useEffect } from "react";
import { ChevronRight, RefreshCw, AlertTriangle, Clock } from "lucide-react";
import { listIdentitySummaries, triggerIdentityAnalysis, fetchArtifactRefs } from "@/lib/api/identityAnalysis";
import { parseIdentitySummary } from "@/lib/parseIdentitySummary";

export function IdentityView({ projectKey }: { projectKey: string }) {
    const [summaries, setSummaries] = useState<IdentitySummary[]>([]);
    const [selected, setSelected] = useState<IdentitySummary | null>(null);
    const [busy, setBusy] = useState(false);
    const [triggerResult, setTriggerResult] = useState<IdentityTriggerDecision | null>(null);

    useEffect(() => {
        listIdentitySummaries(projectKey).then((list) => {
            setSummaries(list);
            setSelected(list[0] ?? null);
        });
    }, [projectKey]);

    const parsed = selected ? parseIdentitySummary(selected.content) : null;

    const handleManualRun = async (force: boolean) => {
        setBusy(true);
        try {
            const decision = await triggerIdentityAnalysis(projectKey, force);
            setTriggerResult(decision);
            // 결과 완료되면 identity_analysis_completed 이벤트 리스너가 list 재로드
        } finally {
            setBusy(false);
        }
    };

    if (!selected) {
        return (
            <EmptyState>
                <p>아직 정체성 분석 결과가 없습니다.</p>
                <p className="hint">
                    Plan 3개 완료 + eligible artifact 10개 누적 시 자동 실행됩니다.
                </p>
                <button onClick={() => handleManualRun(true)} disabled={busy}>
                    강제 분석 실행
                </button>
                {triggerResult && <TriggerStatus result={triggerResult} />}
            </EmptyState>
        );
    }

    return (
        <div className="identity-view">
            <Header summary={selected} summaries={summaries} onSelect={setSelected} />
            <RunButton onClick={handleManualRun} busy={busy} />

            <Section title="Project identity" content={parsed!.sections.projectIdentity} defaultOpen />
            <Section title="User working preference" content={parsed!.sections.userWorkingPreference} />
            <Section title="Agent operating preference" content={parsed!.sections.agentOperatingPreference} />
            <InflectionPointsTimeline points={parsed!.sections.inflectionPoints} />
            <DoAvoidLists items={parsed!.sections.doAvoid} />

            {parsed!.frontmatter.supersedes && (
                <DiffLink prevId={parsed!.frontmatter.supersedes} currentId={selected.id} />
            )}
            <ArtifactRefsLink refs={parsed!.frontmatter.artifactRefs} />
        </div>
    );
}
```

### 4. 하위 컴포넌트

- **InflectionPointsTimeline**: 3 변곡점을 시간순 카드로. 각 카드에 artifact id → 클릭 시 원본 artifact 조회 (side drawer).
- **DoAvoidLists**: "Do" 와 "Avoid" 리스트를 색으로 구분 (녹색/빨강 subtle).
- **DiffLink**: 이전 summary 와 이번 summary 의 섹션별 diff. 문자열 diff 라이브러리 없이 단순 비교 (추후 `diff-match-patch` 도입 가능).
- **ArtifactRefsLink**: frontmatter 의 `artifactRefs` 개수 + 펼치면 list. Read-only.
- **TriggerStatus**: decision 결과 (done_plan_count / eligible_artifact_count / threshold / reason) 렌더.

### 5. Insight 탭 통합

`InsightPanel` 상단 탭 구조 (기존 Insight 6 카테고리) 옆에 **"Identity"** 탭 추가:

```tsx
<Tabs>
  <Tab id="insights">Insight</Tab>
  <Tab id="identity">Identity</Tab>
</Tabs>
{active === "identity" && <IdentityView projectKey={projectKey} />}
```

## Dependencies

depends_on: [03] — identity_summary artifact 가 생성돼야 list 에 내용이 있음.

## Verification

- `npx vitest run src/lib/parseIdentitySummary.test.ts`:
  - 정상 5 섹션 markdown → 모든 섹션 비어있지 않음
  - 1 섹션 누락 → 해당 섹션만 "" 반환 + 나머지 정상
  - 전부 누락 → 모든 섹션 "" + throw X
  - frontmatter 파싱 (projectKey / periodStart / periodEnd / artifactRefs[])
- `npx vitest run src/components/tunaflow/context-panel/IdentityView.test.tsx`:
  - summary 없을 때 EmptyState + "강제 분석 실행" 버튼
  - summary 있을 때 5 섹션 렌더
  - "지금 분석" 버튼 클릭 → `trigger_identity_analysis_now` invoke
- Manual E2E:
  1. subtask-03 실행 후 identity_summary 1건 생성됨
  2. Insight 탭 > Identity 탭 진입
  3. 5 섹션 렌더 확인 (Project identity / User working / Agent operating / Inflection points / Do/Avoid)
  4. 변곡점 카드 클릭 → artifact drawer 열림
  5. "강제 분석 실행" 클릭 → 새 identity_summary 생성 → list 자동 업데이트

## Risks

- **Parser 견고성**: LLM 이 section header 를 정확히 "### Project identity" 로 쓰지 않을 수 있음 (e.g. "## Project Identity" 또는 번역). subtask-03 의 validator 가 이를 강제하지만 legacy summary (구버전 LLM 출력) 가 있으면 parser fail. fallback: 섹션 parse 실패 시 raw markdown 전체를 single section 으로 렌더.
- **LLM 영어 출력 → 한국어 사용자 UX**: identity_summary 는 영어 고정 (i18nPlan + subtask-03). 한국어 사용자에게 UI 는 i18n 적용하되 본문은 영어 그대로 렌더. 후속에서 client-side 번역 (Settings 토글) 고려.
- **Diff 계산 비용**: 전체 summary 2K tokens × 2개 diff = 클라이언트 상에서 충분히 빠름. 단 "diff-match-patch" 같은 lib 도입 전까지는 naive line-by-line diff.
- **Event subscribe 누수**: `identity_analysis_completed` 이벤트 리스너를 useEffect cleanup 에서 반드시 unlisten. 잊으면 listener 누적.
- **Empty state 에서의 UX**: 프로젝트 초기 사용자는 "왜 비어있지?" 혼란. EmptyState 에 "Plan 3개 완료 + artifact 10개 누적" 조건 + 현재 카운트 표시 권장 (subtask-02 의 TriggerStatus 재활용).
- **강제 실행 남용**: 사용자가 "강제 분석" 을 자주 누르면 LLM 호출 비용 누적. 1 분 쿨다운 또는 "최근 실행 표시" 로 UX 가이드. 강제는 주로 디버깅/실험 용도.
