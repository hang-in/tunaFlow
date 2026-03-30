# Handoff Truncation Fix Plan

상태: 제안
작성: 2026-03-30

## 문제

`sendFollowup` 경로가 이전 메시지 또는 artifact 내용을 약 800자로 truncate해 다음 agent prompt에 포함한다.

이 제한 때문에:
- 긴 설계안/리뷰안이 reviewer/tester에게 전문으로 전달되지 않을 수 있다
- handoff validation에서 "이전 산출물을 못 본다"는 오해가 생길 수 있다

## 목표

긴 artifact나 메시지를 handoff할 때, 현재보다 더 신뢰할 수 있게 전달한다.

## 우선 방향

### 1차

- 단순 길이 상향이 아니라 `artifact handoff`를 우선 별도 취급
- artifact 기반 handoff에서는 전문 또는 더 큰 상한을 사용
- 일반 followup 텍스트 경로는 기존 보호 장치를 유지하거나 완만히 상향

### 2차

- 필요하면 요약 + 원문 일부 + 메타 구조로 조합
- 무조건 전체 전문 전달은 context budget과 함께 재검토

## 비목표

- prompt budget 전면 재설계
- context pack 전체 구조 변경
- vector retrieval 도입

## 구현 제안

### Option A

- `sendFollowup`의 800자 제한을 완화
- 가장 빠르지만 budget 관리가 거칠다

### Option B

- artifact handoff 전용 경로를 둔다
- artifact일 때는 전문 또는 더 큰 limit 사용
- 일반 메시지 followup은 기존 제한 유지

권장: **Option B**

## 성공 기준

- 긴 artifact를 reviewer/tester로 넘겼을 때 전문 또는 충분한 본문이 실제로 전달된다
- handoff validation에서 reviewer/tester가 이전 산출물 내용을 구체적으로 참조한다
- 기존 일반 followup과 context budget을 과도하게 깨지 않는다

## 메모

이 작업은 persona validation보다 handoff validation의 선행 조건에 가깝다.
