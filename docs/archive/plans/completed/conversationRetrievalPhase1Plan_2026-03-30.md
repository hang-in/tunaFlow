# Conversation Retrieval Phase 1 Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

Long-term memory의 1, 2단계는 이미 열렸다.

- compressed conversation memory
- structured memory source 강화

이제 남은 3번째 축은 **현재 질문과 의미적으로 연관된 과거 대화 turn/chunk를 다시 회수하는 retrieval layer**다.

기존 `conversationVectorSearchPlan.md`는 검토 메모 성격이 강하므로,
이번 문서는 현재 구조 기준의 **최소 실험 경로**를 고정한다.

## 목표

프로젝트 범위 안에서, 현재 질문과 의미적으로 연관된 과거 대화 chunk를 회수해 ContextPack에 붙이는 최소 retrieval 실험을 만든다.

핵심은:
- recent window 대체가 아님
- rawq 대체가 아님
- long-term conversation memory 보조층

## 왜 지금 필요한가

### 1. recent + compressed memory만으로는 충분하지 않다

- 오래된 대화를 압축해도
- 현재 질문과 관련된 특정 예전 turn을 정밀하게 다시 찾는 기능은 없다

### 2. structured memory도 빈틈이 있다

- artifact로 승격되지 않은 중요한 과거 대화
- 예전 branch/RT의 특정 결정
- 반복적으로 등장하는 논점

은 retrieval 없이 다시 꺼내기 어렵다.

### 3. 이제는 retrieval을 붙일 기반이 생겼다

- ContextPack visibility
- compression/rawq post-processing
- compressed memory
- structured memory policy

까지 정리되어, retrieval이 추가 noise가 되는지 아닌지 볼 수 있는 상태가 됐다.

## 이번 단계에서 할 것

### 1. Turn/Chunk 단위 conversation memory 구조 실험

메시지 단건보다:
- user + assistant 한 쌍
- branch anchor + 직후 응답
- RT brief 같은 chunk

가 retrieval 결과 단위로 더 적합하다.

이번 단계는 최소 구조로 시작한다.

### 2. 프로젝트 범위 retrieval만 허용

- current project scoped only
- cross-project retrieval 금지

### 3. ContextPack 통합 위치 고정

권장 위치:

1. thread anchor / parent turns
2. current recent context
3. **relevant prior conversation**
4. plan / findings / artifacts / rawq

### 4. 최근 대화와 중복 제거

- recent window와 겹치는 chunk는 제외
- compressed memory와 완전히 중복되는 retrieval도 가능하면 줄인다

## 이번 단계에서 하지 않을 것

- sqlite-vec 최적화
- 외부 vector DB
- generic semantic memory engine
- project 밖 retrieval
- retrieval 결과 자동 importance 학습

## 구현 원칙

### 1. 실험은 작게

- 먼저 품질/노이즈를 본다
- 최적화는 나중

### 2. rawq와 문제를 섞지 않는다

- rawq = code retrieval
- conversation retrieval = prior conversation memory

### 3. recent window를 대체하지 않는다

- retrieval은 보조층
- current working memory는 계속 recent messages가 담당

## 권장 접근

### Phase 1A

- chunk 데이터 구조 초안
- project-scoped retrieval 경로

### Phase 1B

- ContextPack에 `Relevant prior conversation` 삽입
- top 3~5 chunk

### Phase 1C

- duplicate/noise 검토
- compressed memory와의 역할 차이 확인

## 성공 기준

- 현재 질문과 관련 있는 예전 대화 chunk가 recent window 밖에서도 회수된다
- retrieval이 rawq나 compressed memory와 역할 충돌 없이 보조층으로 동작한다
- noise가 과도하지 않다

## 후속

이 단계 다음은:

1. 임베딩 경로 확정
2. sqlite-vec 같은 최적화 검토
3. unified memory policy

순으로 이어진다.
