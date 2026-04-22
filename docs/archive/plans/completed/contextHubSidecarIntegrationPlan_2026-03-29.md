# context-hub Sidecar / CLI / MCP 도입 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 초안

## 목적

`context-hub`를 tunaFlow 내부 기능으로 재구현하지 않고,
`CLI / MCP / sidecar` 형태의 외부 의존성으로 도입하는 방향을 정리한다.

## 핵심 판단

`context-hub`는 `rawq`와 비슷하게
제품이 호출하는 외부 지식 도구로 다루는 것이 맞다.

이유:

1. 이미 `search / get / annotate` CLI가 존재한다
2. MCP 인터페이스도 제공한다
3. docs / skills registry는 tunaFlow의 core domain이 아니라 보조 공급층이다
4. 내부 재구현보다 sidecar/CLI 연동이 유지보수 비용이 낮다

## 도입 방식 비교

### 1. 레포 내부 재구현

장점:

- 단일 코드베이스

단점:

- 검색/registry/MCP 기능을 다시 만들어야 함
- upstream 개선을 받기 어렵다

판단:

- 비추천

### 2. CLI 호출

예:

- `chub search`
- `chub get`
- `chub annotate`

장점:

- 구현이 가장 단순
- 기존 rawq 연동 패턴과 유사

단점:

- structured output / streaming 제어가 다소 제한적

판단:

- 1차 도입에 가장 현실적

### 3. MCP 클라이언트 연동

장점:

- structured tool call
- 향후 agent runtime과 더 자연스럽게 연결 가능

단점:

- tunaFlow 쪽 MCP client/runtime 설계가 더 필요

판단:

- 2차 확장에 적합

## 권장 단계

### Phase 1. CLI integration

목표:

- `context-hub` 존재 여부 감지
- `search / get` 최소 명령 연동
- `Knowledge Sources` 상태 진단 표시
- fetched docs를 runtime prompt에 붙일 수 있는 경로 확보

### Phase 2. annotations / source policy

목표:

- local annotations 읽기/쓰기
- source trust / source enable 설정
- private registry source 연결

### Phase 3. MCP integration

목표:

- structured search/get tool call
- long-term flow agent / automatic research path와 연동

## tunaFlow에 필요한 최소 기능

### 검색

- query
- tags/lang/source filter는 후순위

### fetch

- doc entry fetch
- full fetch보다 entry point 우선

### applied visibility

- 어떤 docs를 붙였는지
- source가 무엇인지
- annotation이 있었는지

## UI/UX 위치

### Settings > Knowledge Sources

- context-hub enabled 여부
- source 목록
- community/internal source 상태
- last refresh / health

### Chat / Runtime

- fetched docs badge
- applied docs count
- detail modal 또는 trace meta

## 비목표

- context-hub content repo 내장
- docs editor 내장
- 모든 요청에 자동 search/get 실행
- 1차부터 MCP 의존

## flow agent와의 관계

`flow agent`를 고도화할 때
context-hub는 가장 자연스러운 `research / docs fetch` 공급층이 된다.

즉 순서는:

1. context-hub 최소 연동
2. applied docs visibility
3. 그 다음 flow agent가 task에 따라 docs를 자동 fetch/annotate

## 완료 기준

1. CLI 기반 도입이 가능한지와 범위가 정리된다
2. MCP는 후속 단계로 분리된다
3. flow agent 고도화 시 context-hub가 들어갈 위치가 명확해진다

