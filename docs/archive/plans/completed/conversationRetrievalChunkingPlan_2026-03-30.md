# Conversation Retrieval Chunking Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

Conversation Retrieval Phase 1은 FTS5 기반으로 최소 retrieval layer를 여는 데 성공했다.

하지만 현재 단위는 `메시지 단건`이다. 이 구조는:
- user 질문만 회수되거나
- assistant 응답만 회수되거나
- 문맥상 한 쌍이어야 할 내용이 반쪽으로 들어오는

문제를 만든다.

따라서 다음 단계는 retrieval 결과 단위를 `의미 단위 chunk`로 끌어올리는 것이다.

## 목표

대화 retrieval 결과를 `message`가 아니라 `turn/pair/chunk` 단위로 회수하도록 개선한다.

핵심은:
- 질문+응답을 함께 회수
- branch anchor / RT brief처럼 단일 메시지라도 의미 단위면 chunk로 취급
- retrieval 결과가 ContextPack에 더 자연스럽게 들어가게 만드는 것

## 왜 필요한가

### 1. 반쪽 회수 문제

메시지 단건 검색은 현재 질문과 의미적으로 관련 있어도,
- 질문만 나오고 답변이 빠지거나
- 답변만 나오고 맥락이 빠질 수 있다

### 2. ContextPack에 넣기 애매하다

`Relevant prior conversation`은 단순 검색 결과가 아니라
현재 질문에 도움이 되는 과거 대화 단위여야 한다.

### 3. retrieval 품질을 올리면서 vector path 이전 단계를 만든다

벡터/임베딩 경로로 가기 전에,
우선 retrieval 결과 단위를 올바르게 만드는 것이 더 중요하다.

## 이번 단계에서 할 것

### 1. chunk 규칙 정의

최소 규칙:
- user + 직후 assistant = pair chunk
- branch anchor 단일 메시지 = anchor chunk
- RT brief 단일 요약 = brief chunk

### 2. retrieval 결과를 chunk로 재조립

- FTS5 hit가 user 또는 assistant 어느 쪽이든
- 최종 결과는 pair/chunk 단위로 재구성한다

### 3. 중복 제거

- 같은 pair/chunk가 여러 hit로 반복되지 않게 한다
- recent window와 겹치면 제외한다

### 4. ContextPack 표시 개선

- `Relevant prior conversation`에서
  - pair
  - anchor
  - brief
같은 kind를 최소 수준으로 표시할 수 있다면 더 좋다

## 이번 단계에서 하지 않을 것

- vector embedding 도입
- sqlite-vec
- retrieval learning/reranking
- project 밖 retrieval

## 구현 원칙

- retrieval 엔진은 당장 FTS5 그대로 써도 된다
- 핵심은 검색 엔진이 아니라 retrieval 결과 단위 개선이다
- recent / compressed / structured memory와 역할 충돌을 만들지 말라

## 성공 기준

- retrieval 결과가 질문/응답 쌍 또는 의미 단위 chunk로 들어온다
- 반쪽 메시지 회수가 줄어든다
- ContextPack 내 `Relevant prior conversation`이 더 읽기 쉬워진다

## 후속

이 단계 다음은:

1. retrieval ranking 개선
2. embedding/vector path 검토
3. unified memory policy

순으로 이어진다.
