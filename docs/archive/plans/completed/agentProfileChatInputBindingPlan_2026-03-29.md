# tunaFlow Agent Profile ↔ Chat Input 연결 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 초안

## 목적

`Settings > Agents`에서 만든 `Agent Profile`이
실제 채팅 입력과 실행 경로에서 사용되도록 연결한다.

지금 단계에서 가장 중요한 것은
관리 UI로 만든 profile이 실제 사용자 워크플로에 반영되는 것이다.

## 현재 상태

이미 구현된 것:

- `Settings > Agents`
- profile 목록 / 선택 / 편집
- engine / model / persona / default skills 저장
- settings 기반 persistence

아직 없는 것:

- chat input에서 profile 선택
- 선택 profile의 값이 실제 실행 입력에 반영
- conversation별 현재 active profile 표시

## 핵심 판단

지금은 persona 체계를 더 깊게 만드는 것보다,
이미 만든 `Agent Profile`을 실제 채팅 입력에 연결하는 것이 우선이다.

이유:

1. 사용자는 이제 Settings에서 profile을 만들 수 있다
2. 하지만 현재는 실제 채팅에서 그것을 사용할 수 없다
3. 따라서 제품 가치가 아직 절반만 구현된 상태다

## 목표

### 1. Chat input에서 Agent Profile 선택

- 입력창 근처에 현재 active profile 표시
- 드롭다운 또는 compact selector로 profile 선택 가능

### 2. 선택 profile의 실행 반영

최소 반영 필드:

- engine
- model
- persona
- default skills

### 3. conversation 단위 active profile 유지

- 현재 대화/브랜치에서 마지막으로 선택한 profile 유지
- 앱 재시작 후에도 복원 가능하면 더 좋음

## 권장 UX

### 입력창 표시

현재 엔진/모델 선택 UI보다 상위에
`Agent Profile` 선택기가 먼저 있어야 한다.

예:

- `Architect Claude`
- `Reviewer Codex`
- `Tester Gemini`

선택 시:

- engine selector
- model selector
- persona
- default skills

가 profile 기준으로 채워진다.

### 고급 조정

MVP에서는:

- profile 선택 후
- 기존 engine/model UI는 override 용도로 남겨도 된다

장기적으로는:

- profile 우선
- 세부 override는 advanced 옵션

## 구현 범위

- `NewMessageInput.tsx`
- 입력 관련 selector 컴포넌트
- 관련 store / settings persistence
- 필요 시 conversation metadata 반영

## 비목표

- persona 편집기 구현
- auto skill selection
- project별 profile policy
- role-based automatic profile switching

## 완료 기준

1. chat input에서 agent profile을 고를 수 있다
2. profile 선택 시 engine/model/default skills가 실제 반영된다
3. 사용자는 skill이 아니라 agent를 선택하는 흐름을 경험한다

