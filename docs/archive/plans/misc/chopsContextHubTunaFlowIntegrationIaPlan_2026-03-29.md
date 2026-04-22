# chops + context-hub + tunaFlow 통합 IA 초안

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 초안

## 목적

`chops`, `context-hub`, `tunaFlow`를 하나의 제품으로 합치려는 것이 아니라,
각자의 강점을 유지한 채 `관리층 / 공급층 / 적용층`으로 역할을 분리하는 통합 구조를 정리한다.

## 핵심 판단

세 도구의 역할은 겹치지 않게 나눠야 한다.

- `chops` = 로컬 skill / agent 관리층
- `context-hub` = docs / skills 검색·획득 공급층
- `tunaFlow` = agent orchestration / runtime 적용층

즉 `tunaFlow`가 모든 것을 직접 관리하거나 편집하는 구조보다,
외부 공급/관리 계층을 받아 실행 흐름에 연결하는 구조가 더 적합하다.

## 1. 계층 정의

### A. 관리층: chops

적합한 역할:

- 로컬 filesystem 기반 skills / agents 스캔
- multi-tool 경로 흡수
- collection / grouping
- source path / 설치 위치 가시화
- search / filter / refresh

가져올 것:

- collection 개념
- toolTargets / source path 메타
- 로컬 설치 상태 가시화
- skill / agent 동시 관리 개념

가져오지 않을 것:

- SwiftUI 앱 전체 UX
- 내장 editor 중심 전략
- macOS 전용 watcher 설계

### B. 공급층: context-hub

적합한 역할:

- docs / skills registry 검색
- private / community source 병합
- 필요한 docs를 task-time fetch
- annotations를 통한 세션 간 학습 보조

가져올 것:

- `docs` 와 `skills` 의 명확한 분리
- `search / get / annotate` 모델
- source trust 개념 (`official / maintainer / community`)
- local/private content registry 방식
- CLI / MCP 인터페이스

가져오지 않을 것:

- content repo 전체를 tunaFlow 내부로 복제
- runtime마다 자동 설치하는 과도한 흐름

### C. 적용층: tunaFlow

적합한 역할:

- Agent Profile 선택
- task/engine/branch/RT 문맥에 맞는 prompt 조립
- applied skills / applied docs / applied persona 가시화
- trace / artifact / review / test workflow 연결

## 2. 사용자 개념

사용자가 직접 다루는 1급 개념:

- `Agent Profile`
- `Project`
- `Conversation / RT / Branch`

사용자가 관리하지만 매번 조작하진 않는 것:

- `Skills`
- `Docs sources`
- `Personas`

시스템이 필요 시 적용하는 것:

- `Applied skills`
- `Fetched docs`
- `Runtime context`

## 3. 정보 구조 방향

### Workspace

- 좌측: Projects / Chats / Artifacts / Files
- 중앙: Chat / Plan / Artifacts / Review / Test
- 우측: RT / Branch overlay
- 하단: runtime status

### Settings

- Agents
- Personas
- Skills
- Knowledge Sources
- Runtime / Tools

여기서:

- `Skills` = chops-style 로컬 registry 관리
- `Knowledge Sources` = context-hub source config / fetch 정책 관리

## 4. 실행 흐름

### 기본 실행

1. 사용자는 `Agent Profile` 선택
2. runtime은 profile의 engine / model / persona / default skills 적용
3. 필요 시 `context-hub`로 docs 검색 / fetch
4. 필요 시 registry skill 또는 local skill을 선택 / 적용
5. 최종 prompt 조립 후 실행
6. 결과에 applied persona / skills / docs를 남김

### 장기 흐름

1. `Agent Profile`이 task를 보고 자동으로 skill 후보를 선택
2. `context-hub annotations`와 local memo가 결합될 수 있음
3. trace / artifact / review에서 실행 근거를 확인

## 5. 주의점

- `tunaFlow`가 skill editor 앱이 되면 안 된다
- `context-hub`는 registry / fetch 층으로 제한해야 한다
- `chops`는 관리 철학 참고용이지 앱 복제 대상이 아니다
- applied visibility가 없으면 auto-selection / auto-fetch는 블랙박스로 느껴진다

## 6. 권장 우선순위

### 먼저

1. `Settings > Skills`를 chops-style registry로 정리
2. `Settings > Knowledge Sources`를 추가
3. context-hub를 CLI/MCP sidecar로 붙이는 계획 정리

### 그 다음

4. applied docs / applied skills visibility
5. flow agent 고도화
6. auto-selection / explainability

## 완료 기준

1. chops / context-hub / tunaFlow의 역할 경계가 문서로 명확해진다
2. Settings에서 `Skills`와 `Knowledge Sources`가 분리된 관리 개념으로 정리된다
3. 향후 `flow agent` 고도화가 붙을 위치가 분명해진다
