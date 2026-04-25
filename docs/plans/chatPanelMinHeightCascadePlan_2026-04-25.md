---
title: ChatPanel 내부 `min-h-0` cascade 누락 — plan→dev 전이 시 푸터 밀림 (#191 후속)
status: ready-to-implement (dev 종료 후)
priority: P1 (사용자 가시 UX 결함, #191 fix 가 cascade 까지 못 가서 재발)
created_at: 2026-04-25
related:
  - https://github.com/hang-in/tunaFlow/issues/191  # 같은 카테고리 1차 fix
  - https://github.com/hang-in/tunaFlow/pull/192    # AppShell main flex 의 min-h-0
  - src/components/tunaflow/ChatPanel.tsx
  - src/components/tunaflow/context-panel/PlansPanel.tsx
  - src/components/tunaflow/context-panel/DevProgressView.tsx
canonical: true
---

# 증상 (사용자 보고, 2026-04-25)

> "plan check → dev (브랜치 드로어 열림) 만 해도 푸터 아래가 밀려 올라가서 화면이 깨지는데?"

PR #192 로 `AppShell:186` (main flex) + `AppShell:193` (CenterPanel wrapper) + `BranchThreadPanel:293` 에 `min-h-0` 추가했지만, **ChatPanel 내부 layer 까지 cascade 안 됨**. plan check 단계에서 dev 단계로 phase 전이 시 PlansPanel 안의 DevProgressView 가 mount/swap 되면서 그 안의 grow 가 ChatPanel container 를 stretch.

# 현재 상태 (사실 확인)

## (A) `src/components/tunaflow/ChatPanel.tsx` (line 190~217)

```tsx
// line 190 — empty conv state
<div className="flex flex-col flex-1 min-w-0 bg-background items-center justify-center">
  // ❌ min-h-0 누락

// line 205 — main wrapper
<div className="flex flex-col flex-1 min-w-0 overflow-hidden">
  // ❌ min-h-0 누락 (overflow-hidden 만으로 일부 케이스 방어, 모든 브라우저 보장 X)

  // line 211 — error banner (shrink-0 OK)

  // line 215 — message area
  <div className="flex-1 overflow-hidden relative">
    // ❌ min-h-0 누락
    <div className="h-full overflow-y-auto">  // line 217
```

**3 위치 누락**. 어제 #191 의 cascade 가 ChatPanel 까지 못 옴.

## (B) PlansPanel + DevProgressView

`src/components/tunaflow/context-panel/PlansPanel.tsx:213`:
```tsx
<DevProgressView plan={plan} onPlanUpdate={handlePlanUpdated} />
```

DevProgressView 는 phase=dev 시점 mount. 그 안의 subtask UI / Reviewer 선택 / verdict card 등 multiple 컴포넌트 동적 grow. 부모 (PlansPanel → ChatPanel) 의 `min-h-0` 없으면 stretch.

## (C) 다른 잠재 영역 (Developer audit 대상)

`rg "flex-1.*min-w-0" src/components/` 결과 vs `rg "flex-1.*min-w-0.*min-h-0" src/components/` 결과 비교 → 누락 위치 전수.

특히 의심:
- `ReviewPanel.tsx:264` — `flex min-h-0` 있는데 `flex-col` 명시 없음 (default row). 의도 불명, 검증 필요
- 기타 `context-panel/*.tsx` 의 nested flex
- `MessageList` / `MessageItem` 컨테이너

# 수정

## Layer A — ChatPanel 3 위치 `min-h-0` 추가 (Primary)

**파일**: `src/components/tunaflow/ChatPanel.tsx`

```tsx
// line 190
<div className="flex flex-col flex-1 min-w-0 min-h-0 bg-background items-center justify-center">

// line 205
<div className="flex flex-col flex-1 min-w-0 min-h-0 overflow-hidden">

// line 215
<div className="flex-1 min-h-0 overflow-hidden relative">
```

## Layer B — Audit grep + 누락 cascade fix

```bash
# 누락 위치 전수
rg -n "flex-1\b" src/components/ --type tsx | grep -v "min-h-0" | grep -v "shrink"
```

각 hit 별로:
- 부모가 flex-col 인지 확인 (flex-row 면 min-h-0 무관)
- Height 강제 (`h-full`, `h-screen`) 가 외부에서 이미 보장되는지
- 누락이 의도된 건지 (예: 작은 inline 컨테이너)

수정 대상만 `min-h-0` 추가.

## Layer C — Lint rule (선택, 후속)

ESLint custom rule 또는 stylelint 로 `flex-col` + `flex-1` 조합에서 `min-h-0` 누락 자동 검출. 변경 표면 큼 → 별 plan 후보.

## Layer D — Documentation invariant

`docs/reference/frontendArchitecture.md` (또는 신규 `docs/reference/flexboxConventions.md`) 에:

> **Tailwind flexbox invariant**: `flex flex-col` + `flex-1` 자식이면 그 자식에 `min-h-0` 필수. `min-w-0` 만 챙기는 흔한 함정. `flex flex-row` 의 자식 `flex-1` 은 `min-w-0` 만.

CLAUDE.md §16 코딩 컨벤션 또는 §13 문서 참조에 reference 추가. 미래 architect / Developer 가 새 layout 추가 시 강제 준수.

# Invariants

- **[INV-1]** ChatPanel 의 3 위치 (line 190 / 205 / 215) 에 `min-h-0` 명시. 검증: PR diff
- **[INV-2]** plan→dev phase 전이 시 화면 layout 안정 (푸터 / 상태바 위치 변동 0). 검증: 수동 smoke
- **[INV-3]** PlansPanel + DevProgressView 의 phase 별 mount/unmount 가 상위 ChatPanel container 를 stretch 하지 않음. 검증: 모든 phase (drafting / dev / review / done / rework) 에서 layout 안정
- **[INV-4]** Audit grep 결과 `min-h-0` 누락된 `flex-col + flex-1` 조합 0건 (Layer B)
- **[INV-5]** flexbox invariant 가 SSOT 문서로 박혀있어 미래 layout 추가 시 강제 (Layer D)

# 검증

## 수동 Smoke (PR 필수)

1. **plan check → dev 전이**: PlanCard 에서 plan 승인 → dev phase → 드로어 자동 열림 (또는 수동) → **푸터/상태바 위치 변동 0** 확인
2. **dev 진행 중 message/응답 추가**: 메시지 누적 → ChatPanel 내부 scroll 영역만 grow, 외부 layout 고정
3. **drawer pinned vs overlay**: 두 모드에서 같은 시나리오 — 동일하게 layout 안정
4. **그 외 phase 전이** (review / done / rework) 도 같은 안정성

## 자동

ChatPanel 컴포넌트 snapshot test (있으면 보강) — DevProgressView mock + height 측정.

# Developer 핸드오프 프롬프트

```
[작업] ChatPanel 내부 min-h-0 cascade 누락 fix (#191 후속, plan→dev 전이 시 푸터 밀림)

[SSOT] docs/plans/chatPanelMinHeightCascadePlan_2026-04-25.md

[배경 3줄]
- PR #192 가 AppShell main flex 만 fix → ChatPanel 내부 cascade 안 됨
- plan→dev phase 전이 시 PlansPanel/DevProgressView mount 로 grow → ChatPanel 컨테이너 stretch → 푸터 밀림
- Tailwind flexbox 의 흔한 함정 (min-w-0 만 챙기고 min-h-0 누락)

[수정 범위]

1) Layer A — ChatPanel.tsx 3 위치 min-h-0 추가 (line 190 / 205 / 215)

2) Layer B — Audit:
   - rg "flex-1\b" src/components/ --type tsx | grep -v "min-h-0" | grep -v "shrink"
   - hit 별 부모가 flex-col 이고 height 강제 외부 보장 안 되면 min-h-0 추가
   - 결과: docs/reference/flexboxAuditResult_2026-04-2X.md (간단 보고)

3) Layer D — Documentation invariant:
   - docs/reference/flexboxConventions.md 신규 (또는 frontendArchitecture.md 에 섹션)
   - "flex-col + flex-1 자식은 min-h-0 필수" 규칙 박기
   - CLAUDE.md §16 또는 §13 에 reference

(Layer C — Lint rule 은 별 plan 후보, 본 PR 스코프 외)

[검증]
- npx tsc --noEmit
- 수동 smoke (plan §검증):
  1. plan check → dev 전이 → 푸터 위치 안정
  2. dev 진행 중 message 누적 → scroll 영역만 grow
  3. drawer pinned/overlay 두 모드 안정
  4. 모든 phase 안정성

[커밋 분리]
- fix(chat-panel): add min-h-0 to flex-col flex-1 children (Layer A)
- chore(layout): audit + remaining min-h-0 additions (Layer B)
- docs(ref): flexbox conventions + CLAUDE.md reference (Layer D)

trailer: Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

[PR 제목]
fix(layout): ChatPanel min-h-0 cascade fix — plan→dev phase footer drift (#191 follow-up)

[셀프 이슈]
"bug: footer drifts off-screen on plan→dev phase transition (#191 cascade missed ChatPanel internals)"
이슈 본문에 #191/#192 reference + plan→dev 전이 트리거 명시
```

# 셀프 이슈 본문 초안

```markdown
## Summary

PR #192 fixed `min-h-0` on AppShell main flex container (#191), but the cascade did not propagate into ChatPanel's internal flex-col children. When the plan transitions from `check` → `dev` and PlansPanel mounts DevProgressView, the new content grows past ChatPanel's allocated height, pushing the footer (NewMessageInput) and status bar off-screen.

## Reproduction

1. Open a project with an active plan
2. Plan phase = `drafting` or `check` (footer position OK)
3. Approve plan / advance to `dev` phase (drawer auto-opens or manual)
4. Observe: footer drifts upward, status bar may move off the viewport

Same symptom as #191 but a different code path — that fix didn't reach ChatPanel internals.

## Root cause

3 missing `min-h-0` in `src/components/tunaflow/ChatPanel.tsx`:

- L190: empty conversation state wrapper
- L205: main wrapper (overflow-hidden present but doesn't fully replace min-h-0)
- L215: message area

Plus likely siblings (audit needed): `context-panel/*.tsx`, possibly `ReviewPanel.tsx:264` (`flex min-h-0` without `flex-col`).

## Fix

Per `docs/plans/chatPanelMinHeightCascadePlan_2026-04-25.md`:

- Layer A: 3 ChatPanel `min-h-0` additions
- Layer B: rg audit for `flex-col + flex-1` without `min-h-0`, fix all
- Layer D: SSOT flexbox conventions doc + CLAUDE.md reference (prevent future regressions)

## Sibling

Lint rule for automatic detection — separate plan candidate (#191/#192/this issue 패턴이 반복되면 자동화 가치 큼).
```

# 후속 / Sibling

- `lintFlexMinHeightAutomationPlan` (가칭, 별 plan 후보) — ESLint custom rule 또는 stylelint 로 자동 검출
- 본 plan 의 Layer D (SSOT 문서) 가 단기 안전망. Layer C (lint) 는 장기
