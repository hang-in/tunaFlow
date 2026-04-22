# Evaluation UI Connection Plan

상태: 제안
작성: 2026-03-30

## 목표

이미 구현된 evaluation backend(`eval_runs`, `eval_results`, 관련 6개 command`)를 실제 frontend UI에 연결한다.

## 현재 상태

- backend:
  - `eval_runs`
  - `eval_results`
  - create/list/add/update/delete command 존재
- frontend:
  - 전용 UI 없음
  - 사용자가 평가 run과 결과를 볼 수 없음

## 제품 목표

- 같은 conversation 또는 RT 맥락에서 여러 agent 결과를 비교 가능한 최소 UI를 제공
- evaluator framework를 크게 만들기보다, 현재 있는 backend를 바로 쓰는 1차 비교 화면을 붙인다

## 범위

### Phase 1

- Evaluation run 목록 보기
- run 상세에서 round별/agent별 결과 보기
- status 표시 (`running / done / failed`)
- conversation 범위 기준으로 연결

### Phase 2

- run 생성 최소 UI
- title / prompt / mode / rounds / participants 입력
- 완료 후 결과 자동 갱신 또는 수동 refresh

### Phase 3

- 결과 비교 가독성 개선
- agent별 token/cost/duration 메타 표시
- 결과 복사/forward 등 보조 액션

## 권장 UI 위치

- 1차는 `Review / Test`와 분리된 `Evaluation` 전용 메인 탭보다는,
  기존 구조를 덜 흔드는 위치가 더 적절하다
- 권장:
  - `Review` 또는 `Artifacts`에서 독립 섹션으로 시작하지 말고
  - `Test`와 인접한 새 메인 탭 `Eval` 또는 `Evaluation` 추가 검토

권장 결론:
- 메인 탭에 `Evaluation` 추가

## 비목표

- auto judge / scoring
- rubric editor
- LLM-as-a-judge 전체 프레임워크
- benchmark dashboard

## 성공 기준

- 사용자가 conversation 기준으로 eval run 목록을 볼 수 있다
- run을 열면 participant/round별 결과를 읽을 수 있다
- backend command가 실제 UI에서 연결된다

## 메모

이 작업은 새 평가 시스템 구축이 아니라, 이미 있는 evaluation backend를 사용자 화면에 연결하는 작업이다.
