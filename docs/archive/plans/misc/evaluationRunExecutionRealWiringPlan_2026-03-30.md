# Evaluation Run Execution Real Wiring Plan

상태: 제안
작성: 2026-03-30

## 목표

현재 placeholder 기반 evaluation execution skeleton을 실제 agent 실행 경로와 연결해, eval run이 진짜 응답 결과로 채워지게 만든다.

## 현재 상태

- `Execute` 버튼 존재
- progress/status UI 존재
- `add_eval_result` 저장 흐름 존재
- 하지만 실제 CLI agent 호출은 placeholder

## 목표 상태

- evaluation run 실행 시 실제 agent profile/engine 경로가 호출된다
- 각 round/agent 결과가 `eval_results`에 저장된다
- run status가 실제 실행 결과에 맞게 갱신된다

## 권장 방식

- 1차는 현재 agent 실행 경로를 재사용
- sequential mode 우선
- evaluation 전용 신규 orchestration을 크게 만들지 말 것

## 범위

### 실행

- agent profile별 engine/model/persona/default skills 반영
- 실제 `start_*` 또는 동등 실행 경로 호출
- 응답 결과를 eval result로 저장

### 상태

- `running / done / failed`
- agent별 실패가 있으면 run 전체 상태 정책을 명확히 정함

### UI

- 현재 progress 표시 유지
- 실제 결과가 들어오면 상세 카드에 반영

## 비목표

- scoring
- judge
- distributed execution
- parallel mode 고도화

## 성공 기준

- placeholder 없이 실제 agent 응답이 eval 결과에 저장된다
- 최소 2개 agent profile 결과를 비교할 수 있다
- 기존 run 생성/목록/상세 흐름을 깨지 않는다

## 메모

이 단계가 완료돼야 evaluation이 진짜 제품 기능이 된다.
