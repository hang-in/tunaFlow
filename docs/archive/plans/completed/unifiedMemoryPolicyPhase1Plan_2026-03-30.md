# Unified Memory Policy Phase 1 Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

Long-term memory 로드맵의 앞선 단계는 대부분 열렸다.

- recent context
- structured memory (plan / findings / artifacts)
- compressed conversation memory
- conversation retrieval

이제 문제는 “memory source가 많다”가 아니라,
`어떤 상황에서 무엇을 먼저 넣고 무엇을 양보시킬지`가 코드 전체에 일관되게 고정되지 않았다는 점이다.

즉 다음 단계는 memory source를 하나 더 추가하는 것이 아니라,
기존 memory source들을 하나의 정책으로 통합하는 것이다.

## 목표

ContextPack assembly에서 아래 memory source들을
하나의 `설명 가능한 우선순위 정책`으로 정리한다.

- working memory
- structured task memory
- compressed conversation memory
- retrieval memory

핵심은:
- source priority를 명확히 하고
- overlap/중복 시 어느 층이 살아남는지 정하고
- budget이 부족할 때 무엇이 먼저 잘리는지 고정하는 것이다.

## 왜 필요한가

### 1. 지금은 각 memory layer가 따로 생겨났다

압축 memory, structured memory, retrieval이 각각 필요한 이유로 추가되었지만,
이제는 이들이 서로 경쟁한다.

예:
- retrieval이 structured memory와 같은 내용을 다시 가져올 수 있다
- compressed memory가 plan보다 앞에서 읽히면 안 된다
- recent context와 retrieval이 같은 질문/응답을 중복 전달할 수 있다

### 2. agent-first 품질 기준에 정책이 필요하다

tunaFlow는 단순 UI 앱이 아니라
에이전트가 더 적은 마찰로 더 나은 컨텍스트를 받게 만드는 도구다.

그러려면 memory selection이:
- 예측 가능하고
- budget-aware 하며
- provenance-aware 해야 한다.

### 3. 장기기억은 “더 많이 넣는 것”이 아니라 “더 잘 선택하는 것”이다

memory layer가 늘어날수록
선택 정책이 없으면 토큰 낭비와 노이즈만 커진다.

## 이번 단계에서 할 것

### 1. memory priority 고정

최소 우선순위를 코드와 문서에서 통일한다.

기본안:
1. explicit handoff source
2. working memory (recent context)
3. structured memory (plan / findings / artifacts)
4. retrieval memory
5. compressed memory
6. memo / cross-session / 기타 보조 source

### 2. overlap resolution 규칙 추가

다른 layer와 강하게 겹치는 source는:
- down-rank
- shorten
- skip

중 하나로 처리한다.

최소 규칙:
- structured memory와 retrieval 충돌 시 structured 우선
- recent context와 retrieval 충돌 시 recent 우선
- compressed memory와 structured/retrieval 충돌 시 compressed가 양보

### 3. budget fallback 순서 고정

총 budget이 부족할 때 어느 층부터 줄일지 정한다.

기본 원칙:
- compressed / cross-session / memo가 먼저 양보
- retrieval은 relevance가 낮으면 먼저 줄임
- structured memory는 마지막까지 남김

### 4. trace/readability 보강

가능하면 trace나 context meta에서:
- 어떤 memory layer가 살아남았는지
- 어떤 layer가 줄거나 스킵됐는지
읽을 수 있게 한다.

## 이번 단계에서 하지 않을 것

- vector embedding 추가
- 새로운 memory DB 설계
- 외부 long-term memory stack 도입
- 전면적인 guardrail 엔진 재작성

## 구현 원칙

- 새 memory source를 또 만들지 말고 기존 source selection policy를 정리하라
- 규칙은 설명 가능해야 하며 매직 넘버 남발을 피하라
- agent-first 원칙:
  - 더 많은 정보보다 더 적절한 정보
  - 더 긴 prompt보다 더 높은 task relevance
  - 사람용 설명보다 agent execution 품질 우선

## 성공 기준

- ContextPack memory selection 우선순위가 코드와 문서에서 일관된다
- structured / retrieval / compressed memory 간 역할 충돌이 줄어든다
- budget 부족 시 어떤 layer가 먼저 양보하는지 예측 가능해진다
- trace에서 memory selection 결과를 더 설명할 수 있게 된다

## 후속

이 단계 다음은:

1. retrieval threshold tuning
2. 필요 시 vector/embedding path 재평가
3. 장기기억 정책을 Runtime/Trace surface에 더 노출

순으로 이어진다.
