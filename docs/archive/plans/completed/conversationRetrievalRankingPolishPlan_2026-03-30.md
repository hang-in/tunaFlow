# Conversation Retrieval Ranking Polish Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

Conversation Retrieval은 이미 두 단계를 거쳤다.

1. FTS5 기반 최소 retrieval layer 도입
2. retrieval 결과를 message 단건에서 pair/chunk 단위로 재조립

이제 남은 문제는 retrieval 결과의 `품질`이다.

현재는:
- FTS hit 순서 의존이 강하고
- 비슷한 주제의 chunk가 반복될 수 있으며
- recent / structured / compressed memory와 겹치는 내용이 retrieval에 다시 들어올 수 있다.

따라서 다음 단계는 retrieval 결과를 더 똑똑하게 `정렬`하고 `중복 제거`하는 것이다.

## 목표

`Relevant prior conversation` 섹션의 품질을 높이기 위해:

- chunk 단위 점수화
- 같은 주제/유사 chunk dedup
- recent / structured / compressed memory와 겹치는 retrieval down-rank

를 추가한다.

핵심은 retrieval 결과를 더 적게, 더 관련성 높게, 더 덜 반복되게 만드는 것이다.

## 왜 필요한가

### 1. chunk 단위 retrieval만으로는 충분하지 않다

pair/chunk 재조립이 되더라도:
- 같은 문제를 다루는 비슷한 Q&A가 2~3개 반복될 수 있고
- 실제로는 현재 질문과 직접 관련 없는 chunk가 위로 올라올 수 있다.

### 2. long-term memory source 간 중복이 생긴다

현재 ContextPack에는 이미:
- recent context
- plan
- findings
- artifacts
- compressed memory

가 존재한다.

retrieval이 이들과 같은 내용을 다시 반복하면 budget만 쓰고 효용은 떨어진다.

### 3. retrieval은 noise에 약하다

FTS5는 빠르고 실용적이지만:
- 키워드 hit가 많으면 과거 대화가 과다 회수될 수 있고
- 같은 표현이 반복된 일반적 Q&A가 상위로 몰릴 수 있다.

따라서 retrieval 결과 자체에 품질 필터가 필요하다.

## 이번 단계에서 할 것

### 1. chunk 점수화

최소 휴리스틱으로 chunk score를 계산한다.

후보:
- query hit 수
- hit가 user/assistant 어느 쪽에 걸렸는지
- recency 가중치
- chunk kind 가중치
- overlap penalty

복잡한 ML/reranker가 아니라 설명 가능한 규칙 기반 점수화만 한다.

### 2. retrieval dedup

같은 주제의 chunk가 반복되면 접거나 제거한다.

최소 기준:
- 동일한 pair/anchor/brief 중복 제거
- 높은 Jaccard 유사도 chunk는 하나만 남기기
- 같은 conversation에서 지나치게 비슷한 chunk가 여러 개 나오면 대표 1개만 유지

### 3. overlap suppression

retrieval chunk가 아래와 강하게 겹치면 down-rank 하거나 제외한다.

- recent context
- current plan / findings / artifacts
- compressed memory

목표는 retrieval이 “새로운 관련 기억”을 가져오게 하는 것이다.

### 4. top-N 재선택

초기 hit 집합을 넓게 잡은 뒤, ranking + dedup을 거쳐 최종 top 3~5만 넣는다.

즉:
- retrieve more
- rank
- dedup
- trim

구조로 정리한다.

## 이번 단계에서 하지 않을 것

- vector embedding 도입
- sqlite-vec
- semantic reranker 모델 도입
- retrieval learning
- cross-project retrieval

## 구현 원칙

- 현재 FTS5 + chunk retrieval 구조를 유지한다
- ranking은 규칙 기반이어야 하고 설명 가능해야 한다
- structured memory보다 retrieval을 앞세우지 말라
- retrieval이 이미 ContextPack에 있는 정보를 반복 주입하지 않게 하라

## 성공 기준

- `Relevant prior conversation`의 중복 chunk가 줄어든다
- recent / plan / findings / artifacts와 겹치는 retrieval이 감소한다
- retrieval 결과가 더 적은 수로 더 높은 관련성을 유지한다
- 코드가 복잡한 ML/reranker 경로 없이도 설명 가능하게 유지된다

## 후속

이 단계 다음은:

1. retrieval ranking threshold 조정
2. 필요 시 turn/chunk grouping 추가 보강
3. 그 후 embedding/vector path 검토

순으로 이어진다.
