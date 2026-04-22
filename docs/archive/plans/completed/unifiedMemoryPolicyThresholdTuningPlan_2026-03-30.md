# Unified Memory Policy Threshold Tuning Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

Unified Memory Policy Phase 1로 memory selection 순서와 fallback 규칙은 정리되었다.

현재 기준:
- retrieval는 `remaining > 4000`일 때만 포함
- compressed memory는 `remaining > 2000`일 때만 포함

이 값들은 합리적인 시작점이지만, 아직 경험적 기준이다.

즉 다음 단계는 policy 구조를 더 바꾸는 것이 아니라,
현재 threshold가 실제 사용 흐름에서 적절한지 조정하는 것이다.

## 목표

retrieval / compressed memory inclusion threshold를 실제 ContextPack 조립 결과에 맞게 조정한다.

핵심은:
- retrieval이 너무 자주 skip되지 않는지
- compressed memory가 너무 쉽게 밀려나지 않는지
- 반대로 budget을 과도하게 잡아먹지 않는지

를 확인하고, 설명 가능한 threshold로 다듬는 것이다.

## 왜 필요한가

### 1. 현재 값은 구조상 맞지만 데이터 기반 튜닝이 아니다

`4000 / 2000`은 안전한 시작점이지만,
실제 대화 패턴에서는:
- retrieval이 거의 항상 skip될 수 있고
- compressed memory가 너무 늦게만 들어올 수 있다.

### 2. agent-first 기준에서는 “있어야 할 기억이 자주 빠지면” 안 된다

tunaFlow는 단순 UX보다
에이전트가 필요한 기억을 덜 놓치게 하는 것이 중요하다.

즉 threshold는 보수적이기만 해서는 안 되고,
실제로 도움이 되는 기억이 적절히 살아남아야 한다.

### 3. 반대로 무턱대고 threshold를 낮추면 토큰 낭비가 커진다

retrieval / compressed memory가 너무 쉽게 들어오면:
- prompt 길이 증가
- 중복 증가
- structured memory 가독성 저하

가 생길 수 있다.

## 이번 단계에서 할 것

### 1. threshold 점검용 계측 보강

가능하면 trace/meta에서:
- retrieval included/skipped 빈도
- compressed memory included/skipped 빈도
- skip 이유
- 남은 budget 분포

를 더 쉽게 파악할 수 있게 한다.

### 2. retrieval threshold 조정

현재 `remaining > 4000` 기준을 검토한다.

가능한 방향:
- 절대값 조정
- mode별 차등 threshold
- retrieval hit score가 높을 때만 완화

핵심은 retrieval이 “항상 빠지는 보조층”이 되지 않게 하는 것이다.

### 3. compressed memory threshold 조정

현재 `remaining > 2000` 기준을 검토한다.

가능한 방향:
- stale/fresh 상태에 따라 차등
- summary length에 따라 유연화
- structured memory가 작을 때 더 쉽게 포함

### 4. hardcoded 상수 정리

가능하면 threshold를:
- 상수명
- 주석
- mode별 규칙

수준으로 더 읽기 쉽게 정리한다.

## 이번 단계에서 하지 않을 것

- vector embedding
- memory source 우선순위 재설계
- 새로운 memory layer 추가
- Runtime에서 사용자 직접 threshold 조정 UI 추가

## 구현 원칙

- threshold tuning은 policy correction이지 architecture rewrite가 아니다
- structured memory 우선 원칙은 유지한다
- retrieval / compressed memory는 여전히 보조층이어야 한다
- 과한 토큰 사용보다 높은 task relevance를 우선하되, relevance 없는 반복은 늘리지 말라

## 성공 기준

- retrieval / compressed memory skip 빈도가 더 설명 가능해진다
- 적절한 상황에서 retrieval과 compressed memory가 더 자주 살아남는다
- budget tight 상황에서 structured memory 우선 원칙은 유지된다
- threshold 규칙이 코드에서 읽기 쉬워진다

## 후속

이 단계 다음은:

1. memory policy trace surface 추가 보강
2. 필요 시 mode별 threshold 세분화
3. 그 후 vector/embedding path 재평가

순으로 이어진다.
