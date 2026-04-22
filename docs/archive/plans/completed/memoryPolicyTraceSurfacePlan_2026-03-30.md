# Memory Policy Trace Surface Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

Unified Memory Policy와 threshold tuning까지 끝나면서,
이제 ContextPack memory selection은 내부 규칙으로는 꽤 정리되었다.

현재는 로그와 `included_sections`를 통해:
- retrieval included/skipped
- compressed-memory included/skipped
- budget tight 이유

정도를 확인할 수 있다.

하지만 이 정보는 아직 개발자/로그 중심이다.

다음 단계는 memory policy 결과를
Trace/Runtime surface에서 더 직접 읽을 수 있게 만드는 것이다.

## 목표

TracePanel과 가능하면 RuntimeStatusBar에서:
- 어떤 memory layer가 실제 포함되었는지
- 어떤 layer가 skip되었는지
- 어떤 threshold가 적용되었는지
- 왜 retrieval / compressed memory가 빠졌는지

를 사람이 빠르게 파악할 수 있게 한다.

핵심은 “memory policy가 어떻게 작동했는지”를
trace metadata 해석 없이 바로 읽게 만드는 것이다.

## 왜 필요한가

### 1. 정책이 생겼으면 설명 가능해야 한다

지금 tunaFlow는:
- working memory
- structured memory
- retrieval
- compressed memory

를 budget과 overlap에 따라 선택한다.

이 구조는 agent-first 품질에 좋지만,
왜 어떤 기억이 빠졌는지 안 보이면 운영과 튜닝이 어렵다.

### 2. retrieval/compressed memory가 빠졌을 때 진단이 필요하다

현재는:
- `remaining 2340 < threshold 4000`
같은 로그로는 볼 수 있다.

하지만 TracePanel에서 바로 보이지 않으면
실제 사용 흐름에서 해석이 어렵다.

### 3. 다음 단계 threshold tuning과 vector 재평가에도 기반이 된다

memory layer visibility가 잘 되어 있어야:
- cutoff가 너무 보수적인지
- 어느 mode에서 retrieval이 거의 안 살아남는지
- compressed memory가 언제 의미 있게 쓰이는지

를 판단할 수 있다.

## 이번 단계에서 할 것

### 1. TracePanel memory policy 요약 추가

각 trace/run에 대해 최소한 아래를 읽을 수 있게 한다.

- active memory layers
- skipped memory layers
- applied budget bucket
- retrieval threshold
- compressed threshold

### 2. skip reason 가독성 개선

예:
- `retrieval skipped — budget tight`
- `compressed skipped — threshold not met`
- `cross-session folded`

같은 식으로 짧고 읽기 쉬운 문구를 쓴다.

### 3. RuntimeStatusBar 보조 노출

가능하면 마지막 run 기준으로:
- memory mode 약어
- retrieval/compressed 포함 여부

정도의 짧은 힌트를 노출한다.

단, 너무 시끄럽지 않게 최소 정보만 둔다.

### 4. memory layer 이름 통일

Trace/UI/로그에서 이름이 다르게 보이지 않게:
- `recent`
- `structured`
- `retrieval`
- `compressed`

같은 공통 라벨을 정리한다.

## 이번 단계에서 하지 않을 것

- threshold 자체 재설계
- vector retrieval 도입
- Runtime에서 memory policy 직접 조정 UI 추가
- 새 memory layer 추가

## 구현 원칙

- 새 정책을 만드는 단계가 아니라, 기존 정책을 읽기 쉽게 드러내는 단계다
- TracePanel은 설명 가능성 중심
- RuntimeStatusBar는 최소 신호만
- agent-first 원칙:
  - memory policy를 사람이 이해해야 결국 에이전트 품질도 더 잘 튜닝할 수 있다

## 성공 기준

- TracePanel에서 memory layer 포함/스킵 상태가 한눈에 보인다
- retrieval/compressed memory가 빠진 이유를 로그 없이 읽을 수 있다
- threshold tuning 결과가 실제 UI에서도 해석 가능해진다
- layer 이름과 용어가 UI/로그/코드에서 더 일관된다

## 후속

이 단계 다음은:

1. mode별 threshold 세분화 여부 검토
2. retrieval threshold 추가 튜닝
3. 그 후 vector/embedding path 재평가

순으로 이어진다.
