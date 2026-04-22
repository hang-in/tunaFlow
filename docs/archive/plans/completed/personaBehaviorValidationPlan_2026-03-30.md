# Persona Behavior Validation Plan

상태: 제안
작성: 2026-03-30

## 목표

현재 구현된 persona 시스템이 설정/UI 수준을 넘어 실제 응답 행동 차이로 이어지는지 검증한다.

핵심 질문:
- `promptFragment`가 실제 runtime prompt에 안정적으로 들어가는가
- `General`, `Reviewer`, `Tester`가 같은 입력에 대해 구분되는 출력 경향을 보이는가
- 4개 엔진 모두에서 개념이 일관되게 적용되는가

## 범위

- Persona 구현 추가가 아니라 검증/평가
- 기본 검증 대상:
  - `General`
  - `Reviewer`
  - `Tester`
- 가능하면 동일 또는 유사 입력을 4개 엔진에 적용

## 비목표

- 새 persona 추가
- persona editor 확장
- 자동 skill selection
- prompt 시스템 전체 재설계

## 검증 시나리오

### Scenario 1: General

- 중립적인 구현/설명 요청
- 기대:
  - 과도하게 비판적이지 않음
  - 범용 조언/설명 중심

### Scenario 2: Reviewer

- 코드/설계 검토 요청
- 기대:
  - 문제/리스크 중심 응답
  - findings-first 경향
  - 승인/반려/보류 판단에 가까운 출력

### Scenario 3: Tester

- 테스트 전략 또는 검증 요청
- 기대:
  - 테스트 케이스/실패 조건/검증 항목 중심
  - 재현/검증 절차에 더 민감

## 평가 방법

1. 같은 작업 프롬프트를 persona만 바꿔 실행
2. 응답 차이를 비교
3. 최소한 아래를 확인
   - tone 차이
   - output structure 차이
   - task focus 차이
4. 가능하면 trace 또는 debug 경로로 `## Persona` section 존재도 확인

## 성공 기준

- `General / Reviewer / Tester`가 눈에 띄게 구분되는 출력 경향을 보인다
- 적어도 Claude/Codex/Gemini/OpenCode에서 persona section 주입이 빠지지 않는다
- 현재 persona 스키마를 유지해도 제품 가치가 있다고 판단할 수 있다

## 후속 판단

검증 후 아래 중 하나를 선택한다.

1. 현재 persona baseline 유지 후 polish
2. 특정 persona(예: Tester) 보강
3. persona fragment 규칙 재설계

## 메모

이 작업은 기능 추가보다 신뢰 검증 성격이 강하다. 지금은 이미 `Agent Profile + Persona + runtime binding`이 붙어 있으므로, 실제 행동 차이를 확인해 제품 축이 맞는지 먼저 판단하는 것이 중요하다.
