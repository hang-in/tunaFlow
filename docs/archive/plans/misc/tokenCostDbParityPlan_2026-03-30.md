# Token Cost DB Parity Plan

상태: 제안
작성: 2026-03-30

## 문제

현재 UI에서는 엔진별로 `N/A` 표기를 일부 보강했지만, DB 레벨에서는 여전히 아래가 구분되지 않는다.

- 실제 값이 0인 경우
- 엔진이 usage/cost를 제공하지 않는 경우
- 아직 수집되지 않은 경우

이 때문에 trace/log/집계에서 의미가 흐려진다.

## 목표

usage/cost 데이터를 DB 레벨에서 더 정확히 표현해, frontend 표시와 backend 집계를 일치시킨다.

## 범위

- trace/log 또는 usage 저장 테이블의 상태 표현 보강
- 최소한 `exact / unavailable / unknown` 수준의 구분
- 기존 UI `N/A` 표기와 정합성 맞추기

## 비목표

- cost 추정 모델 도입
- provider별 billing 정확도 향상
- analytics 대시보드 구축

## 권장 방향

### Option A

- `usage_status` 또는 동등 필드를 추가한다
- 값:
  - `exact`
  - `unavailable`
  - `unknown`

### Option B

- token/cost를 nullable로 바꾸고 null 의미를 문서화한다
- 단순하지만 상태 의미가 덜 풍부하다

권장: **Option A**

## 구현 포인트

- DB schema/migration
- Rust 모델과 serialize 타입
- trace/runtime/frontend formatting 정합성
- 기존 데이터 migration 시 default 처리

## 성공 기준

- OpenCode/Gemini one-shot처럼 usage 미지원 엔진이 DB에서 `0`처럼 보이지 않는다
- frontend `N/A` 표기와 backend 상태가 일치한다
- 기존 trace/runtime 집계 로직이 상태를 구분해 처리한다

## 메모

이 작업은 새 기능이라기보다 데이터 의미 보강이다. 최근 runtime/trace 가시화가 올라온 만큼, 이제 DB 의미도 맞춰야 한다.
