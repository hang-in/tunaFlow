---
title: Flexbox `min-h-0` Audit Result (2026-04-25)
updated_at: 2026-04-25
canonical: false
status: snapshot
owner: tunaFlow-core
related:
  - docs/plans/chatPanelMinHeightCascadePlan_2026-04-25.md  # SSOT
  - docs/reference/flexboxConventions.md
---

# 목적

`docs/plans/chatPanelMinHeightCascadePlan_2026-04-25.md` Layer B 의 산출물.
`flex-col + flex-1` 자식 중 `min-h-0` 누락 케이스를 전수 점검하고 수정/보류를 분류.

# 절차

```bash
rg -n "flex-1\b" src/components/ --type tsx | grep -v "min-h-0" | grep -v "shrink"
rg -n "className=\"flex-1 overflow-y-auto" src/components/ --type tsx
rg -n "flex-col[^\"]*flex-1|flex-1[^\"]*flex-col" src/components/ --type tsx
```

184 개 hit 중 `flex-col` 컨테이너의 `flex-1` 자식 또는 column 안 `flex-1 overflow-y-auto` 만 1차 추출 → 14개 후보.

# 결과

## 수정 (Layer A — ChatPanel cascade primary)

| 위치 | before | after |
|---|---|---|
| `src/components/tunaflow/ChatPanel.tsx:190` | `flex flex-col flex-1 min-w-0 bg-background ...` | `+ min-h-0` |
| `src/components/tunaflow/ChatPanel.tsx:205` | `flex flex-col flex-1 min-w-0 overflow-hidden` | `+ min-h-0` |
| `src/components/tunaflow/ChatPanel.tsx:215` | `flex-1 overflow-hidden relative` | `+ min-h-0` |

## 수정 (Layer B — sibling master-detail column 안의 flex-1 overflow-y-auto)

ArtifactDetail / Insight right-panel 의 column 안 `flex-1 overflow-y-auto` 자식. 부모 row 의 `min-h-0` cascade 가 column 의 자식까지는 자동으로 전달되지 않으므로 자식에도 명시.

| 위치 | before | after |
|---|---|---|
| `src/components/tunaflow/context-panel/ReviewPanel.tsx:197` | `flex-1 overflow-y-auto px-4 py-4 space-y-4` | `+ min-h-0` |
| `src/components/tunaflow/context-panel/ArtifactsPanel.tsx:256` | `flex-1 overflow-y-auto px-4 py-3` | `+ min-h-0` |
| `src/components/tunaflow/context-panel/InsightPanel.tsx:623` | `flex-1 overflow-y-auto p-3` | `+ min-h-0` |

## 보류 (N/A — isolated layout, plan→dev cascade 와 무관)

다음 hit 들은 본 plan 의 #191 trigger 와 직접 관련 없는 isolated layout (모달 / 드롭다운 / Notes tab / MetaFloating chat). `overflow-y-auto` 가 max-height 를 강제하므로 현재 사용 시나리오에서 푸터 밀림 표면화 안 됨. 별도 plan 후보로만 기록하고 본 PR 스코프에서 제외.

| 위치 | 이유 |
|---|---|
| `TraceModal.tsx:58` | 모달 — viewport 고정 height 안에서 자체 스크롤 |
| `NotificationBell.tsx:156` | 드롭다운 — 자체 max-height 클래스 |
| `PlanDocumentModal.tsx:98` | 모달 — viewport 고정 height |
| `IdentityView.tsx:102` | 자체 root, 부모 panel 이 height 보장 |
| `CenterPanel.tsx:251` | Workflow tab — `overflow-y-auto` 로 max-height 강제됨, 현재 표면화 미관찰 |
| `CenterPanel.tsx:335` | Notes tab — 동일 |
| `MetaFloatingChat.tsx:532` | floating panel 자체 height fixed |

## 의도 불명 (검증 후 그대로 유지)

`ReviewPanel.tsx:264` — `<div className="flex min-h-0">` (root, `flex-col` 없이 `flex` 만, `flex-row` default).

- 부모는 CenterPanel:251 `flex-1 overflow-y-auto p-5` (column flow).
- 본인은 row container, `flex-1` 도 없이 height auto. `min-h-0` 자체가 효과 약함.
- 그러나 **유해하지 않고** 의도된 안전 토큰일 가능성 (예: 미래 height 강제 시 흐름 보장).
- 본 PR 에서 변경 없음.

## 의도된 OK (변경 불필요, 참고)

이미 `min-h-0` 명시된 column flex chain — 정상 cascade.

- `CenterPanel.tsx:223 / 226 / 293` — `flex-1 min-h-0 ...`
- `Sidebar.tsx:71 / 380 / 502 / 550` — `... min-h-0`
- `MetaFloatingChat.tsx:518 / 574` — `... min-h-0`
- `MetaAgentSelector.tsx:150` — `... min-h-0`
- `ProjectOnboardingModal.tsx:340` — `... min-h-0`
- `InsightPanel.tsx:368 / 479` — `... min-h-0`
- `ArtifactsPanel.tsx:477` — `flex-1 flex min-h-0`
- `TerminalFloatingPanel.tsx:287` — `flex-1 min-h-0 ...`

# 후속

- (lint) `lintFlexMinHeightAutomationPlan` (가칭, plan 의 Layer C) — ESLint custom rule 자동 검출. 위 N/A 케이스 (모달 / 드롭다운) 에 false-positive 가 발생하지 않도록 ignore comment 패턴 도 같이 설계.
- (audit) 새 master-detail column layout 추가될 때마다 본 보고서 갱신 (또는 lint rule 으로 대체).
