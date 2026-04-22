# tunaFlow 장기 에이전트 실행을 background/event 구조로 전환 Phase 1

적용 스킬:
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build\react-best-practices`
  - 이유: 현재 UI 멈춤 문제는 프론트 렌더 최적화만으로 해결되지 않고, 장기 실행 lifecycle 자체를 분리해야 함
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build\composition-patterns`
  - 이유: Tauri command 요청-응답과 장기 subprocess 실행을 분리하고, progress/completion/error를 event + DB 기반으로 재조립해야 함
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build\frontend-design`
  - 이유: 목표는 단순 구조 변경이 아니라, 답변 중에도 스크롤/클릭/전환이 가능한 체감 UX를 만드는 것임

프로젝트:
- `D:\privateProject\tunaFlow`

참고 문서:
- `D:\privateProject\tunaFlow\docs\plans\backgroundAgentExecutionPlan.md`

현재 문제:
- 프론트 리렌더 최적화 후에도, 모든 에이전트 실행 중 채팅창 스크롤조차 막히는 현상이 남아 있음
- 이는 단순 React 병목을 넘어서,
  **장기 에이전트 실행이 여전히 Tauri command 요청-응답 경로에 붙어 있는 구조 문제**로 보는 것이 타당함

비교 기준:
- `tunaChat` / `tunaDish`는 별도 `pi/tunapi` 프로세스가 장기 실행을 처리하고,
  앱은 ws/event만 받기 때문에 UI 병목이 덜했음
- 다만 ws 끊김 시 응답 유실 리스크가 있었음

이번 작업 목표는:
**장기 agent 실행을 command의 synchronous request-response 경로에서 분리하고, background worker + event + DB 기반으로 전환해서 UI 응답성을 회복하는 것**이다.

중요:
- 실제 코드 기준으로만 작업
- 이번 단계는 Phase 1: background 실행 골격 전환
- `tunapi`처럼 ws 서버까지 완전히 분리하는 것은 아님
- 응답 유실을 피하기 위해 DB를 SSOT로 유지할 것
- 모든 응답과 보고는 한국어로만 작성하라

---

## 목표

최소한 아래를 만족하라.

1. agent 실행 시작 command는 빠르게 반환
2. 실제 긴 subprocess 실행은 background task/thread에서 진행
3. progress / chunk / completed / error는 event로 전달
4. 최종 결과는 DB에 기록되어, 프론트가 event를 놓쳐도 복구 가능
5. 실행 중에도 앱 스크롤/클릭/패널 전환이 막히지 않게 만드는 방향이어야 함

---

## 먼저 확인할 파일

### Backend
- `D:\privateProject\tunaFlow\src-tauri\src\commands\agents.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\roundtable.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\roundtable_helpers\executor.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\db\mod.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\messages.rs`

### Frontend
- `D:\privateProject\tunaFlow\src\stores\slices\runtimeSlice.ts`
- `D:\privateProject\tunaFlow\src\components\tunaflow\ChatPanel.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\BranchThreadPanel.tsx`

필요 시 비교:
- `D:\privateProject\tunaChat`
- `D:\privateProject\tunaDish`

---

## 구현 요구사항

### 1. 핵심 구조 전환
현재:
- `invoke("send_with_*")` / `invoke("stream_with_claude")`
- command가 긴 실행을 끝까지 들고 있음

목표:
- `start_*` command는 **시작만 하고 바로 반환**
- 실제 agent run은 background thread/task에서 진행
- 진행 중 상태는 event emit
- 완료/오류도 event emit
- 결과는 DB에 즉시 기록

예:
- `start_claude_stream`
- `start_gemini_run`
- `start_codex_run`
- `start_opencode_run`

중요:
- 이름은 실제 코드 스타일에 맞게 정하라
- 핵심은 “빠른 반환 + 백그라운드 실행”

### 2. DB를 SSOT로 유지
반드시:
- user message
- placeholder / streaming message
- 최종 assistant message
- error 상태
를 DB에 기록하라

중요:
- 프론트는 event를 놓쳐도 `list_messages()` 재조회로 복구 가능해야 함
- `tunapi/ws` 구조의 “응답 유실” 문제를 그대로 가져오면 안 됨

### 3. 이벤트 모델 정리
최소한 아래 이벤트 계층을 유지/정리하라.

- progress
- chunk (가능한 엔진만)
- completed
- error

중요:
- Claude는 기존 streaming 유지 가능
- Gemini/Codex/OpenCode는 1차에 chunk가 없어도 됨
- 대신 progress/completed/error는 event 기반으로 통일하는 것이 중요

### 4. Frontend runtimeSlice 정리
프론트는:
- command가 길게 await되는 구조에서 벗어나
- 시작 command 호출 후
- 상태는 event + DB refresh로 받게 정리하라

중요:
- 현재 `sendMessage`, `sendWithGemini`, `sendWithCodex`, `sendWithOpencode` 흐름이
  “invoke 기다림 → 끝나면 list_messages” 패턴이면
  이를 background/event 구조에 맞게 바꿔야 함

### 5. 1차 범위
이번 단계는 최소한 아래까지면 된다.

- Claude
- Gemini
- Codex
- OpenCode

RT는 가능하면 후속 또는 최소 연결만
왜냐하면 RT는 participant loop라 범위가 더 크다

즉 이번 단계 1차 목표는:
**일반 agent send 경로를 background/event 구조로 전환**

### 6. 범위 제한
이번 단계에서는 하지 말 것:
- tunapi/ws 서버 별도 프로세스 완전 이식
- 네트워크 기반 아키텍처 전환
- RT 전체 구조까지 한 번에 재작성
- docs 작업 같이 하기

---

## 구현 우선순위

권장:
1. backend start_* + background worker 골격
2. DB 기록 시점 정리
3. progress/completed/error event 정리
4. runtimeSlice를 await-result 구조에서 event/refresh 구조로 수정
5. 일반 agent 4종 연결
6. 가능하면 RT는 후속으로 남김

---

## 검증

작업 후 반드시 아래를 설명하라.

1. 왜 현재 구조가 UI 병목을 만든다고 판단했는지
2. 어떤 command를 start/background 구조로 바꿨는지
3. DB를 SSOT로 어떻게 유지했는지
4. 프론트가 event를 놓쳐도 어떻게 복구 가능한지
5. Claude/Gemini/Codex/OpenCode 각각 어떤 방식으로 연결했는지
6. 타입체크/빌드/가능한 검증 결과
7. 남은 리스크

---

## 출력 형식

### A. Changes Made
### B. Files Modified
### C. Background Agent Flow
### D. Verification
### E. Remaining Risks

바로 코드 수정까지 진행하라.
