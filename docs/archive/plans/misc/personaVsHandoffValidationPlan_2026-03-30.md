# Persona vs Handoff Validation Plan

상태: 제안
작성: 2026-03-30

## 문제 정의

기존 persona 검증은 아래 두 가지를 한 번에 섞어서 확인하려 했다.

1. persona가 실제 응답 스타일/초점 차이를 만드는가
2. 다른 agent/persona의 직전 응답을 다음 agent가 자동으로 참조하는가

이 둘은 서로 다른 검증 대상이다.

## 결론

- `persona validation`과 `handoff validation`을 분리한다
- persona는 동일 입력 비교로 검증한다
- handoff는 artifact 또는 명시적 source를 넘기는 시나리오로 검증한다

## Track A: Persona Validation

### 목표

`General / Reviewer / Tester`가 같은 입력에 대해 구분되는 출력 경향을 보이는지 확인한다.

### 방법

- 동일한 질문을 3개 persona에 각각 독립 실행
- 비교 항목:
  - tone
  - output structure
  - task focus

### 예시 입력

`GraphQL API에 JWT 인증을 붙이는 방향을 제안해줘. 구현 순서와 주의점을 간단히 정리해줘.`

### 기대

- `General`: 균형 잡힌 설명/제안
- `Reviewer`: 문제점/리스크/빠진 가정 우선
- `Tester`: 테스트 관점/검증 포인트 우선

## Track B: Handoff Validation

### 목표

한 agent가 만든 결과를 다른 agent가 실제 입력으로 참조할 수 있는지 확인한다.

### 방법

1. `General` 또는 `Architect`가 초안을 생성
2. 그 결과를 아래 둘 중 하나로 명시적으로 전달
   - artifact로 저장
   - source message / selected content / forwarded content로 전달
3. `Reviewer` 또는 `Tester`가 그 산출물을 대상으로 후속 작업 수행

### 검증 질문

- reviewer가 실제로 초안 내용을 인용/참조하는가
- tester가 초안의 특정 항목을 기준으로 케이스를 만드는가
- 자동 인용이 아니라면 현재 제품에서 어떤 handoff 방식이 필요한가

## 성공 기준

### Persona

- 같은 입력에서 persona별 차이가 눈에 보인다

### Handoff

- reviewer/tester가 이전 산출물을 실제로 참조한다
- 자동이 아니면 어떤 명시적 handoff가 필요한지 정리된다

## 메모

reviewer가 “설계안 전문이 없다”고 응답한 것은 persona 실패 신호라기보다 handoff 입력이 실제로 전달되지 않았다는 신호일 수 있다.
