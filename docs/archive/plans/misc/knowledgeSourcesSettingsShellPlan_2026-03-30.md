# tunaFlow Settings > Knowledge Sources Shell MVP 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30
- 상태: 초안

## 목적

`Settings` 안에 `Knowledge Sources` 섹션을 추가해,
외부 docs/skills 공급원을 제품 개념으로 먼저 자리 잡게 만든다.

이번 단계는 실제 `context-hub` 연동보다
사용자에게 “local skills”와 “external knowledge sources”가 다른 개념이라는 점을
명확히 보여주는 제품 셸을 만드는 것이 목표다.

## 핵심 판단

지금은 flow agent보다 제품 구조가 먼저다.

즉:

1. `Settings > Skills`는 로컬 registry 관리
2. `Settings > Knowledge Sources`는 외부/내부 docs 공급원 관리

이 구분이 먼저 눈에 보여야
이후 `context-hub` 연동과 `flow agent` 고도화가 자연스럽다.

## 현재 상태

이미 있는 것:

- `Settings` shell
- `Agents / Personas / Skills / Runtime` 섹션
- `Skills`는 settings로 이동 완료
- `context-hub` 도입 방향 문서 존재

아직 없는 것:

- `Knowledge Sources` 섹션 자체
- source 개념 설명
- external/internal source 상태 shell
- 향후 `context-hub`가 들어갈 UI 자리

## 목표

### 1. Settings에 `Knowledge Sources` 섹션 추가

최소 구성:

- 좌측 settings nav에 `Knowledge Sources`
- 우측 본문에 shell UI

### 2. 제품 개념 설명

반드시 보여줘야 하는 구분:

- `Skills` = 내가 이미 갖고 있는 것
- `Knowledge Sources` = 실행 중 필요할 때 찾는 것

### 3. 향후 연결점 자리 확보

placeholder라도 아래 항목은 보여줘야 한다:

- source status
- community/internal source 개념
- CLI/MCP availability 자리
- fetched docs가 나중에 어디에 쓰이는지 설명

## 권장 UX

### Information blocks

예시:

- `Knowledge Sources`
  - 설명 텍스트
  - `context-hub` integration status
  - `Community source`
  - `Internal source`
  - `Future: fetched docs visibility`

### 상태 톤

지금은 연동 전이므로
`Not configured`, `Planned`, `Coming next` 같은 상태가 적절하다.

구현되지 않은 기능을 구현된 것처럼 보이게 하면 안 된다.

## 구현 범위

- Settings nav 수정
- `Knowledge Sources` section shell/placeholder
- `Skills` section과의 설명 경계 보강

## 비목표

- 실제 `context-hub` CLI health check
- source enable/disable 저장
- fetched docs 실행 연동
- flow agent 구현

## 완료 기준

1. Settings에 `Knowledge Sources`가 보인다
2. `Skills`와 `Knowledge Sources`의 차이가 UI 텍스트로 명확하다
3. 이후 `context-hub`/flow agent 작업이 붙을 제품 자리가 확보된다

