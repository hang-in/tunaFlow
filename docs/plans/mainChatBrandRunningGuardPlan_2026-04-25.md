---
title: Main chat 입력 guard — brand 에서 Developer 작업 중일 때 main chat 메시지 차단 (옵션 B)
status: ready-to-implement
priority: P1 (사용자 보고 — brand=main session 공유의 의도외 사이드이펙트)
created_at: 2026-04-25
related:
  - src/components/tunaflow/NewMessageInput.tsx        # 본 변경 위치
  - docs/plans/branchInheritsMainSessionPlan_2026-04-25.md  # PR #198 — session 공유 결정
  - docs/plans/multiDeveloperActivePlanIsolationPlan_2026-04-25.md  # PR #204 — ContextPack 격리만
  - docs/reference/branchSessionPolicy.md              # session 공유 invariants
canonical: true
owners:
  - architect (본 plan 작성)
  - developer (구현)
---

# 증상 (사용자 보고, 2026-04-25)

> "이거 세션을 공유하니깐 개발 소넷이 개발 중간에 본문 채팅에 던진 질문에 대답하네 ㅋㅋ"

# 진단

PR #198 (`branchInheritsMainSessionPlan`) 머지로 brand 와 main 이 같은 SDK WS session (process 1개) 공유. 그 부작용:

- brand drawer 의 Developer subagent 가 작업 중인 동안 사용자가 *main chat* 에 메시지 입력 → 같은 process 가 수신 → Developer 가 의도외 응답
- PR #204 (`multiDeveloperActivePlanIsolationPlan`) 가 ContextPack 차원 격리만 했고 메시지 라우팅은 단일 process 라 같은 axis

**의도된 동작** = brand 작업 결과를 main 에서 자연스럽게 이어받기 (PR #198 의 정합).
**의도외 동작** = main 입력이 *진행 중인 brand process* 에 끼어들기.

# 옵션 비교

| 옵션 | 설명 | Pros | Cons |
|---|---|---|---|
| A | send 시점에 conversation_id 분리해서 새 turn 으로 라우팅 | session 정합 유지 | SDK turn 분리 정확히 안 되면 오작동, 검증 어려움 |
| **B (채택)** | brand 가 running 중이면 main input disable + "drawer 에서 진행 중" 안내 | 단순, 사용자 의도 가까움, race 차단 확실 | brand 작업 끝날 때까지 main 입력 못함 (수용 가능 — 어차피 같은 session) |
| C | brand 별 process spawn (격리) | 진짜 격리 | PR #198 정합 깨짐, session 공유 이점 상실 |

# 채택 — 옵션 B

## 핵심 로직

`NewMessageInput.tsx` 가 main mode (`threadMode === false`) 일 때:

- 현재 `isCurrentThreadRunning` (line 186) 은 `effectiveThreadId` (= main 인 경우 `selectedConversationId`) 가 `runningThreadIds` 에 있는지 검사
- **추가**: main mode 에서 *brand 도 running 중인지* 별도 체크 — `runningThreadIds` 안에 `branch:` prefix 키가 있으면 brand running

```tsx
// 새 derived state
const brandRunning = !threadMode && runningThreadIds.some((id) => id.startsWith("branch:"));
const inputDisabled = !selectedConversationId || isCurrentThreadRunning || brandRunning;
```

- `brandRunning` 일 때 textarea `disabled`
- 안내 banner: "브랜치에서 작업 진행 중 — 완료 후 입력 가능"
- Send 버튼 / 키보드 단축키 (Cmd+Enter) 도 같이 차단

## 안내 UX

ContextBadges 위 또는 mode bar 자리에 inline banner:

```tsx
{brandRunning && (
  <div className="mb-1.5 flex items-center gap-2 text-[10px] rounded px-2.5 py-1 text-amber-600/70 bg-amber-500/8">
    <Loader2 className="w-3 h-3 animate-spin" />
    <span className="flex-1">브랜치에서 진행 중인 작업이 있어 메인 채팅 입력이 비활성화됐습니다</span>
    <button onClick={() => useChatStore.getState().openThread(/* running brand id */)}
      className="text-amber-600 hover:underline">드로어 열기</button>
  </div>
)}
```

`openThread` 클릭 시 사용자가 진행 중 brand drawer 로 이동.

# Invariants

- **[INV-1]** main mode + 어떤 brand 가 running (= `runningThreadIds` 에 `branch:*` 키 있음) → textarea disabled + send 차단
- **[INV-2]** banner 표시 + "드로어 열기" 액션 — 사용자가 즉시 진행 상황 확인 가능
- **[INV-3]** brand 작업 끝나면 (`runningThreadIds` 에서 brand 키 제거) 자동으로 input 활성화 (re-render)
- **[INV-4]** thread mode (`threadMode === true`) 즉 brand drawer 안에서는 본 guard 무관 — drawer 내부 입력은 그대로 동작
- **[INV-5]** main 의 다른 conversation 으로 전환했을 때도 *전역 brand running* 이면 guard 작동 (원치 않으면 후속 plan 으로 conversation_id scoping 추가)

# 구현 (단일 경로)

## 파일

`src/components/tunaflow/NewMessageInput.tsx` 만 수정.

## 변경

1. **derived state 추가** (line 186 근처):

```tsx
const brandRunning = useMemo(
  () => !threadMode && runningThreadIds.some((id) => id.startsWith("branch:")),
  [threadMode, runningThreadIds],
);
const runningBrandConvId = brandRunning
  ? runningThreadIds.find((id) => id.startsWith("branch:"))
  : null;
```

2. **textarea disabled** (line 520) 에 추가:

```tsx
disabled={!selectedConversationId || brandRunning}
```

3. **send 버튼 disabled** (line 593) 에 추가:

```tsx
disabled={!text.trim() || !selectedConversationId || ptyRespawning || brandRunning}
```

4. **handleKeyDown** (Cmd+Enter 처리부 — 별도 함수 안에서) 에 brandRunning 체크 추가하여 send 차단

5. **banner** 추가 (line 392 ContextBadges 위):

```tsx
{brandRunning && (
  <div className="mb-1.5 flex items-center gap-2 text-[10px] rounded px-2.5 py-1 text-amber-600/70 bg-amber-500/8">
    <Loader2 className="w-3 h-3 animate-spin" />
    <span className="flex-1">{t("input.brand_running_guard")}</span>
    {runningBrandConvId && (
      <button
        onClick={() => {
          const branchId = runningBrandConvId.replace(/^branch:/, "");
          useChatStore.getState().openThread(branchId);
        }}
        className="text-amber-600/80 hover:underline"
      >
        {t("input.brand_running_open_drawer")}
      </button>
    )}
  </div>
)}
```

6. **i18n key 추가** — `src/locales/ko/chat.json`, `src/locales/en/chat.json`:
   - `input.brand_running_guard` — "브랜치에서 진행 중인 작업이 있어 메인 채팅 입력이 비활성화됐습니다" / "Branch task running — main chat input disabled"
   - `input.brand_running_open_drawer` — "드로어 열기" / "Open drawer"

# 검증

## 수동 Smoke

1. main 에서 conversation 선택 → message 입력 → 정상 send
2. brand drawer 열기 → brand 에서 message send → brand 응답 streaming 중
3. main chat 으로 이동 (drawer 그대로 둠) → textarea disabled + banner 표시 확인
4. banner "드로어 열기" 클릭 → drawer 가 running brand 로 전환됨
5. brand 응답 완료 후 → main 으로 이동 → textarea 활성화 (auto re-enable)
6. thread mode (brand drawer 안) 입력은 영향 없음

## 자동

- `NewMessageInput.test.tsx` 추가:
  - `runningThreadIds: ["branch:abc"]` + `threadMode: false` → input disabled + banner present
  - `threadMode: true` → input enabled (drawer 내부 unaffected)

# Developer 핸드오프 프롬프트

```
[작업] Main chat 입력 guard — brand running 중 main input disable + 안내 banner (P1, plan B)

[SSOT] docs/plans/mainChatBrandRunningGuardPlan_2026-04-25.md

[배경 3줄]
- PR #198 brand=main session 공유의 부작용. brand 에서 Developer 작업 중일 때 main chat 입력이 같은 process 에 끼어들기
- 옵션 B 채택: main mode 에서 brand running 시 input disable + "드로어 열기" 안내
- 단일 파일 수정 (NewMessageInput.tsx) + i18n key 2개

[수정 범위]

1) src/components/tunaflow/NewMessageInput.tsx
   - line 186 근처: brandRunning + runningBrandConvId derived state (useMemo)
   - line 392 (ContextBadges 위): banner JSX (plan §구현 §5)
   - line 520 (textarea): disabled 조건에 brandRunning 추가
   - line 593 (send button): disabled 조건에 brandRunning 추가
   - handleKeyDown 의 Cmd+Enter 처리부에 brandRunning 가드 추가

2) src/locales/ko/chat.json + src/locales/en/chat.json
   - input.brand_running_guard
   - input.brand_running_open_drawer

3) src/tests/components/NewMessageInput.test.tsx (또는 비슷)
   - brandRunning 상태에서 disabled + banner 검증
   - threadMode=true 일 땐 unaffected 검증

4) docs/plans/index.md 등록

[검증]
- npx tsc --noEmit
- npx vitest run src/tests/components/NewMessageInput.test.tsx (해당 파일)
- 수동: plan §검증 §수동 Smoke 1~6 모두

[커밋]
- feat(input): main chat guard while brand task running (option B)
- docs(plans): register mainChatBrandRunningGuardPlan
- test(input): brand-running guard coverage

trailer: Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

[PR 제목]
feat(input): guard main chat while brand task running (PR #198 follow-up)
```

# 후속 / Sibling

- `metaFloatingChatPosClampPlan_2026-04-25` — 같은 시점 보고된 별 axis (footer drift driver)
- (후속 후보) brand running 의 *conversation scoping* — 다른 main conversation 으로 전환 시 guard 미적용 옵션. 일단 본 plan 은 전역 (모든 brand running 에 main input 차단). 사용자 사용 패턴 보고 결정.
- (후속 후보) 옵션 A 도입 — send 시점 conversation_id 분리. 본 plan 옵션 B 가 먼저 검증되고 사용자가 진짜 분리 원하면 추가.
