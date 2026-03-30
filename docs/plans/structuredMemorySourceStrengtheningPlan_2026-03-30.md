# Structured Memory Source Strengthening Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

Long-term memory의 1차로 compressed conversation memory가 들어왔다.

하지만 tunaFlow의 장기기억은 대화 요약만으로 충분하지 않다.
이 제품의 핵심은 단순 채팅이 아니라 작업 구조이므로,

- plans
- findings
- artifacts
- memos
- cross-session rows

같은 **구조화된 작업 객체**가 장기기억의 진실원에 더 가깝다.

## 목표

structured memory source를 ContextPack 안에서 더 명확하고 예측 가능하게 사용하도록 강화한다.

핵심은:
- 대화 continuity는 compressed memory가 담당
- 작업 continuity는 structured memory가 담당

로 역할을 더 선명히 나누는 것이다.

## 왜 지금 필요한가

### 1. 장기기억은 대화만으로 유지되지 않는다

- 오래된 recent context를 요약해도
- 실제 작업의 중요한 결정/판정/산출물은
  - plan
  - findings
  - artifacts
  - memo
에 담긴다

### 2. 현재는 포함은 되지만 정책이 약하다

지금도 plan/findings/artifacts/memo/cross-session은 ContextPack에 들어갈 수 있지만,
- 언제 무엇을 우선 포함하는지
- 어떤 것이 더 중요한 memory source인지
- source 간 역할 차이가 무엇인지

가 충분히 명확하지 않다.

### 3. retrieval 전에 structured memory를 먼저 세워야 한다

conversation vector retrieval은 나중에 들어와도,
우선 current task와 직접 연결된 structured source가 더 강해야 한다.

## 다루는 source

### 1. Plans

- 현재 task 구조
- subtask 상태
- owner
- 후속 실행 방향

### 2. Findings

- review/test/analysis에서 나온 핵심 발견
- 위험, 빠진 가정, 결정 포인트

### 3. Artifacts

- 설계안
- 구현 브리프
- handoff 문서
- review/test/decision 계열 문서

### 4. Memos

- 짧은 참고 조각
- 중요 포인트 pin
- 아직 artifact까지는 아닌 조각 기억

### 5. Cross-session

- 다른 conversation/branch/RT에서 남은 관련 요약

## 목표 정책

### 1. 역할 분리

- `compressed memory` = 오래된 대화 continuity
- `artifact/plan/findings` = structured task memory
- `memo` = lightweight pin/reference
- `cross-session` = 다른 세션에서 넘어온 연결 memory

### 2. 우선순위 정리

권장 기본 우선순위:

1. explicit source
2. current plan / active subtask
3. findings
4. recent relevant artifacts
5. compressed memory
6. memo
7. cross-session

즉 “대화 요약”보다 “현재 작업과 직접 연결된 구조화 객체”가 앞서야 한다.

### 3. source 품질 개선

- artifact는 제목/타입/상태만이 아니라 summary가 더 중요하다
- memo는 importance/scope가 약하면 노이즈가 된다
- cross-session은 현재 task relevance가 없으면 쉽게 부풀 수 있다

## 이번 단계에서 할 것

1. structured source 우선순위를 명문화하고 가능한 범위에서 ContextPack 조립에 반영
2. artifact / findings / memo / cross-session의 inclusion policy를 더 분명히 함
3. compressed memory와 structured memory의 역할이 Trace/Runtime에서 헷갈리지 않게 함

## 비목표

- vector retrieval
- memo 시스템 전면 재설계
- artifact editor 대형 확장
- plan schema 재설계
- knowledge graph

## 성공 기준

- 현재 task와 직접 관련된 structured source가 대화 요약보다 우선한다
- artifact/plan/findings/memo/cross-session의 역할이 더 명확해진다
- long-term memory가 “대화 요약 + 구조화 작업 memory”의 조합으로 보이기 시작한다

## 후속

이 단계 다음은:

1. conversation retrieval
2. unified memory policy

로 이어진다.
