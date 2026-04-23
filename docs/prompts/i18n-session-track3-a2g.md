---
title: i18n Track 3 — A2-G Chat / Branch / Input / Common UI
created_at: 2026-04-24
parallel_track: 3 of 3
ssot: docs/plans/i18nCompletionPlan_2026-04-24.md §"Slice A2-G"
---

# Developer Session Prompt — A2-G Chat / Branch / Input / Common

> Plan 의 A2-F 는 2026-04-24 세션 PR #165/#168 에서 완결. A2-G 를 Track 3 로 앞당긴다.

새 Claude Code 세션에 아래 블록 전체를 붙여넣는다.

```
[작업] i18n PR A2-G — Chat / Branch / Input / Common UI 전환

[SSOT]
- docs/plans/i18nCompletionPlan_2026-04-24.md §"Slice A2-G"
- 현재 main 이 기준. 2026-04-24 세션에서 A2-F (Plan/Workflow cards) 이미 완결.

[브랜치]
main 에서 feat/i18n-pr-a2g-chat 생성.

[범위 — ~12 파일 / ~130 Korean lines / ~130 keys 예상]
**큰 파일 우선 (실측 많은 것):**
- src/components/tunaflow/NewMessageInput.tsx (29 lines)
- src/components/tunaflow/MetaAgentSelector.tsx (19 lines)
- src/components/tunaflow/MetaFloatingChat.tsx (29 lines)
- src/components/tunaflow/ProjectOnboardingModal.tsx (38 lines)
- src/components/tunaflow/RuntimeStatusBar.tsx (18 lines)
- src/components/tunaflow/ContextMenu.tsx (12 lines)

**중소 파일:**
- src/components/tunaflow/AppShell.tsx (5)
- src/components/tunaflow/CenterPanel.tsx (6)
- src/components/tunaflow/TerminalPanel.tsx (3)
- src/components/tunaflow/TerminalFloatingPanel.tsx (6)
- src/components/tunaflow/ChatPanel.tsx (5)
- src/components/tunaflow/ErrorBoundary.tsx (4)
- src/components/tunaflow/NotificationBell.tsx (5)

**input 하위:**
- src/components/tunaflow/input/useSendActions.ts (30 lines — INV-6 대상 있을 수 있음)
- src/components/tunaflow/input/ContextBadges.tsx (2)

**제외 (이미 거의 완료)**:
- BranchThreadPanel.tsx (2) / CreateRoundtableDialog.tsx (2) / RoundtableView.tsx (2) / MessageItem.tsx (3) — 주석만 남음 (PR #164/#166 완결)
- message/MessageActions.tsx (1)

[Namespace]
기존 확장:
- chat.* (chat / message / input)
- branch.* (이미 신설됨 — PR #166)
- dialog.* (기존, modal / onboarding 확장)
- common.* (AppShell / StatusBar / Terminal / ContextMenu / ErrorBoundary / NotificationBell)

[신규 키 예상]
- chat.input.* 확장 (NewMessageInput 29 라인)
- common.app.* (AppShell)
- common.status_bar.* (RuntimeStatusBar)
- common.terminal.* (TerminalPanel / TerminalFloatingPanel)
- common.context_menu.* (ContextMenu)
- common.error_boundary.* (ErrorBoundary)
- common.notification.* (NotificationBell)
- dialog.onboarding.* (ProjectOnboardingModal)
- dialog.meta_agent.* (MetaAgentSelector / MetaFloatingChat)

[INV-6 주의]
useSendActions.ts 에 sendMessage() 템플릿 있으면 locale 기반 i18n 대상 (chat 히스토리에 user msg 로 노출되면 → i18n, 순수 agent 지시 → 영어 고정).
30 lines 중 템플릿 vs 주석 vs 토스트 분류 필수.

[INV]
- INV-1 (agent 프롬프트 영어 고정)
- INV-5 (3계층)
- INV-6 (useSendActions 템플릿 locale i18n)
- INV-7 (settings/* 는 Track 1, context-panel/* 는 Track 2. 본 Track 은 그 외 주요 컴포넌트)

[ErrorBoundary 특례]
i18n 초기화 실패 시에도 렌더돼야 하므로 ErrorBoundary fallback 텍스트는 영어 hardcode 유지. `t()` 대신 literal string 사용.

[검증]
- npx tsc --noEmit
- npx vitest run (기대: 322 pass 유지, 기존 smoke-rt-rounds/ui-regression 테스트 안 깨짐 주의)
- 수동: Main chat 입력 / AppShell tabs / StatusBar / Terminal / ProjectOnboardingModal / RuntimeStatusBar / Notification

[커밋/PR]
feat(i18n): PR A2-G — Chat/Branch/Common (~130 keys)
Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

[주의 — Track 1/2 와 충돌]
- src/locales/index.ts / src/types/i18next.d.ts: Track 2 도 편집. rebase 시 Track 2 신규 namespace (trace/quality/skills/harness) 반영
- 본 Track 은 common/chat/dialog namespace 확장 중심. settings / context-panel / lib 건드리지 않음
```

## 참고 — 이 Track 이 아닌 것

- Settings subpanels (Track 1)
- Context panel tabs (Track 2)
- Plan/Workflow cards (이미 완결)
- Insight/Identity (이미 완결)
- lib/stores services (A3-ext, 후속)
