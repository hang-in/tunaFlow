# Background Agent Execution Plan

작성자: OpenAI Codex
작성일: 2026-03-28
상태: Phase 1 구현 완료 (2026-03-28)

## 목적

`tunaFlow`의 장기 에이전트 실행 경로를 Tauri command의 긴 request-response lifecycle에서 분리해,
**background worker + event + DB 기반** 구조로 전환한다.

현재 목표는 `tunapi/ws` 구조를 그대로 이식하는 것이 아니라,
`tunaFlow` 안에서 다음을 동시에 만족하는 것이다.

- UI가 답변 중에도 멈추지 않음
- 결과가 유실되지 않음
- DB가 최종 SSOT를 유지함
- 프론트는 event를 놓쳐도 재조회로 복구 가능함

## 문제 배경

현재 `tunaFlow`는:

- `send_with_claude`
- `stream_with_claude`
- `send_with_codex`
- `send_with_gemini`
- `send_with_opencode`
- `roundtable_run`
- `roundtable_followup`

같은 command가 장기 subprocess 실행을 command 내부에서 끝까지 들고 있다.

프론트 리렌더 최적화 이후에도,
에이전트 실행 중 채팅창 스크롤과 패널 상호작용이 막히는 현상이 남아 있다.

즉 병목은 단순 React 렌더링이 아니라,
**장기 실행이 여전히 command lifecycle에 붙어 있는 구조** 자체에 있다.

## tunapi/tunaChat/tunaDish와의 차이

기존 `tunapi` 기반 구조는:

- 별도 프로세스가 장기 실행 담당
- 앱은 websocket/event만 수신
- UI 병목은 적음

하지만 단점도 있었다.

- 앱 재시작 / ws disconnect 시 응답 유실 가능성

`tunaFlow`는 이 문제를 그대로 가져오지 말아야 한다.

즉 목표는:

- 장기 실행은 background로 분리
- 결과는 DB에 즉시 기록
- 프론트는 event + DB 재조회로 복구

이다.

## 현재 구조 요약

### 현재 일반 send 구조

1. 프론트 `invoke("send_with_*")` 또는 `invoke("stream_with_claude")`
2. backend command가 시작부터 완료까지 유지됨
3. command 내부에서 subprocess spawn + wait/read + DB write
4. command 완료 후 프론트가 `list_messages()` 재조회

### 현재 문제

- command가 오래 살아 있음
- 프론트가 await 중인 동안 UI 응답성이 떨어질 수 있음
- streaming/event를 쓰더라도 command lifecycle 자체는 길게 유지됨

## 목표 구조

### 핵심 원칙

1. **start command는 빨리 반환**
2. **실제 긴 실행은 background worker가 수행**
3. **진행 상태는 event로 전달**
4. **최종 상태는 DB에 저장**
5. **프론트는 event를 놓쳐도 DB에서 복구 가능**

### 목표 흐름

```text
Frontend send
  -> invoke("start_agent_run")
  -> returns immediately

Backend start_agent_run
  -> persist user message
  -> create placeholder assistant row
  -> spawn background worker
  -> return ack(job_id/message_id)

Background worker
  -> run subprocess
  -> emit progress/chunk/completed/error events
  -> update DB incrementally/finally

Frontend
  -> listen to events
  -> optimistic UI update
  -> on reconnect/reload, list_messages()/trace re-query
```

## 1차 범위

우선 일반 agent send 경로부터 바꾼다.

### 대상

- Claude
- Codex
- Gemini
- OpenCode

### 제외

- RT full migration
- reviewer lane
- worktree/git
- 외부 ws 서버 분리

## 데이터 원칙

### DB가 SSOT

반드시 DB에 남아야 할 것:

- user message
- placeholder assistant message
- streaming/partial state (가능한 경우)
- final assistant message
- error 상태
- trace_log

즉 event는 전달 수단이지,
최종 기록 저장소가 아니다.

### event는 보조 채널

event는 아래 역할만 한다.

- 진행 상태 표시
- 부분 chunk 표시
- 완료/에러 알림

앱이 event를 놓쳐도,
프론트는 `list_messages`, `list_traces` 등으로 복구 가능해야 한다.

## 권장 백엔드 구조

### Phase 1: start_* command 분리

예시:

- `start_claude_stream`
- `start_codex_run`
- `start_gemini_run`
- `start_opencode_run`

역할:

- 입력 검증
- user message 저장
- placeholder assistant row 저장
- background task spawn
- 즉시 ack 반환

### Phase 2: background worker 실행

worker 역할:

- subprocess 실행
- progress/chunk/completed/error emit
- DB 상태 업데이트
- trace 기록

### Phase 3: job registry

1차에는 단순 conversation_id/message_id 기준으로도 가능하지만,
장기적으로는 아래가 필요하다.

- job_id
- conversation_id
- engine
- started_at
- status

이건 cancel/reconnect/diagnostics에 유리하다.

## 권장 프론트 구조

### 현재 문제

현재 runtimeSlice는 대체로:

- invoke 시작
- 완료까지 await
- 끝나면 `list_messages` 재조회

패턴이다.

### 목표

프론트는:

1. `start_*` command 호출
2. ack 수신 후 즉시 return
3. event로 progress/chunk/completed/error 반영
4. 필요 시 final refresh만 수행

즉 `await final result`가 아니라
**start + subscribe** 구조로 바뀌어야 한다.

## 엔진별 고려사항

### Claude

- 이미 streaming event 구조가 있음
- 가장 먼저 background start 구조로 옮기기 쉬움

### Gemini

- 현재 one-shot
- 1차는 full streaming보다 progress/completed 이벤트만 붙여도 가치 큼

### Codex

- stdin 입력 + JSONL stdout
- 1차는 one-shot background로 충분
- 나중에 event parsing 확장 가능

### OpenCode

- 1차는 one-shot background로 충분

## RT는 왜 후속인가

RT는 아래가 더 복잡하다.

- participant loop
- sequential/deliberative
- progress emit 다수
- brief 저장
- followup path

즉 일반 agent send를 먼저 background 구조로 정리한 뒤,
그 패턴을 RT로 가져가는 것이 안전하다.

## 장점

- UI 응답성 회복 가능성 큼
- 스크롤/패널/입력 막힘 완화
- event + DB 구조로 유실 완화
- `tunapi`의 장점과 `tunaFlow`의 로컬 DB 장점을 동시에 취할 수 있음

## 단점 / 리스크

- 구조 변경 범위 큼
- cancel semantics 다시 맞춰야 함
- placeholder/finalize race condition 점검 필요
- event와 DB update 타이밍 차이를 조정해야 함

## 단계별 우선순위

1. 일반 agent 4종 start/background 구조
2. runtimeSlice를 start + event 구조로 전환
3. cancel semantics 재점검
4. RT run/followup 이관
5. 필요 시 job registry 확장

## 완료 기준

아래가 되면 1차 완료로 본다.

1. 일반 agent send command가 빠르게 반환한다
2. 실제 subprocess는 background에서 돈다
3. progress/completed/error가 event로 온다
4. 결과는 DB에 기록된다
5. 프론트는 event를 놓쳐도 DB 재조회로 복구 가능하다
6. 실행 중에도 채팅창 스크롤과 기본 UI 상호작용이 막히지 않는다
