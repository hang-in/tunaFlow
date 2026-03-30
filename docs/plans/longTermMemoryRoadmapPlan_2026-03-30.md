# Long-Term Memory Roadmap Plan

상태: 중요 / 제안
작성: 2026-03-30

## 결론

Long-term memory는 tunaFlow에 **반드시 필요한 핵심 기능 축**이다.

현재 tunaFlow는:
- recent messages
- thread inheritance
- cross-session
- plans / findings
- artifacts
- rawq
- context-hub

를 조합해 ContextPack을 만든다.

이 구조만으로도 제품은 작동하지만, 대화가 길어지고 멀티에이전트/RT/branch/eval이 쌓일수록 **최근성(window) 기반 기억만으로는 품질이 무너진다.**

따라서 장기적으로는:
- 구조화된 memory source
- 오래된 대화의 compression memory
- 의미 기반 retrieval

까지 포함하는 long-term memory 구조가 필요하다.

## 왜 중요한가

### 1. recent window는 단기기억일 뿐이다

- 최근 6개, 10개, 12개로 늘리는 것은 단기기억 범위 조정일 뿐
- 장기기억 문제를 해결하지 못한다

### 2. 멀티에이전트 구조에서는 기억 손실이 더 빨리 온다

- agent 전환
- RT 참가자 교체
- artifact handoff
- evaluation

이 많아질수록 최근 메시지 몇 개만으로는 continuity가 깨진다.

### 3. artifact 승격만으로는 부족하다

- 모든 중요한 대화가 artifact로 승격되지는 않는다
- artifact가 아닌 중요한 결정/실수/선호가 장기기억에서 누락될 수 있다

## 현재 수준 평가

현재 tunaFlow의 메모리 구조는 아래처럼 보는 것이 맞다.

### 단기기억

- recent messages
- current thread
- current prompt

### 중기기억

- thread inheritance
- plans / findings
- artifacts
- memo
- cross-session rows

### 장기기억

- 아직 조립형/초기형
- 자동 회상과 구조화 압축이 약하다

## 목표 구조

장기적으로는 아래 4층 구조를 목표로 한다.

### Layer 1. Working Memory

- 최근 대화
- 현재 branch/RT 실행 흐름
- immediate handoff source

### Layer 2. Structured Task Memory

- plans
- findings
- artifacts
- memo
- review/test/eval 결과

### Layer 3. Compressed Conversation Memory

- 오래된 대화/branch/RT를 구조화 요약으로 압축
- 원본 메시지는 유지하되 `compressed` 또는 동등 메타로 working memory에서 제외 가능
- AgentScope식 `SummarySchema`/marked memory 패턴 차용 대상

### Layer 4. Retrieval Memory

- 의미 기반 prior conversation retrieval
- 현재 질문과 관련된 과거 turn/chunk만 다시 회수
- code retrieval(rawq)와 분리된 conversation retrieval

## 권장 구현 순서

### Phase 1. Compressed Memory

우선순위: P0

목표:
- 오래된 recent context를 단순 window가 아니라 요약 memory로 유지
- recent N을 무작정 늘리지 않고 continuity를 보존

후보:
- AgentScope식 memory compression
- `SummarySchema`
- compressed mark
- branch/cross-session summary cache

이 단계는 현재 tunaFlow 구조에 가장 자연스럽게 들어간다.

### Phase 2. Structured Memory 강화

우선순위: P0

목표:
- artifacts / plans / memos / findings를 더 명확한 memory source로 쓴다
- long-term memory의 진실원을 “대화”만이 아니라 구조화 객체까지 확장

후보:
- artifact summary 우선순위 강화
- memo scope/importance 정교화
- cross-session inclusion 정책 개선

### Phase 3. Conversation Retrieval

우선순위: P1

목표:
- 현재 prompt와 의미적으로 연관된 과거 turn/chunk를 회수
- recent window 밖의 중요한 대화를 다시 가져온다

기준 문서:
- `conversationVectorSearchPlan.md`

원칙:
- rawq 대체가 아니라 conversation memory retrieval
- current project scoped only
- top 3~5 chunk
- recent window와 중복 제거

### Phase 4. Unified Memory Policy

우선순위: P1

목표:
- working/compressed/structured/retrieval memory를 한 정책으로 통합
- ContextPack assembly에서 memory source selection 우선순위를 고정

예:
- current task relevance
- provenance
- recency
- memory type priority
- budget-aware selection

## 하지 말아야 할 것

- recent window만 12개, 20개로 계속 늘려서 장기기억 문제를 덮기
- 바로 mem0/ReMe 같은 외부 long-term memory 스택을 도입하기
- vector retrieval를 먼저 넣고 compression/structured memory를 건너뛰기
- code retrieval(rawq)와 conversation retrieval를 같은 문제로 취급하기

## 참고 문서

- `docs/explanation/agentscopeAnalysis.md`
- `docs/plans/conversationVectorSearchPlan.md`
- `docs/plans/contextPackCompressionAndRawqPostprocessPlan_2026-03-30.md`
- `docs/plans/contextPackP0Phase1Plan_2026-03-30.md`

## 현재 권장 결론

장기기억의 첫 구현은:

1. `memory compression`
2. `structured memory source 강화`

가 맞다.

그 다음에:

3. `conversation vector retrieval`

로 넘어가야 한다.

즉 지금 tunaFlow에서 long-term memory는 “언젠가 있으면 좋은 기능”이 아니라, **성숙한 프로덕션 수준으로 가기 위해 반드시 필요한 핵심 기능**으로 봐야 한다.
