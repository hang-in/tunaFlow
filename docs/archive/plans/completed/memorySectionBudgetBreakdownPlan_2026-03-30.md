# Memory Section Budget Breakdown Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

Memory Policy Trace Surface로 이제는:
- 어떤 memory layer가 포함되었는지
- 어떤 layer가 skip되었는지
- retrieval / compressed memory가 왜 빠졌는지

를 Trace/Runtime에서 읽을 수 있게 되었다.

하지만 여전히 중요한 질문이 남아 있다.

예:
- 왜 input이 98k까지 올라갔는가
- 어떤 section이 budget을 가장 많이 먹고 있는가
- retrieval이 문제인지, artifacts인지, rawq인지

현재 active/skipped 정보만으로는 이 질문에 답할 수 없다.

## 목표

ContextPack을 구성하는 각 section의 `길이 기여도`를 Trace/Runtime에서 읽을 수 있게 한다.

핵심은:
- 어떤 section이 몇 chars/tokens를 차지했는지
- 어느 section이 가장 큰 budget consumer인지
- skip되지 않았더라도 과하게 큰 section을 바로 찾을 수 있게 하는 것이다.

## 왜 필요한가

### 1. agent-first 기준에서 낭비 지점을 봐야 한다

tunaFlow는:
- 에이전트가 더 나은 컨텍스트를 받고
- 토큰 낭비 없이 일하게 만드는 것

이 중요하다.

그러려면 단순히 “몇 개 section이 포함되었나”가 아니라,
`무엇이 실제로 prompt를 부풀렸는가`를 봐야 한다.

### 2. memory policy tuning의 다음 근거가 된다

threshold와 selection 정책은 이제 들어갔다.

다음 튜닝은:
- retrieval threshold 조정
- rawq top-K/다해상도 추가 튜닝
- artifacts/findings 압축 조정

같은 식으로 갈 텐데,
section별 budget breakdown이 있어야 우선순위를 정할 수 있다.

### 3. 큰 input이 정상인지 비정상인지 판단할 수 있다

예:
- `artifacts 28k`
- `rawq 18k`
- `cross-session 12k`

처럼 보이면 즉시 병목이 드러난다.

## 이번 단계에서 할 것

### 1. section별 길이 메타 기록

가능하면 ContextPack 조립 시 각 section별:
- chars
- 가능하면 tokens 또는 approximate tokens

를 기록한다.

최소한 chars만 있어도 된다.

### 2. TracePanel에 top budget consumers 표시

예:
- `artifacts 28.4k`
- `rawq 17.9k`
- `plan 6.1k`

상위 3~5개만 보여줘도 충분하다.

### 3. skipped와 별개로 oversized section 표시

skip되지 않았더라도 너무 큰 section은:
- badge
- warning tint
- top consumer 표시

중 하나로 드러낼 수 있다.

### 4. RuntimeStatusBar는 최소 요약만

StatusBar에는 자세한 수치 대신:
- max consumer 이름
- 또는 “top heavy section exists” 정도의 최소 힌트만 둔다.

상세는 TracePanel이 담당한다.

## 이번 단계에서 하지 않을 것

- token accounting 전면 재작성
- section별 hard cap 재설계
- 사용자 직접 per-section budget 조정 UI
- vector retrieval 도입

## 구현 원칙

- policy를 바꾸는 단계가 아니라 관측성을 높이는 단계다
- chars 기준이 먼저고, 정확 token은 후속이어도 된다
- Trace는 상세, StatusBar는 최소 요약
- agent-first 원칙:
  - 더 많은 정보를 넣는 것보다, 무엇이 비용을 먹는지 빨리 알아내는 것이 중요하다

## 성공 기준

- 큰 input run에서 어떤 section이 budget을 가장 많이 먹었는지 바로 보인다
- active/skipped만으로는 안 보이던 병목이 드러난다
- 다음 threshold/compression 튜닝 우선순위를 세우기 쉬워진다

## 후속

이 단계 다음은:

1. top heavy section별 targeted tuning
2. 필요 시 mode별 per-section heuristic 조정
3. 그 후 vector/embedding path 재평가

순으로 이어진다.
