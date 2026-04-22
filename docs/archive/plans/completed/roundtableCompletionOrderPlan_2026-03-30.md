# Roundtable Deliberative Completion-Order Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

현재 Roundtable `Deliberative` 모드는 participant subprocess를 병렬로 시작한다.

하지만 결과 수집은 `JoinHandle::join()`을 participant 배열 순서대로 호출한다.

즉:
- 뒤 participant가 이미 끝났어도
- 앞 participant가 느리면
- 결과 표시/저장/진행 이벤트가 함께 막힌다

이건 논문식 분산 시스템 관점에서 전형적인 reduce 단계 straggler 병목이다.

## 목표

`Deliberative` 모드의 결과 수집을 `participant order`가 아니라 `completion order` 기준으로 바꾼다.

핵심은:
- 느린 첫 participant가 전체 UI/저장을 막지 않게 하고
- 먼저 끝난 participant는 즉시 persist + emit 하게 만드는 것

이다.

## 왜 필요한가

### 1. 현재 병목은 구현 비효율이다

Sequential은 설계상 직렬이라 괜찮다.

하지만 Deliberative는 병렬 fan-out을 하면서도 reduce를 배열 순서대로 묶고 있으므로,
의도된 토론 품질 문제가 아니라 순수한 구현 병목이다.

### 2. 체감 RT UX가 개선된다

완료된 participant가 즉시 보이면:
- progress가 더 자연스럽고
- 긴 대기 시간이 줄어든 것처럼 느껴지며
- straggler 하나 때문에 전체 RT가 멈춘 듯 보이지 않는다.

### 3. 다음 blind verifier 단계의 기반이 된다

blind verifier를 나중에 추가하더라도,
기본 fan-out / collect 구조가 completion-order여야 운영이 더 자연스럽다.

## 이번 단계에서 할 것

### 1. Deliberative result collection을 completion-order로 전환

방법은 구현 자유지만 목표는 동일하다.

예:
- worker thread가 결과를 channel로 보내고
- 메인 수집 루프가 수신 순서대로 persist + emit

배열 순서 `join`은 피한다.

### 2. participant별 상태 이벤트 유지

기존:
- running emit
- done/error emit
- roundtable:progress emit

흐름은 유지하되, 완료 순서가 실제 completion-order가 되게 한다.

### 3. transcript/round_responses semantics 보존

Deliberative는 원래 same-round peer context를 보지 않는다.

따라서 completion-order로 바꿔도:
- prompt semantics
- current_round_refs 없음

은 유지되어야 한다.

### 4. 최소 테스트 보강

가능하면:
- completion-order 수집 로직
- participant order와 무관한 persist 순서

를 보호하는 테스트를 추가한다.

## 이번 단계에서 하지 않을 것

- Sequential 모드 재설계
- blind verifier phase 도입
- role-based output cap
- lead decomposition

## 구현 원칙

- Deliberative의 의미는 유지하고, reduce 병목만 제거한다
- UI 이벤트와 DB persist는 실제 완료 순서를 반영하게 한다
- participant 배열 순서는 기본 config/reference 용도로만 남고, 완료 순서를 강제하지 않게 한다

## 성공 기준

- Deliberative에서 먼저 끝난 participant가 즉시 표시된다
- 느린 participant 하나가 뒤 결과 표시를 막지 않는다
- 기존 prompt semantics는 바뀌지 않는다
- Roundtable UX가 더 responsive해진다

## 후속

이 단계 다음은:

1. blind verifier phase
2. role-based output cap
3. lead decomposition 별도 milestone

순으로 이어진다.
