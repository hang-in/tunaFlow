# Artifact Navigation Actions Plan

상태: 제안
작성: 2026-03-30

## 목표

Artifacts 탭과 상세 모달에서 provenance/workflow 정보를 단순 표시에서 끝내지 않고 실제 탐색 액션으로 연결한다.

## 범위

- artifact 카드 또는 상세 모달에서 source conversation/branch/RT로 이동
- subtaskId가 있으면 관련 plan/subtask로 점프
- forward 액션이 현재 어떤 엔진/대화로 보내는지 더 명확히 표시

## 비목표

- artifact 자동 승격
- artifact graph 뷰
- artifact 기반 plan 자동 생성
- cross-project 탐색

## 사용자 가치

- 저장된 artifact를 다시 열었을 때 원래 맥락으로 빠르게 돌아갈 수 있다
- artifact를 문서 저장소가 아니라 워크플로 진입점으로 쓸 수 있다
- provenance가 단순 텍스트가 아니라 실제 내비게이션 역할을 한다

## 구현 제안

### Phase 1

- 카드 provenance line의 conversation/branch/RT 텍스트를 클릭 가능하게 만든다
- 클릭 시 해당 conversation/thread를 열고 필요한 경우 drawer/overlay를 연다
- 상세 모달 Source 행에서도 동일한 액션을 제공한다

### Phase 2

- subtask linked가 있으면 Plans 탭의 해당 subtask로 포커스 이동한다
- 이동 실패 시 조용히 무시하지 말고 최소한의 toast 또는 상태 피드백을 준다

### Phase 3

- Forward 버튼 라벨/툴팁을 보강해 현재 대화 전달인지, 특정 엔진 전달인지 더 분명히 보여준다

## 검증 기준

- artifact 카드에서 source를 눌렀을 때 올바른 conversation/branch/RT가 열린다
- artifact 모달에서도 동일한 이동이 동작한다
- subtask linked artifact에서 Plans 탭으로 이동 가능하다
- 기존 artifact 상세 보기와 상태 변경 액션은 깨지지 않는다

## 메모

이 단계는 search보다 우선한다. 사용자는 이미 artifact를 찾은 상태에서 "어디서 왔는가"와 "어디로 돌아갈 수 있는가"를 더 자주 필요로 한다.
