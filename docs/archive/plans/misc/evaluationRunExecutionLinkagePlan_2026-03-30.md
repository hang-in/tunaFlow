# Evaluation Run Execution Linkage Plan

상태: 제안
작성: 2026-03-30

## 목표

생성된 evaluation run이 빈 컨테이너로 남지 않도록, 실제 agent 실행과 결과 저장 흐름을 연결한다.

## 현재 상태

- run 생성 가능
- run 목록/상세 보기 가능
- backend에 `add_eval_result`, `update_eval_run_status` 존재
- 하지만 생성한 run이 실제 agent 결과로 채워지는 기본 흐름이 약하다

## 제품 목표

- 사용자가 evaluation run을 만들고, 실제 비교 결과를 얻을 수 있다
- 1차는 완전 자동 평가 프레임워크가 아니라, “같은 prompt를 여러 agent에 보내고 결과를 run에 저장”하는 최소 연결이면 충분하다

## 1차 범위

### 실행 연결

- run 생성 후 선택한 participant/engine에 prompt를 보내는 최소 실행 액션
- 결과를 `eval_results`에 저장
- run status를 `running → done/failed`로 갱신

### UI

- `Run` 또는 `Execute` 버튼
- 진행 상태 표시
- 결과가 들어오면 상세 뷰에 즉시 반영

## 권장 방식

- 1차는 현재 존재하는 agent 실행 경로를 재사용
- eval 전용 orchestration을 크게 만들지 않는다
- sequential mode 우선

## 비목표

- scoring
- auto judge
- rubric
- tournament / matrix compare
- parallel distributed execution

## 성공 기준

- 사용자가 run을 생성하고 실행할 수 있다
- 최소 2개 participant 결과가 `eval_results`에 저장된다
- run status가 적절히 갱신된다
- Evaluation 상세 뷰에서 비교가 가능하다

## 메모

이 단계는 evaluation을 실제 제품 기능으로 올리는 핵심 연결 단계다. run 생성만 있고 결과가 비어 있으면 기능 가치가 약하다.
