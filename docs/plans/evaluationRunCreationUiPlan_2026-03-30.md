# Evaluation Run Creation UI Plan

상태: 제안
작성: 2026-03-30

## 목표

이미 연결된 `Test > Evaluation` 화면에서 사용자가 새 evaluation run을 직접 시작할 수 있게 한다.

## 현재 상태

- backend:
  - `create_eval_run`
  - `list_eval_runs`
  - `list_eval_results`
  - 기타 status/result command 존재
- frontend:
  - run 목록 보기 가능
  - run 상세 보기 가능
  - 생성 진입점 부족

## 제품 목표

- 사용자가 현재 conversation 맥락에서 비교 run을 시작할 수 있다
- 복잡한 평가 프레임워크가 아니라, 최소 입력으로 run을 만들 수 있다

## 1차 범위

### 생성 UI

- `New Run` 버튼
- 최소 입력 필드:
  - `title`
  - `prompt`
  - `mode` (`sequential` 등)
  - `rounds`
  - `participants`

### 생성 후 흐름

- run 생성 성공 시 목록 갱신
- 생성된 run 자동 선택
- 빈 결과 상태 표시

## 비목표

- auto execution orchestration
- rubric/scoring
- template library
- bulk run management

## 권장 UX

- `Test > Evaluation` 우측 상단에 `New Run`
- 클릭 시 modal 또는 inline form
- 1차는 modal이 더 안전

## 성공 기준

- 사용자가 eval run을 생성할 수 있다
- 생성 후 목록과 상세 뷰가 일관되게 갱신된다
- 기존 결과 보기 UI를 깨지 않는다

## 메모

이 단계는 evaluation을 “볼 수 있는 기능”에서 “시작할 수 있는 기능”으로 올리는 최소 연결 단계다.
