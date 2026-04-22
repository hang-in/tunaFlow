# Context Stack Reevaluation Plan

상태: 제안
작성: 2026-03-30

## 목표

다음 기술 라운드에서 `ContextPack`, `context-hub`, `flow agent` 중 무엇을 먼저 구현해야 하는지 다시 좁힌다.

## 왜 지금 필요한가

- 제품 표면 기능은 많이 올라왔다
  - Agent Profile / Persona
  - Runtime
  - Artifacts
  - Search
  - Git sync
  - Evaluation
- 다음 큰 점프는 “더 좋은 컨텍스트 공급/선택/적용” 계층일 가능성이 높다
- 하지만 `ContextPack`, `context-hub`, `flow agent`는 서로 연결돼 있어 순서를 잘못 잡으면 placeholder나 과설계가 되기 쉽다

## 세 축 정의

### 1. ContextPack

- 현재 실행 시점 prompt 조립 계층
- 이미 제품에 존재
- 다음 후보:
  - traceability 강화
  - selection/compression 고도화
  - dynamic budget

### 2. context-hub

- docs / external knowledge 공급층
- 아직 제품 코드에 직접 연결되진 않음
- `search / get / annotate` 성격의 온디맨드 공급 경로

### 3. flow agent

- 더 높은 수준의 orchestration / routing / context decision 계층
- 현재는 선행 조건이 덜 갖춰진 상태
- context-hub와 ContextPack 위에 올라갈 가능성이 큼

## 평가 기준

### 제품 가치

- 사용자 체감이 큰가
- 지금 tunaFlow 화면/워크플로에 자연스럽게 붙는가

### 선행 조건

- 지금 이미 필요한 셸/메타/실행 경로가 있는가
- placeholder 없이 실제 기능이 되는가

### 리스크

- 범위가 너무 커지지 않는가
- 현재 안정화한 기능을 흔들지 않는가

## 현재 가설

### 후보 A: ContextPack 고도화 먼저

장점:
- 기존 제품 축 위에서 바로 개선 가능
- handoff / retrieval / prompt 품질 향상에 직접 연결

단점:
- 잘못하면 내부 구조 최적화에만 머물 수 있음

### 후보 B: context-hub 최소 연동 먼저

장점:
- 외부 docs 공급층을 실제 기능으로 붙일 수 있음
- 이후 Knowledge Sources와 flow agent의 기반이 됨

단점:
- Settings shell 없이 붙이면 UX가 어정쩡함
- 실제 어떤 화면에서 쓰일지 더 정리 필요

### 후보 C: flow agent 먼저

장점:
- 장기적으로 가장 큰 도약

단점:
- 지금은 선행 조건 부족
- orchestration이 추상화만 많고 실기능이 빈약해질 위험

## 현재 권장 결론

1. `ContextPack 고도화` 또는 `context-hub 최소 연동` 중 하나를 먼저 고른다
2. `flow agent`는 그 다음 라운드
3. 단, `Knowledge Sources shell` 같은 placeholder는 다시 만들지 않는다

## 산출물

- P0: 다음 한 라운드에 바로 구현할 대상 1개
- P1: 그 다음 후보 1개
- Hold: 아직 이르거나 자리만 만들 가능성이 큰 항목

## 메모

이번 작업은 새 기능 구현이 아니라 다음 큰 기술 축의 순서를 고정하는 판단 작업이다.
