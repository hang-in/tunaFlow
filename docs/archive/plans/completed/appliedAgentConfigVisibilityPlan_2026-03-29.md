# tunaFlow Applied Agent Config 가시화 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 초안

## 목적

사용자가 메시지를 보낸 뒤,
이번 응답이 어떤 `Agent Profile / Persona / Skills` 조합으로 실행됐는지
다시 확인할 수 있게 만든다.

지금은 입력창에서 선택 상태를 볼 수 있지만,
실행 후 assistant 결과와 연결된 applied config visibility는 약하다.

## 현재 상태

이미 구현된 것:

- `Agent Profile` 선택
- `engine / model / persona / default skills` 연결
- `personaFragment` runtime prompt 반영
- 입력창 selector에 현재 profile 요약 표시

아직 없는 것:

- assistant message와 연결된 applied profile / persona / skills 표시
- trace/runtime 영역에서 같은 실행 구성을 재확인할 수 있는 지점
- 전송 당시 설정과 이후 store 상태를 구분하는 최소 메타

## 핵심 판단

지금 단계에서 가장 자연스러운 1차 위치는
`assistant message meta`다.

이유:

1. 사용자는 응답을 읽는 순간 “어떤 agent setup으로 실행됐는지”를 가장 알고 싶다
2. trace는 진단용이므로 보조 위치가 더 맞다
3. 메인 chat surface에 applied config가 보여야 agent profile 기반 UX가 완성된다

## 목표

### 1. assistant message meta에 applied config 표시

최소 표시 후보:

- profile name
- persona name
- skills count

선택 표시:

- engine / model

### 2. trace/runtime은 보조 표시로 유지

trace/runtime에서는
세부 확인용 또는 badge 수준으로 연결한다.

핵심은 메인 정보 위치를 trace가 아니라 message surface에 두는 것이다.

### 3. applied config는 실행 시점 기준이어야 한다

중요:

- 현재 store 상태를 나중에 읽어 붙이면 안 된다
- 전송 시점의 `profile / persona / skills`를 기준으로 표시해야 한다

이번 단계에서 DB를 크게 늘리지 않더라도,
최소한 런타임/메시지 표시 기준은 “실행 시점 snapshot”이어야 한다.

## 권장 UX

### Message meta

assistant message header 하단 또는 상단의 compact meta row:

- `Architect Claude`
- `Architect`
- `3 skills`

또는:

- `profile: Architect Claude`
- `persona: Architect`
- `skills: 3`

너무 무겁지 않게 compact chip / muted text로 유지한다.

### Trace/runtime

- badge 또는 compact detail
- 메인 정보가 아니라 확인용

## 구현 범위

- assistant message 렌더링 컴포넌트
- 필요 시 runtime/trace meta 연결
- 전송 시점 applied config snapshot 전달/유지 경로

## 비목표

- full audit log
- DB 대규모 스키마 변경
- flow agent explainability 전체 구현
- applied docs visibility 동시 도입

## 완료 기준

1. 사용자는 assistant 응답에서 어떤 profile/persona/skills가 적용됐는지 재확인할 수 있다
2. 표시 정보는 전송 시점 기준으로 안정적이다
3. trace/runtime은 보조 확인 지점으로만 유지된다

