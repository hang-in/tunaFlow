# Evaluation Under Test Plan

상태: 제안
작성: 2026-03-30

## 결정

`Evaluation`은 메인 1급 탭으로 승격하지 않고, `Test` 탭 하위의 비교/검증 뷰로 배치한다.

## 이유

- Evaluation은 성격상 `검증/비교`에 가깝다
- `Chat / Plan / Artifacts / Review / Test`는 일상 메인 워크플로이고, Evaluation은 아직 그 수준의 상시 모드는 아니다
- 지금 단계에서 메인 탭을 하나 더 늘리면 IA가 무거워진다

## 권장 IA

- 메인 탭:
  - `Chat`
  - `Plan`
  - `Artifacts`
  - `Review`
  - `Test`
- `Test` 내부 서브 뷰:
  - `Reports`
  - `Evaluation`

## 범위

- 기존에 추가된 `Eval` 메인 탭이 있다면 제거
- `TestPanel` 또는 그 하위 구조 안에 `Evaluation` 서브 뷰를 배치
- evaluation run 목록, 상세 비교, refresh/create 액션은 유지

## 비목표

- evaluation 기능 제거
- evaluation backend 변경
- scoring/rubric 추가

## 성공 기준

- 사용자는 `Test` 탭 안에서 evaluation run을 볼 수 있다
- 메인 탭 수는 늘어나지 않는다
- 기존 evaluation UI 기능은 유지된다

## 메모

나중에 evaluation 사용 빈도가 충분히 높아지면 독립 탭 승격을 다시 검토할 수 있다. 현재는 `Test` 하위가 가장 자연스럽다.
