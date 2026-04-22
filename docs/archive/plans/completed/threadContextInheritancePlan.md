# tunaFlow Thread / RT Context Inheritance 설계

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 20:40 KST

## 목적

현재 `tunaFlow`의 thread와 RT는 프로젝트 경계 안에서 동작하지만,
실제 모델 입력에서는 부모 대화의 local context가 충분히 상속되지 않아
"같은 프로젝트인데도 이전 대화를 모른다"는 응답이 나올 수 있다.

이 문서는 thread / RT가 어떤 컨텍스트를 기본으로 상속해야 하는지,
무엇은 압축해서 넘기고 무엇은 anchor/source로 명시해야 하는지 정리한다.

## 문제 정의

현재 문제는 두 경계가 섞여 있기 때문이다.

- 프로젝트 경계
  - codebase
  - rawq
  - plans / findings / artifacts
- 대화 경계
  - 부모 메시지
  - 최근 몇 턴
  - 분기를 만든 이유

즉 "같은 프로젝트"라고 해서 자동으로 "같은 세션 맥락"이 되는 것은 아니다.

## 핵심 원칙

### 1. 프로젝트 컨텍스트는 기본 상속

thread와 RT도 프로젝트 안의 작업 단위이므로 아래는 기본 상속하는 것이 맞다.

- project path
- codebase context
- rawq context
- 향후 graph context
- plan / findings / artifacts
- project-level instructions

즉 프로젝트/코드베이스 맥락은 항상 자동 상속이 원칙이다.

### 2. 대화 전체를 통째로 넘기지는 않는다

부모 conversation 전체를 그대로 넘기면:

- 토큰 낭비
- 오래된 맥락 오염
- 렌더링 부담
- 토론 초점 분산

이 생긴다.

따라서 대화 맥락은:

- anchor
- recent turns
- short summary

중심으로 압축 상속하는 것이 맞다.

### 3. explicit source는 recent turns보다 우선한다

artifact, plan, message 같은 명시적 source가 있으면
이는 최근 대화보다 우선해야 한다.

즉 우선순위는:

1. explicit source
2. anchor message
3. recent turns

이다.

## 권장 컨텍스트 계층

### Layer 1. Base Project Context

항상 포함:

- project identity
- codebase context
- rawq section
- plan/findings/artifacts summary
- project instructions

가능하면 기존 `ContextPack`을 재사용한다.

### Layer 2. Inherited Conversation Context

분기/RT 생성 시 포함:

- parent conversation id
- anchor message id
- recent user/assistant turns 2~4개
- 짧은 inherited summary(후속 단계)

### Layer 3. Explicit Source Context

명시적 source가 있으면 최우선 포함:

- selected message
- selected artifact
- selected plan
- RT brief
- adopt summary

### Layer 4. Thread / RT Instruction

마지막으로:

- 왜 이 thread가 열렸는지
- RT에서 무엇을 토론해야 하는지
- 어떤 관점으로 이어가야 하는지

를 instruction으로 명시한다.

## 일반 thread와 RT의 차이

### 일반 thread

부모 대화의 연장선이 더 강하므로:

- base project context
- anchor message
- recent local turns

을 더 풍부하게 가져가는 것이 맞다.

### RT

토론/비교 목적이 강하므로:

- base project context
- explicit source
- anchor message
- 짧은 recent turns

을 우선하고, 전체 recent turns는 더 압축하는 것이 좋다.

즉 RT는 일반 thread보다 "대화 압축"이 더 강해야 한다.

## 구현 방향

### 1. 기존 ContextPack 확장

새 시스템을 만들기보다 기존 ContextPack 위에 inheritance layer를 얹는 것이 맞다.

예:

- `build_thread_inheritance_section(...)`
- `build_rt_inheritance_section(...)`

또는 동등한 helper

### 2. 생성 시점 snapshot은 후속

1차는 매 실행 시:

- explicit source
- anchor message
- recent turns

를 조립하는 것으로 충분하다.

2차에서:

- inherited summary
- snapshot artifact/memo

를 도입하면 된다.

## 추천 우선순위

### Phase 1

- parent anchor message 자동 포함
- explicit source 최우선 포함
- recent turns 2~3개 포함

### Phase 2

- inherited summary artifact 또는 memo
- branch/thread 생성 시 snapshot 저장

### Phase 3

- context mode 분리
  - thread: richer local context
  - RT: concise inherited context

## 테스트 포인트

### A. RT 시작

- 메시지에서 `RT 분기`
- anchor message 내용이 실제 참가자 prompt에 포함되는지

### B. 일반 thread 시작

- 부모 메시지와 최근 대화가 thread 입력에 들어가는지

### C. explicit source 우선순위

- artifact/plan/message 선택 상태에서 해당 source가 recent turns보다 우선하는지

### D. 과다 컨텍스트 방지

- 전체 대화가 통째로 들어가지 않는지
- prompt가 과도하게 길어지지 않는지

## 현재 판정

thread와 RT는 프로젝트/코드베이스 맥락을 기본 상속해야 한다.
다만 대화 전체를 통째로 넘기는 대신,

- project context는 기본 상속
- local conversation context는 요약 + 최근 턴 + anchor
- explicit source는 우선 상속

으로 나누는 것이 가장 정확하고 가볍다.
