---
title: i18n Track 2 — A2-E Context panel tabs (축소)
created_at: 2026-04-24
parallel_track: 2 of 3
ssot: docs/plans/i18nCompletionPlan_2026-04-24.md §"Slice A2-E"
---

# Developer Session Prompt — A2-E Context panel tabs

> **Plan 원본 대비 축소**: InsightPanel / IdentityView 는 2026-04-24 세션 PR #167 에서 이미 i18n 완결. insight namespace 도 신설 완료. 본 Track 은 **남은 7 파일** 만 처리.

새 Claude Code 세션에 아래 블록 전체를 붙여넣는다.

```
[작업] i18n PR A2-E — Context panel tabs 전환 (축소)

[SSOT]
- docs/plans/i18nCompletionPlan_2026-04-24.md §"Slice A2-E"
- 현재 main 이 기준. 2026-04-24 세션에서 insight namespace / InsightPanel / IdentityView 이미 완결 (PR #167).

[브랜치]
main 에서 feat/i18n-pr-a2e-context 생성.

[범위 — 7 파일 / ~140 Korean lines / ~100 keys 예상]
대상 (Plan 대비 InsightPanel/IdentityView 제외):
- src/components/tunaflow/context-panel/PlansPanel.tsx (15 lines)
- src/components/tunaflow/context-panel/TracePanel.tsx (2 lines — 거의 없음, 확인 후 포함 여부)
- src/components/tunaflow/context-panel/QualityDashboard.tsx (17 lines)
- src/components/tunaflow/context-panel/SkillsPanel.tsx (8 lines)
- src/components/tunaflow/context-panel/HarnessSummary.tsx (4 lines)
- src/components/tunaflow/context-panel/insight/insightConstants.tsx (6 lines — CATEGORY_META.label)
- src/components/tunaflow/context-panel/PlanDocumentModal.tsx (5 lines)
- src/components/tunaflow/context-panel/SubtaskReviewView.tsx (85 lines — 가장 큰 파일)

실측 시 주석/코드 로직 구분 필수 (예: TracePanel 2 라인 = 주석만일 수 있음).

[Namespace]
기존 insight 확장 + 신규:
- trace.* (신규)
- quality.* (신규)
- skills.* (신규)
- harness.* (신규)
- insight.category.* (기존, insightConstants.label 이관)
- workflow.plan.* (기존, PlanDocumentModal / SubtaskReviewView / PlansPanel 일부 재사용 가능)

[중요 — insightConstants.tsx CATEGORY_META 처리]
- label 필드는 "UI 노출 label"
- key (stability/test/...) 는 "영어 고정 identifier" — 에이전트 프롬프트 카테고리 키로 사용 (INV-1)
- 두 축 구분: label 제거 or t() 래핑, key 는 불변

2026-04-24 세션 PR #167 에서 `insight.category.{stability|test|...}` 키 이미 생성됨. insightConstants 에서는 label 필드를 제거하거나 사용처에서 `t(\`category.\${k}\`)` 로 override.

[패턴 준수]
기존 A2-E #165 / A2-G #167 참고.
- useTranslation("<namespace>")
- 3계층 키
- ko SSOT, en 번역

[신규 namespace 등록 필요]
trace / quality / skills / harness 는 신규. 추가 작업:
1. src/locales/{ko,en}/trace.json 신규
2. src/locales/{ko,en}/quality.json 신규
3. src/locales/{ko,en}/skills.json 신규
4. src/locales/{ko,en}/harness.json 신규
5. src/locales/index.ts 에 import + resources + ns 배열 등록
6. src/types/i18next.d.ts 에 declare module 추가

[INV]
- INV-1 (agent 프롬프트 영어 고정) — 본 Track UI 만
- INV-5 (3계층)
- INV-7 (settings/* 는 Track 1 전용, chat/branch/common 은 Track 3 전용. 본 Track 은 context-panel/* 만)

[검증]
- npx tsc --noEmit
- npx vitest run
- 수동: Plans / Trace / Quality / Skills / Harness / PlanDocumentModal / SubtaskReview 각 탭 ko ↔ en

[주의 — Track 충돌 방지]
- src/locales/index.ts 편집은 Track 1/3 와 **동시 편집 시 merge conflict 가능**. 본 Track 이 가장 많은 namespace 추가하므로 Track 1/3 rebase 시 본 Track 편집분 반영.

[커밋/PR]
feat(i18n): PR A2-E — Context panel tabs (~100 keys)
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

## 참고 — 이 Track 이 아닌 것

- Settings subpanels (Track 1)
- Chat/Branch/Input/Common UI (Track 3)
- Plan/Workflow cards (이미 완결 — PR #165/#168)
- InsightPanel/IdentityView (이미 완결 — PR #167)
