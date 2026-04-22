# Evaluation Usability Pass Plan

상태: 제안
작성: 2026-03-30

## 목표

실제 실행 가능한 Evaluation 기능 위에 최소한의 사용성 보강을 얹어, 반복 실행과 결과 활용이 쉬운 상태로 만든다.

## 현재 상태

- run 생성 가능
- 실제 CLI agent 실행 가능
- 결과 저장 및 비교 가능
- 하지만 실사용 관점에서 반복 실행/실패 복구/결과 재활용 UX가 약하다

## 우선 범위

### 1. 실행 제어

- 실행 중 `Cancel`
- 완료 또는 실패 후 `Retry`

### 2. run 편집성

- participant/engine 구성을 다시 확인하거나 최소 수정 가능
- run 생성 직후 실수 수정 부담 완화

### 3. 결과 활용

- 결과 복사
- 결과를 대화나 artifact로 forward
- 실패/빈 상태/부분 완료 상태를 더 명확히 표시

## 비목표

- scoring
- judge
- rubric
- 고급 benchmark 관리

## 권장 순서

### Phase 1

- Cancel / Retry
- failed / empty / partial 상태 polish

### Phase 2

- participant 편집 또는 최소 재설정
- 결과 복사 / forward

## 성공 기준

- 사용자가 잘못 시작한 run을 취소할 수 있다
- 실패한 run을 다시 실행하기 쉽다
- 결과를 다시 활용하는 액션이 생긴다
- Evaluation이 단발 데모가 아니라 반복 가능한 도구처럼 느껴진다

## 메모

이 단계는 새 평가 기능 추가가 아니라, 방금 완성된 execution 기능을 실제로 자주 쓸 수 있게 만드는 polish 단계다.
