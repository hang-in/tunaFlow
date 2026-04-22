# ContextPack P0 Phase 1 Plan

상태: 제안
작성: 2026-03-30

## 결정

다음 기술 라운드의 P0는 `ContextPack 고도화`다.

단, 1차 구현 순서는 아래처럼 잡는다.

1. section visibility / traceability
2. compression 개선
3. context budget scaling UI

## 왜 이 순서인가

### 1. visibility 먼저

- 지금은 어떤 section이 실제로 prompt에 들어갔는지 사용자가 충분히 보기 어렵다
- visibility 없이 budget만 올리면 “왜 답이 달라졌는지” 설명이 안 된다

### 2. compression 두 번째

- 긴 section이 잘릴 때 현재보다 더 좋은 축약 품질이 필요하다
- budget을 무조건 키우기 전에 selection/compression 품질을 먼저 올려야 한다

### 3. budget UI는 마지막

- 사용자가 직접 조절할 필요는 아직 제한적이다
- 먼저 시스템이 무엇을 넣고 줄이는지 보이게 해야 조정 UI도 의미가 생긴다

## Phase 1 범위

### A. Section visibility

- trace 또는 runtime surface에서 실제 포함된 ContextPack section 표시
- 최소 표시 대상:
  - persona
  - plan
  - findings
  - artifacts
  - skills
  - rawq
  - cross-session
  - thread inheritance

### B. Compression 개선

- 긴 section이 truncate fallback으로 가기 전에 더 나은 요약/축약 경로 확인
- 현재 compression 동작의 결과를 더 명확히 알 수 있게 함

### C. Budget scaling 준비

- 지금 단계에서는 설정 UI를 바로 여는 대신,
- 현재 budget/section inclusion trace를 기준으로 상향 실험 준비

## 비목표

- context-hub 연동
- flow agent
- vector retrieval
- 전면적인 prompt 시스템 재설계

## 성공 기준

- 사용자가 어떤 section이 실제로 들어갔는지 볼 수 있다
- 긴 section이 왜/어떻게 줄었는지 추적 가능하다
- 이후 budget scaling UI를 붙일 근거가 생긴다

## 메모

이 단계는 “더 많이 넣기”보다 “무엇을 넣는지 보이게 하고, 줄일 때 더 잘 줄이는 것”이 핵심이다.
