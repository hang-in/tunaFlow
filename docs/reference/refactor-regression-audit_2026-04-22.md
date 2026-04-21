---
title: Refactor Regression Audit — 2026-04-22
status: active
canonical: false   # 주관적 판단 포함
created_at: 2026-04-22
owner: architect
related:
  - docs/reference/harnessMaturityAudit_2026-04-16.md
  - docs/plans/refactorRoadmap_2026-04-20.md
---

# Refactor Regression Audit

Phase 5 수동 테스트 중 발견된 증상들 (beach ball · 메시지 사라짐 · 두 번째 turn 진입 실패) 의 **근본 원인 수사** + **증상별 hotfix 누적 접근 중단** 을 위한 감사.

## 0. 동기

PR #109 에 8개 fix 가 누적됐지만 여전히 재현. 각 fix 는 실제 버그를 잡았지만 근본 구조 문제가 해결 안 됨 → audit 필요.

## 1. 감사 방법

- 문제 영역 파일 4개 선정: `runtimeSlice.ts`, `conversationSlice.ts`, `ChatPanel.tsx`, `RuntimeStatusBar.tsx`
- Phase 1 리팩토링 직전 commit (`783261d^`) 을 **pre-refactor baseline** 으로
- `diff baseline current` 수행

## 2. 핵심 발견

### 발견 1: recovery 로직 자체는 pre-refactor 와 동일 (매우 중요)

```
ChatPanel.tsx        : baseline 349 lines = current 349 lines   → unchanged
RuntimeStatusBar.tsx : baseline 411 lines = current 411 lines   → unchanged
runtimeSlice.ts      : baseline 505 → current 451               → refactored
conversationSlice.ts : baseline 305 → current 358               → refactored
```

`ChatPanel` 의 auto-recover useEffect 와 `RuntimeStatusBar` 의 orphan-recovery polling 은 **리팩토링에서 건드리지 않았음**. 즉 **이 로직들은 원래부터 있었던 것**.

### 발견 2: 실제 변경 — selectConversation 의 set 패턴

Pre-refactor: 한 번의 `set()` 로 `messages/branches/memos/artifacts` 전부 reset + `Promise.all` 로 병렬 invoke 후 단일 `set()`:

```ts
set({ selectedConversationId: id, messages: [], branches: [], memos: [], artifacts: [] });
const [msgs, brs, memos, arts] = await Promise.all([
  invoke("list_messages", ...),
  invoke("list_branches", ...),
  invoke("list_memos_by_conversation", ...),
  invoke("list_artifacts", ...),
]);
set({ messages, branches, memos, artifacts });
```

Post-refactor: 각 slice owner 의 action 을 **순차 호출** — 중간에 state transition 여러 번:

```ts
set({ selectedConversationId: id, messages: [] });
get().closeThread();               // ← 별도 set
get().resetBranchState();          // ← 별도 set
get().clearConversationAssets();   // ← 별도 set
const messages = await invoke("list_messages", ...);
await Promise.all([
  get().loadBranches(id),
  get().loadMemos(),
  get().loadArtifacts(),
]);
```

영향: 순차 set 들 사이의 **state transition 창** 이 늘어나 polling 로직이 중간 상태를 관측할 수 있음. 다만 주증상(메시지 사라짐)의 직접 원인은 아니다.

### 발견 3: 주증상의 structural root

```
[증상]  테스트1 정상 → 테스트2 스트리밍 시작 전 hang / 사라짐
   ↓
[관찰] 백엔드 로그에 [sdk-session] 없음 → prepare_engine_run 이 write lock 에서 대기
   ↓
[원인] 이전 turn 의 on_run_completed 의 vector indexing 이 write lock 을 오래 hold
   ↓
[도미노] agent_jobs INSERT 지연 → orphan-recovery 의 10s GRACE 넘어감 → false positive
   ↓
[결과] FP 처리 시 list_messages 재조회 + setState({messages: fresh}) 로 in-flight 덮어씀
```

리팩토링 자체가 원인이 아니라, **Phase 3+ 에서 추가된 heavy post-completion hook (vector indexing, memory compression, session-link discovery)** 이 **기존 recovery 로직의 GRACE 가정** 을 깨뜨리는 구조.

### 발견 4: 덮어쓰기 경로가 2개 중복 (wrapSet 우회)

```
src/components/tunaflow/RuntimeStatusBar.tsx:163
  → invoke("list_messages") → useChatStore.setState({ messages: msgs })

src/components/tunaflow/ChatPanel.tsx:103
  → invoke("list_messages") → useChatStore.setState({ messages: fresh })
```

두 경로 모두 `useChatStore.setState()` 직접 호출. Zustand create((set, get)...) 의 set 만 감싼 diagnostic wrapper 는 이 호출을 못 잡음. 두 recovery 가 서로 경쟁하며 race.

## 3. 증상 → 경로 매핑

| 증상 | 경로 | 근본 |
|------|------|------|
| Beach ball (save_progress) | main thread sync Tauri command + write.lock() | sync command 설계 |
| Beach ball (update_plan_phase) | 동일 패턴 | 동일 |
| 메시지 사라짐 (1차) | orphan-recovery 의 setState({messages}) 덮어쓰기 | FP + 덮어쓰기 |
| 메시지 사라짐 (2차) | ChatPanel auto-recover 의 setState({messages}) 덮어쓰기 | 동일 FP + 중복 경로 |
| 테스트2 진입 실패 | prepare_engine_run 이 vector index write lock 대기 | heavy hook |

주증상 **3개는 사실 1개 root cause** — post-completion hook write lock 장기 hold → orphan FP → 덮어쓰기 race.

## 4. 통합 해결 방향 (symptom hotfix 누적 중단)

### 핵심 원칙 — "Single source of truth for message recovery"

### 4-1. Recovery 경로 1개로 통일

- `RuntimeStatusBar` 의 orphan-recovery **만** 유지
- `ChatPanel` 의 auto-recover useEffect **삭제** (중복)

### 4-2. Recovery 는 `messages` 덮어쓰지 않음

- `markConversationStale(id)` + `_endRun(silent: true)` 만
- 사용자가 re-select 또는 다음 turn 시작 시 자연스럽게 재조회
- **직접 `useChatStore.setState({messages})` 금지**

### 4-3. Grace 현실화

- 10s → 45s + dev 환경에서는 60s
- 더 좋은 방법: `agent_jobs` 의 `status IN ('running', 'queued')` + 최근 `updated_at` 기준 "실제 write 경합 감지" 로 판단

### 4-4. 근본 fix — write lock hold 단축

- `index_chunks_blocking` 의 per-chunk write lock 분할 (PR #109 fix 7 에 이미 존재, 보존)
- 가능하면 `prepare_engine_run` 의 `agent_jobs INSERT` 를 write lock 획득 직후 **가장 먼저** 수행 → orphan FP 창 축소

### 4-5. Sync Tauri command 의 `state.write.lock()` 패턴 audit

- `pub fn` + `state.write.lock()` 조합은 main thread freeze 위험
- 22개 파일 중 UI 클릭 경로부터 `pub async fn` + `spawn_blocking` 로 전환
- PR #109 fix 1-2 (save_progress, update_plan_phase) 는 이 패턴의 대표 사례 — 보존 + 확대 적용

## 5. PR #109 처리 방침

6개 code fix 중:

| Fix | 판단 |
|-----|------|
| save_progress_content async | 유지 — 4-5 항목 구현 |
| update_plan_phase async | 유지 — 4-5 항목 구현 |
| focus-visible textarea 제외 | 유지 — UI 이슈, 별개 |
| selectConversation same-id skip | **폐기** — 근본 원인 아님, 복잡도만 증가 |
| orphan-recovery markStale 전환 | 유지 — 4-2 항목 구현 |
| ChatPanel auto-recover length guard | **폐기 후 재작성** — 4-1 항목에 따라 useEffect 제거 |
| vector index per-chunk lock | 유지 — 4-4 항목 구현 |
| debug middleware | revert |

→ PR #109 는 closed (hotfix 누적 부적절). 위 원칙에 따른 **신규 PR 로 재구성**.

## 6. 다음 행동

1. PR #109 close (또는 draft 유지) + reference 문서 링크
2. 신규 audit-based PR 브랜치 (`feat/recovery-unification`) 에서 4-1 ~ 4-5 통합 구현
3. 변경 후 수동 E2E 재검증
4. pass → 머지 → Phase 5 나머지 시나리오 진행

## 7. 교훈

- 증상별 hotfix 누적은 근본 구조 모순을 숨김. 3-4회 누적되면 **중단하고 audit**
- 리팩토링 자체를 탓하기 전에 **누적 기능(hook, polling)의 interaction** 확인
- recovery / polling 경로가 2개 이상이면 경쟁 위험 — **SSOT 설계 원칙** 으로 통합
- Zustand 에서 `setState` 직접 호출은 middleware 를 우회 — diagnostic/추적에 한계
