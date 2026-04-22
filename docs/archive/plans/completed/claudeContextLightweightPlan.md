# tunaFlow Claude Context 경량화 1차 계획

작성 목적:
- `tunaFlow`에서 Claude 경로가 느린 핵심 원인을 줄이기 위한 현실적인 1차 범위를 정의한다.
- `tunaChat` 참고 기준으로, 매 요청마다 무거운 ContextPack을 모두 조립하는 구조를 바로잡는 것이 목표다.

## 현재 문제 요약

현재 `tunaFlow`의 Claude 경로는 매 요청마다 아래를 거의 모두 조립한다.

- skills
- plan
- findings
- artifacts
- rawq
- cross-session
- context summary

특히 rawq는 semantic search까지 포함될 수 있어, 단순 질문에도 수 초 지연을 만들 수 있다.

반면 `tunaChat`는:
- send payload는 얇고
- project/context 정보는 별도 로드/캐시하며
- rawq는 항상 자동 주입하지 않는다

## 1차 목표

1차에서는 다음만 한다.

1. Claude 요청의 기본 컨텍스트를 `lite` 기준으로 낮춘다
2. rawq를 항상 실행하지 않고 조건부로 실행한다
3. context mode 개념을 도입해 이후 확장이 가능하게 만든다

즉, "모든 것을 미리 캐시"까지는 가지 않는다.

## 이번 단계에서 포함할 것

### 1. context mode 도입

최소 mode:

- `lite`
- `standard`
- `full`

권장 기본값:
- 기본 일반 대화: `lite`
- branch / follow-up / plan 관련 요청: `standard`
- 명시적 deep analysis 또는 사용자가 원할 때만 `full`

### 2. rawq 조건부 실행

rawq는 기본 always-on을 중단한다.

권장 규칙:
- 파일명, 경로, 함수, 클래스, 구현, 코드, 구조 같은 신호가 있으면 실행
- 아주 짧은 일반 질문이면 skip
- timeout은 유지

즉:
- "코드 관련 가능성이 높은 질문"에서만 rawq

### 3. 기본 Claude prompt 축소

`lite`에서는 다음 정도만 포함한다.

- project path
- base system prompt
- 짧은 context summary

선택적으로:
- skills는 정말 필요한 경우만

즉 `lite`에는:
- plan
- findings
- artifacts
- cross-session
- rawq
를 기본으로 넣지 않는다

### 4. `standard`는 선택적 확장

`standard`는:
- plan
- findings
- artifacts
중 일부를 포함한다

예:
- follow-up
- plan/subtask 기반 실행
- branch thread

### 5. `full`은 명시적 상황만

`full`은:
- rawq
- cross-session
- full context summary
를 포함 가능

예:
- "이 코드베이스 구조 전체를 분석해"
- "관련 파일까지 다 찾아서 검토해"

## 이번 단계에서 하지 않을 것

- `project.context` 캐시 계층 본격 도입
- context prefetch 백그라운드 작업
- 전체 엔진 공통 context mode 통합
- sidecar 기반 context orchestration
- rawq relevance scoring 고도화

## 구현 우선순위

### Phase 1A

- `ContextMode` enum 또는 동등 개념 추가
- Claude 기본 경로를 `lite`로 전환

### Phase 1B

- rawq 조건부 실행 추가
- timeout/skip 이유를 로그로 남김

### Phase 1C

- follow-up / branch / plan 기반 요청에 `standard` 적용

## 후속 작업

### 후속 1. project context 사전 로드/캐시

`tunaChat`처럼:
- 프로젝트 선택 시 context 준비
- send 시에는 캐시된 값을 읽는 구조

### 후속 2. rawq relevance 개선

- 질문 유형 판별 정교화
- artifact/plan/path 신호와 연계
- 필요 시 lazy search

### 후속 3. 엔진별 context policy 통합

- Claude만이 아니라
- Codex/Gemini/OpenCode도 mode 기반으로 정리

### 후속 4. sidecar 도입 후 context orchestration 통합

- `sidecarMigrationPlan`과 연결
- model catalog, availability, progress, context 정책을 sidecar와 정합화

### 후속 5. UI에서 context mode 가시화

- 현재 요청이 `lite / standard / full`인지
- 필요하면 사용자가 override 가능하게

## 검증 포인트

1. 단순 질문에서 rawq가 더 이상 자동 실행되지 않는지
2. Claude 첫 응답 체감이 개선되는지
3. branch/follow-up 같은 문맥 작업에서 필요한 정보가 너무 빠지지 않는지
4. `standard`와 `full`로 올라가는 조건이 과하지 않은지

## 최종 판단

현실적인 1차 범위는:
- `rawq 조건부 실행`
- `기본 lite context`
- `mode 분리 도입`
까지다.

`project.context` 캐시 계층은 중요하지만 후속으로 미루는 것이 안전하다.
