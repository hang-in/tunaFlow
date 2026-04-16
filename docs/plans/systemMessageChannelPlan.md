---
title: System Message Channel — 자동 생성 메시지 분리
status: in-progress
created_at: 2026-04-16
priority: P1
related: sdkUrlSessionModePlan.md, contextPackTieringIdea.md
---

# System Message Channel — 자동 생성 메시지 분리

> tool-request 결과, 워크플로우 지시, 에스컬레이션 등 **tunaFlow가 자동 생성하는 메시지**를
> user role에서 분리하여 system role로 저장·전달·렌더링한다.

---

## 1. 문제

현재 tunaFlow가 자동 생성하는 메시지(tool-request 결과, rework 지시, review 요청 등)가
`role='user'`로 저장되어:

1. **사용자 대화창에 노출** — 사용자가 보낸 것처럼 보임
2. **ContextPack 히스토리에 포함** — 에이전트가 "사용자가 이걸 말했다"로 해석
3. **토큰 낭비** — 도구 결과가 매 턴 히스토리에 누적

---

## 2. 설계

### 2.1 DB: `role='system'` 활용

messages 테이블의 role 컬럼은 이미 TEXT. 새 값 추가만 하면 됨:

| role | 의미 | 예시 |
|------|------|------|
| `user` | 사용자가 직접 입력 | 일반 채팅 |
| `assistant` | 에이전트 응답 | Claude/Codex/Gemini 응답 |
| `system` | **tunaFlow 자동 생성** | tool-request 결과, rework 지시, workflow 트리거 |

### 2.2 적용 대상

| 현재 경로 | 변경 |
|-----------|------|
| `runtimeSlice.sendWithEngine(engine, followUp)` (tool-request 결과) | `sendSystemMessage(followUp)` |
| workflow orchestration (rework/review/escalation 자동 메시지) | `role='system'` 저장 |

### 2.3 에이전트 전달

- **-p/sdk-url 모두**: system 메시지도 프롬프트에 포함해야 에이전트가 받음
- ContextPack 히스토리 조립 시: `role='system'` 메시지는 **최근 1개만** 포함 (전체 누적 방지)

### 2.4 UI 렌더링

- `role='system'` → 접힘 카드 (현재 ToolResultCollapsible과 동일 패턴)
- 사용자 bubble 스타일 아님 — 시스템 알림 스타일 (border + muted 색상)

---

## 3. 구현 범위

### Phase 1: tool-request 결과 분리 (이번 PR)

1. **Rust**: `persist_system_message()` 함수 추가 — `role='system'` 저장
2. **Rust**: `send_system_followup` Tauri 커맨드 — system 메시지 저장 + 에이전트 실행
3. **TS**: `runtimeSlice` — tool-request follow-up 경로를 `sendWithEngine` → `sendSystemFollowup`으로 변경
4. **TS**: `MessageItem` — `role='system'` 전용 렌더링 (접힘 카드, 시스템 색상)
5. **Rust**: ContextPack 히스토리 조립 — system 메시지는 최근 1개만 포함

### Phase 2: 워크플로우 자동 메시지 (후속)

- rework/review/escalation 자동 메시지를 system role로 전환
- 현재는 user role로 저장되는 모든 자동 메시지 식별 + 전환

---

## 4. 변경 파일

| 파일 | 변경 |
|------|------|
| `persistence.rs` | `persist_system_message()` 추가 |
| `agents.rs` | `send_system_followup` 커맨드 추가 |
| `context_loading.rs` | system 메시지 히스토리 포함 정책 (최근 1개) |
| `runtimeSlice.ts` | tool-request follow-up → system 메시지 경로 |
| `MessageItem.tsx` | `role='system'` 렌더링 분기 |
| `types/index.ts` | Message role 타입에 'system' 추가 (이미 string이면 불필요) |
