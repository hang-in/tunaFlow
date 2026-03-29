# tunaFlow Agent Profile 사용성 보강 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 초안

## 목적

`Agent Profile`이 이제 실제 채팅 입력과 RT 참가자 구성에 연결됐으므로,
다음 단계는 사용자가 현재 어떤 profile을 쓰고 있는지,
그 profile이 어떤 값들을 적용하는지,
`Custom` 전환 시 무엇이 바뀌는지를 더 명확히 이해하게 만드는 것이다.

이번 단계의 초점은 새 기능 추가보다
이미 연결된 `Agent Profile` 흐름을 더 읽기 쉽고 예측 가능하게 만드는 데 있다.

## 현재 상태

이미 구현된 것:

- `Settings > Agents`에서 profile 생성/편집/저장
- chat input에서 profile 선택
- `Custom` 모드 fallback
- profile 선택 시 engine/model/default skills 반영
- RT participant 초기값에 profile 사용

아직 부족한 것:

- 현재 선택된 profile의 persona/default skills 요약 가시성
- `Profile ↔ Custom` 전환 시 값 유지/초기화 규칙 설명 부족
- profile이 실제로 어떤 필드를 강제하는지 사용자가 직관적으로 알기 어려움
- 선택 profile 결과를 채팅 표면에서 재확인하기 어려움

## 핵심 판단

지금은 persona 편집기를 더 확장하는 것보다,
이미 도입된 `Agent Profile` 선택 경험을 안정화하는 것이 우선이다.

이유:

1. 사용자는 이제 profile을 실제로 선택할 수 있다
2. 하지만 선택 결과가 충분히 설명되지 않으면 신뢰가 떨어진다
3. `Profile`과 `Custom`의 경계가 불명확하면 UX가 금방 혼란스러워진다

## 목표

### 1. 현재 선택 profile 요약 표시

최소 표시 항목:

- profile name
- engine
- model
- persona key
- default skills count 또는 짧은 목록

이 정보는 입력창 근처 또는 selector 확장 영역에서
짧고 읽기 쉬운 형태로 보여야 한다.

### 2. `Custom` 전환 규칙 명확화

명확히 해야 할 것:

- `Custom`으로 바꾸면 engine/model을 직접 고를 수 있는지
- profile에서 가져온 default skills가 유지되는지 초기화되는지
- 다시 profile로 돌아오면 어떤 값이 복원되는지

이번 단계에서는 동작을 바꾸지 않아도 되지만,
UI에서 현재 규칙을 더 명확히 보여줘야 한다.

### 3. Profile 적용 상태의 재확인 지점 추가

예:

- 입력창 selector 하단 요약
- hover tooltip
- compact chips

핵심은 사용자가 보내기 직전에
지금 어떤 agent setup으로 보낼지 빠르게 확인할 수 있게 하는 것이다.

## 권장 UX

### 기본 구조

- selector는 현재처럼 compact하게 유지
- 선택 직후 요약 행 또는 compact detail row를 노출

예:

- `Architect Claude · claude · sonnet-4`
- `persona: architect`
- `skills: 3 default`

### Custom 표시

`Custom`은 예외 경로이므로
일반 profile과 같은 시각적 무게로 두기보다
“manual override”임이 드러나야 한다.

예:

- `Custom`
- `Manual engine/model`

### RT 연결

RT participant도 profile 기반으로 바뀌었으므로,
가능하면 참가자 카드에서도 profile 이름과 실제 engine/model이 함께 보이게 유지한다.

## 구현 범위

- `src/components/tunaflow/NewMessageInput.tsx`
- 필요 시 profile selector 관련 하위 컴포넌트
- RT participant 표시 영역
- 최소한의 helper/selector 정리

## 비목표

- persona 편집기 구현
- skill auto-selection
- agent execution policy 고도화
- profile collections/shared preset
- settings IA 재설계

## 완료 기준

1. 현재 선택된 profile의 핵심 값이 입력창 근처에서 바로 보인다
2. `Profile ↔ Custom` 전환 시 사용자가 어떤 값이 유지되는지 이해할 수 있다
3. RT participant profile 표시가 profile 기반 구조와 일치한다
4. 사용자가 `agent profile`을 제품의 1급 선택 단위로 체감한다

