# Settings > Skills / Knowledge Sources 재구성 초안

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 초안

## 목적

현재 `Settings > Skills`를
로컬 registry 관리와 외부 docs/skills 공급 설정으로 분리해
사용자 정신 모델을 더 명확하게 만든다.

## 핵심 판단

`Skills`와 `Knowledge Sources`는 같은 것이 아니다.

- `Skills` = 이미 설치되었거나 로컬에 존재하는 실행 보조 리소스
- `Knowledge Sources` = docs/skills를 검색하고 가져오는 공급원

지금까지 둘이 하나처럼 보일 위험이 있었으므로,
설정 화면에서 분리하는 것이 맞다.

## 추천 Settings 구조

- Agents
- Personas
- Skills
- Knowledge Sources
- Runtime

## 1. Skills

역할:

- 로컬 installed skills / agents 브라우징
- vendor / source path / tool target 확인
- collections / tags / search / refresh

참고 구조:

- chops-style registry

보여줄 것:

- installed 여부
- source path
- vendor
- global / project scope
- collection

지금 하지 않을 것:

- full editor 중심 UX
- 원격 registry 탐색

## 2. Knowledge Sources

역할:

- context-hub source config
- community / internal registry on/off
- source health / last refresh
- docs fetch policy 설명

보여줄 것:

- enabled/disabled
- source list
- CLI/MCP availability
- fetched docs concept 설명

장기적으로:

- source trust 정책
- annotation 상태

## 3. 사용자 워크플로

### Skills

- 사용자는 로컬에 있는 skill/agent를 정리한다
- collections를 관리한다
- agent profile의 default skills와 연결한다

### Knowledge Sources

- 사용자는 어떤 external/internal knowledge source를 쓸지 정한다
- runtime은 필요 시 docs를 가져온다

## 4. 제품 메시지

사용자에게 보여줄 핵심 구분:

- `Skills` = 내가 이미 갖고 있는 것
- `Knowledge Sources` = 실행 중 필요할 때 찾아오는 것

이 구분이 있어야
skill registry와 docs fetch가 같은 UX로 섞이지 않는다.

## 5. 단계별 구현 제안

### Phase 1

- `Settings` 메뉴에 `Knowledge Sources` 추가
- placeholder + status shell
- `Skills` 설명을 로컬 registry 중심으로 보정

### Phase 2

- context-hub CLI health check
- source list / availability 표시

### Phase 3

- fetched docs visibility
- skill/apply / docs/fetch 연결 정리

## flow agent와의 관계

나중에 `flow agent`가 고도화되면:

- `Skills`는 local reusable capability pool
- `Knowledge Sources`는 on-demand research pool

이 둘을 agent가 조합해 사용하게 된다.

## 완료 기준

1. Settings에서 `Skills`와 `Knowledge Sources`가 명확히 분리된다
2. 사용자가 local registry와 external source를 헷갈리지 않게 된다
3. flow agent 고도화 전 필요한 UI/UX 자리 확보가 된다

