# Top Heavy Section Tuning Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

이제 Trace/Runtime에서 ContextPack의 section별 budget 기여도를 볼 수 있게 되었다.

즉 더 이상:
- input이 왜 큰지 추측만 하지 않아도 되고
- 어떤 section이 budget을 가장 많이 먹는지 바로 확인할 수 있다.

다음 단계는 memory policy를 또 바꾸는 것이 아니라,
실제로 가장 큰 section들을 하나씩 줄이는 것이다.

## 목표

상위 budget consumer section에 대해:
- 왜 큰지 파악하고
- 어떤 방식으로 줄일지 정하고
- agent 품질 손실이 적은 축소 규칙을 적용한다.

핵심은 “모든 section을 조금씩 줄이기”가 아니라,
가장 무거운 section 몇 개를 targeted tuning 하는 것이다.

## 왜 필요한가

### 1. 이제는 병목이 보인다

예:
- artifacts 28k
- rawq 17k
- cross-session 12k

처럼 보이면 전체 input budget 문제는
정책이 아니라 특정 section의 과대 기여 문제일 가능성이 높다.

### 2. agent-first 기준에 맞는 최적화가 가능하다

에이전트에게 중요한 것은:
- 더 많은 정보를 넣는 것보다
- 더 적절한 정보를 적절한 해상도로 넣는 것이다.

즉 section별로:
- 유지해야 할 핵심
- 줄여도 되는 반복
- 해상도를 낮춰도 되는 부분

을 다르게 다뤄야 한다.

### 3. 다음 threshold/vector 논의보다 ROI가 높다

지금은 retrieval/vector를 더 키우기보다,
이미 들어가는 section 중 과한 곳을 줄이는 편이 더 직접적인 효과가 있다.

## 이번 단계에서 할 것

### 1. top heavy section 우선순위 정리

최근 trace 기준으로 가장 자주 상위에 뜨는 section을 본다.

예상 후보:
- artifacts
- rawq
- cross-session
- findings
- retrieval

실제 코드/trace 기준으로 상위 2~3개만 먼저 다룬다.

### 2. section별 축소 전략 적용

예시:

#### artifacts
- 최근/승인/관련 artifact만 유지
- 긴 content는 title + summary + 핵심 excerpt 위주로
- 중복 artifact는 접기

#### rawq
- top-N 자체보다 해상도 조정 우선
- full/skeleton/reference 비율 재조정
- import fold 이후에도 긴 snippet은 더 축소

#### cross-session
- Jaccard fold 더 공격적으로
- relevance 낮은 row 제외
- 반복된 상태 설명 축소

### 3. section별 “최대 허용 해상도” 명확화

모든 section이 full text일 필요는 없다.

각 section에 대해:
- full
- summary
- excerpt
- reference

중 어느 해상도가 기본인지 더 명확히 한다.

### 4. trace로 개선 확인

변경 후:
- top consumers가 줄었는지
- input budget이 실제로 내려갔는지
- structured memory 가독성이 좋아졌는지

를 확인한다.

## 이번 단계에서 하지 않을 것

- memory policy 전체 재설계
- vector retrieval 도입
- per-section 사용자 조정 UI
- token accounting 전면 재작성

## 구현 원칙

- 실제로 무거운 section부터 다룬다
- section마다 같은 방식으로 줄이지 말고, 성격에 맞는 해상도 전략을 쓴다
- agent 품질 손실이 큰 정보는 남기고, 반복/장식/저신호 정보부터 줄인다

## 성공 기준

- top heavy section 2~3개의 기여도가 눈에 띄게 감소한다
- 큰 input run에서 총 input이 줄어든다
- agent가 필요한 핵심 정보는 유지된다
- trace에서 줄어든 효과를 확인할 수 있다

## 후속

이 단계 다음은:

1. 추가 section tuning
2. mode별 per-section heuristic 조정
3. 그 후 vector/embedding path 재평가

순으로 이어진다.
