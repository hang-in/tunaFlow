# context-hub Explicit Handoff Plan

상태: 제안
작성: 2026-03-30

## 목표

`context-hub`에서 검색/조회한 문서를 자동 주입하지 않고, 사용자가 명시적으로 현재 작업 흐름에 넘길 수 있게 만든다.

## 왜 explicit handoff인가

- 자동 fetch/자동 주입은 아직 과하다
- 지금 정책은 `bundled/local/private only`와 함께 사용자 통제를 유지하는 것이 중요하다
- 먼저 “선택한 문서를 현재 대화/아티팩트에 넘긴다”는 명시적 흐름을 만들어야 한다

## 범위

### 1. 문서 액션

- `Copy`
- `Send to Current Context`
- 가능하면 `Save as Artifact`

### 2. 현재 대화 연결

- 사용자가 선택한 문서 내용을 현재 대화 입력 보조 컨텍스트로 넘기거나
- follow-up/forward와 유사한 명시적 source로 붙인다

### 3. 가시성

- 어떤 context-hub 문서가 현재 요청에 명시적으로 넘겨졌는지 확인 가능해야 한다
- 최소한 message meta 또는 trace에 흔적을 남긴다

## 비목표

- auto ContextPack injection
- background fetch
- flow agent 자동 선택
- public source fetch

## 권장 UX

### Settings > Runtime > context-hub

- 문서 미리보기 하단 액션:
  - `Copy`
  - `Send to Context`
  - `Save as Artifact` (가능하면)

### Chat surface

- handoff된 문서는 현재 입력창 또는 message action 흐름과 자연스럽게 연결
- “이 문서가 포함될 예정” 정도의 작은 표시가 있으면 좋다

## 성공 기준

- 사용자가 context-hub에서 문서를 검색해 현재 대화 맥락으로 넘길 수 있다
- 자동 주입 없이 명시적 handoff만 동작한다
- 어떤 문서가 전달됐는지 최소한의 가시성이 있다

## 메모

이 단계는 knowledge search를 agent workflow와 연결하는 첫 단계다. 중요한 건 자동화보다 명시성이다.
