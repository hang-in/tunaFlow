# tunaFlow Persona Runtime Binding 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 초안

## 목적

`Settings > Personas`와 `Agent Profile`에서 선택된 persona가
실제 agent 실행 시 prompt 조립 경로에 반영되도록 연결한다.

이번 단계의 목표는 persona를 더 많이 만드는 것이 아니라,
이미 선택 가능한 persona가 실제 runtime에서 동작하는지 보장하는 것이다.

## 현재 상태

이미 구현된 것:

- `Settings > Personas`
- built-in persona 목록/편집 UI
- `Agent Profile`이 `personaId`를 참조
- chat input과 RT participant가 `Agent Profile` 기반으로 동작

아직 없는 것:

- 선택 persona의 `promptFragment`가 실제 runtime prompt에 들어가는지에 대한 보장
- 어떤 persona가 현재 요청에 적용되는지 UI에서 재확인할 수 있는 지점
- trace/log/message meta에서 applied persona를 볼 수 있는 최소 가시성

## 핵심 판단

지금은 persona editor를 더 확장하는 것보다,
선택된 persona가 실제 실행 경로에서 쓰인다는 신뢰를 만드는 것이 우선이다.

이유:

1. 사용자는 이미 persona를 선택할 수 있다
2. 하지만 runtime 반영이 보이지 않으면 설정 UI가 장식처럼 느껴진다
3. `Agent Profile` 중심 UX가 완성되려면 persona도 실제 실행 단위로 동작해야 한다

## 목표

### 1. runtime prompt에 persona section 삽입

- persona는 최종 system prompt 전체를 대체하지 않는다
- 기존 normalized prompt 또는 provider별 prompt 조립 경로에
  `persona section` 또는 `persona block`으로 삽입한다
- `project / context / plan / findings / artifacts / skills / rawq`와 충돌하지 않아야 한다

### 2. applied persona 가시성

최소 하나는 필요:

- chat input 근처의 현재 persona 요약
- 전송 직전/직후 message meta
- trace/runtime 영역에서 applied persona 표시

이번 단계에서는 과하지 않게
사용자가 “지금 어떤 persona로 보냈는지”를 재확인할 수 있으면 충분하다.

### 3. provider parity 유지

- Claude만 persona를 받는 구조는 안 된다
- 4개 엔진 모두 동일한 persona section 개념을 공유해야 한다
- provider별 차이는 prompt 조립 방식이지, persona 적용 여부가 아니어야 한다

## 권장 구현 방향

### prompt 조립

- `promptFragment`를 별도 section으로 감싼다
- section 이름은 `Persona` 또는 `Role Contract`처럼 명확해야 한다
- fragment가 비어 있으면 section을 생략할 수 있다

### UI 가시성

가벼운 선택지:

- input selector summary에 persona 유지
- 마지막 assistant message header/meta에 persona 표시
- trace/runtime 영역에 applied persona 뱃지 추가

이번 단계에서는 메인 chat surface 또는 trace 중 한 곳만 먼저 붙여도 충분하다.

## 구현 범위

- prompt 조립 관련 Rust 명령/헬퍼
- persona 데이터 로드/선택 경로
- 최소한의 frontend applied persona 표시

## 비목표

- persona editor 확장
- persona stack/multi-persona
- auto persona recommendation
- prompt template 변수 시스템 고도화
- DB 영속화 대규모 변경

## 완료 기준

1. 선택된 persona가 실제 runtime prompt 조립에 포함된다
2. 4개 엔진 모두 persona section 개념을 공유한다
3. 사용자는 현재 요청에 어떤 persona가 적용됐는지 최소 한 곳에서 확인할 수 있다

